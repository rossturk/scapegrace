// Encapsulates all mouse/keyboard interaction for the overworld canvas.

import type { BundledCampaign } from '../types/pack';
import type { OwCanvasState } from './overworld-renderer';
import { hitTestRegion, hitTestHandle } from './overworld-renderer';

export interface PopupInfo {
  x: number;
  y: number;
  type: 'connection' | 'node';
  connIdx?: number;
  nodeIdx?: number | string;
  nodeW?: number; // tile width of the node
}

export interface OwInteractionCallbacks {
  onRedraw: () => void;
  onSelectNode: (nodeIdx: number | string | null) => void;
  onOpenLevel: (levelIdx: number) => void;
  onConnectionCreated: (fromId: string, toId: string) => void;
  onWaypointMoved: (connKey: string, wpIdx: number, tx: number, ty: number) => void;
  onWaypointDeleted: (connKey: string, wpIdx: number) => void;
  onNodeResized: (nodeId: string | number, w: number, h: number) => void;
  onShowPopup: (popup: PopupInfo) => void;
  onRegionMoved: () => void;
  onToast: (msg: string, type: string) => void;
}

export class OverworldInteraction {
  private canvas: HTMLCanvasElement;
  private campaign: BundledCampaign;
  private state: OwCanvasState;
  private callbacks: OwInteractionCallbacks;
  private handlers: { event: string; handler: any; options?: any }[] = [];
  private dragWaypoint: { connKey: string; wpIdx: number } | null = null;
  private resizing: { nodeId: string | number; originTileX: number; originTileY: number } | null = null;

  constructor(
    canvas: HTMLCanvasElement,
    campaign: BundledCampaign,
    state: OwCanvasState,
    callbacks: OwInteractionCallbacks,
  ) {
    this.canvas = canvas;
    this.campaign = campaign;
    this.state = state;
    this.callbacks = callbacks;
    this.attach();
  }

  private on(event: string, handler: any, options?: any) {
    this.canvas.addEventListener(event, handler, options);
    this.handlers.push({ event, handler, options });
  }

  private getMousePos(e: MouseEvent): { mx: number; my: number } {
    const rect = this.canvas.getBoundingClientRect();
    return { mx: e.clientX - rect.left, my: e.clientY - rect.top };
  }

  private getVpSize(): { vpW: number; vpH: number } {
    const rect = this.canvas.getBoundingClientRect();
    return { vpW: rect.width, vpH: rect.height };
  }

  private resolveNodeId(nodeIdx: number | string): string {
    if (typeof nodeIdx === 'string') return nodeIdx;
    const levels = this.campaign.overworld.levels || [];
    const md = this.state.mapData;
    const storeRegion = md?.regions?.find((r: any) => r.node_idx > levels.length);
    if (nodeIdx === 0) return 'start';
    if (nodeIdx === storeRegion?.node_idx) return 'store';
    return 'level_' + (nodeIdx - 1);
  }

  private resolveNodeIdx(id: string | number): number | null {
    const str = String(id);
    if (str === 'start') return 0;
    const levels = this.campaign.overworld.levels || [];
    const md = this.state.mapData;
    const storeRegion = md?.regions?.find((r: any) => r.node_idx > levels.length);
    if (str === 'store' || str === 'end') return storeRegion?.node_idx ?? null;
    const m = str.match(/level_(\d+)/);
    return m ? parseInt(m[1]) + 1 : null;
  }

  // Convert screen position to tile coordinate
  private screenToTile(mx: number, my: number): { tx: number; ty: number } | null {
    const s = this.state;
    const md = s.mapData;
    if (!md) return null;
    const { vpW, vpH } = this.getVpSize();
    const tz = 4 * s.zoom;
    const baseOx = (vpW - md.width * tz) / 2 + s.panX;
    const baseOy = (vpH - md.height * tz) / 2 + s.panY;
    const tx = Math.floor((mx - baseOx) / tz);
    const ty = Math.floor((my - baseOy) / tz);
    return { tx, ty };
  }

  // Find which connection owns the tile at screen position, using cached hallway paths
  private findConnectionAt(mx: number, my: number): number | null {
    const s = this.state;
    const c = this.campaign;
    if (!s.hallwayCache) return null;
    const tile = this.screenToTile(mx, my);
    if (!tile) return null;
    const { tx, ty } = tile;

    const conns = c.overworld.connections || [];
    // Check each connection's cached path for a tile match (with 1-tile tolerance for walls)
    for (let ci = conns.length - 1; ci >= 0; ci--) {
      const path = s.hallwayCache.get(`${conns[ci][0]}->${conns[ci][1]}`);
      if (!path) continue;
      for (const [px, py] of path) {
        if (Math.abs(px - tx) <= 1 && Math.abs(py - ty) <= 1) {
          return ci;
        }
      }
    }
    return null;
  }

  private attach() {
    const s = this.state;
    const cb = this.callbacks;
    let dragMoved = false;

    // Double-click: open level OR zoom in on empty space
    this.on('dblclick', (e: MouseEvent) => {
      const { mx, my } = this.getMousePos(e);
      const { vpW, vpH } = this.getVpSize();
      const nodeIdx = hitTestRegion(mx, my, vpW, vpH, s, this.campaign);
      if (nodeIdx !== null && typeof nodeIdx === 'number' && nodeIdx > 0) {
        cb.onOpenLevel(nodeIdx - 1);
      } else if (nodeIdx === null) {
        const { vpW, vpH } = this.getVpSize();
        const newZoom = Math.min(5, s.zoom * 1.5);
        const md = s.mapData;
        const gridW = md ? md.width : 60;
        const gridH = md ? md.height : 36;
        const oldTz = 4 * s.zoom, newTz = 4 * newZoom;
        const oldBaseOx = (vpW - gridW * oldTz) / 2 + s.panX;
        const oldBaseOy = (vpH - gridH * oldTz) / 2 + s.panY;
        const wx = (mx - oldBaseOx) / oldTz;
        const wy = (my - oldBaseOy) / oldTz;
        s.panX = (mx - wx * newTz) - (vpW - gridW * newTz) / 2;
        s.panY = (my - wy * newTz) - (vpH - gridH * newTz) / 2;
        s.zoom = newZoom;
        cb.onRedraw();
      }
    });

    // Mouse down: start pan, region drag, or connection drag
    this.on('mousedown', (e: MouseEvent) => {
      if (e.button !== 0) return;
      dragMoved = false;
      const { mx, my } = this.getMousePos(e);
      const { vpW, vpH } = this.getVpSize();

      // Check waypoint drag first
      const wp = this.hitTestWaypoint(mx, my);
      if (wp) {
        this.dragWaypoint = wp;
        s.lastMouse = { x: e.clientX, y: e.clientY };
        return;
      }

      // Check resize corner handles on selected room
      const resizeHit = this.hitTestResizeCorner(mx, my, vpW, vpH);
      if (resizeHit) {
        this.resizing = resizeHit;
        s.lastMouse = { x: e.clientX, y: e.clientY };
        return;
      }

      const handle = hitTestHandle(mx, my, vpW, vpH, this.campaign, s);
      if (handle && handle.type === 'exit') {
        s.connectingFrom = handle;
        s.connectMousePos = { x: handle.sx, y: handle.sy };
        return;
      }

      const hit = hitTestRegion(mx, my, vpW, vpH, s, this.campaign);
      if (hit !== null) {
        s.dragRegion = hit as any;
      } else {
        s.dragging = true;
        cb.onShowPopup(null as any); // dismiss popup on pan
      }
      s.lastMouse = { x: e.clientX, y: e.clientY };
    });

    // Mouse move
    this.on('mousemove', (e: MouseEvent) => {
      if (this.resizing) {
        const { mx, my } = this.getMousePos(e);
        const tile = this.screenToTile(mx, my);
        if (tile) {
          dragMoved = true;
          const newW = Math.max(4, tile.tx - this.resizing.originTileX + 1);
          const newH = Math.max(4, tile.ty - this.resizing.originTileY + 1);
          cb.onNodeResized(this.resizing.nodeId, newW, newH);
          s.hallwayCacheKey = null;
          cb.onRedraw();
        }
        return;
      }

      if (this.dragWaypoint) {
        const { mx, my } = this.getMousePos(e);
        const tile = this.screenToTile(mx, my);
        if (tile) {
          cb.onWaypointMoved(this.dragWaypoint.connKey, this.dragWaypoint.wpIdx, tile.tx, tile.ty);
          s.hallwayCacheKey = null; // invalidate
          cb.onRedraw();
        }
        return;
      }

      if (s.connectingFrom) {
        const { mx, my } = this.getMousePos(e);
        s.connectMousePos = { x: mx, y: my };
        cb.onRedraw();
        return;
      }

      if (!s.lastMouse) {
        // Not dragging — update cursor for resize hover
        const { mx: hmx, my: hmy } = this.getMousePos(e);
        const { vpW: hvpW, vpH: hvpH } = this.getVpSize();
        const resHover = this.hitTestResizeCorner(hmx, hmy, hvpW, hvpH);
        this.canvas.style.cursor = resHover ? 'nwse-resize' : 'default';
        return;
      }
      const dx = e.clientX - s.lastMouse.x;
      const dy = e.clientY - s.lastMouse.y;
      if (Math.abs(dx) > 5 || Math.abs(dy) > 5) dragMoved = true;
      s.lastMouse = { x: e.clientX, y: e.clientY };

      if (s.dragRegion !== null) {
        const tz = 4 * s.zoom;
        const md = s.mapData;
        const r = md?.regions?.find((r: any) => r.node_idx === s.dragRegion);
        if (r) {
          const cur = s.regionOverrides[s.dragRegion] || { ox: r.ox, oy: r.oy };
          cur.ox += Math.round(dx / tz);
          cur.oy += Math.round(dy / tz);
          s.regionOverrides[s.dragRegion] = cur;
          cb.onRedraw();
        } else if (typeof s.dragRegion === 'string') {
          const cur = s.regionOverrides[s.dragRegion as any] || { ox: 0, oy: -20 };
          cur.ox += Math.round(dx / tz);
          cur.oy += Math.round(dy / tz);
          s.regionOverrides[s.dragRegion as any] = cur;
          cb.onRedraw();
        }
      } else if (s.dragging) {
        s.panX += dx;
        s.panY += dy;
        cb.onRedraw();
      }
    });

    // Mouse up
    this.on('mouseup', (e: MouseEvent) => {
      if (this.resizing) {
        this.resizing = null;
        dragMoved = true;
        return;
      }

      if (this.dragWaypoint) {
        this.dragWaypoint = null;
        dragMoved = true;
        return;
      }

      if (s.connectingFrom) {
        const { mx, my } = this.getMousePos(e);
        const { vpW, vpH } = this.getVpSize();
        const handle = hitTestHandle(mx, my, vpW, vpH, this.campaign, s);
        if (handle && handle.type === 'entry' && handle.nodeIdx !== s.connectingFrom.nodeIdx) {
          const fromId = this.resolveNodeId(s.connectingFrom.nodeIdx);
          const toId = this.resolveNodeId(handle.nodeIdx);
          cb.onConnectionCreated(fromId, toId);
        }
        s.connectingFrom = null;
        s.connectMousePos = null;
        cb.onRedraw();
        return;
      }

      const wasDragging = s.dragRegion !== null;
      if (wasDragging) cb.onRegionMoved();
      s.dragging = false;
      s.dragRegion = null;
      s.lastMouse = null;

      // Click (not drag) — check for connection or node click to show popup
      if (!dragMoved && e.button === 0) {
        const { mx, my } = this.getMousePos(e);
        const { vpW, vpH } = this.getVpSize();

        // Check connection first — popup at click position
        const ci = this.findConnectionAt(mx, my);
        if (ci !== null) {
          cb.onShowPopup({ x: mx, y: my, type: 'connection', connIdx: ci });
          return;
        }

        // Check node — popup at bottom-center of node
        const nodeIdx = hitTestRegion(mx, my, vpW, vpH, s, this.campaign);
        if (nodeIdx !== null) {
          const anchor = this.getNodeBottomCenter(nodeIdx, vpW, vpH);
          cb.onSelectNode(nodeIdx);
          cb.onShowPopup({ x: anchor.x, y: anchor.y, type: 'node', nodeIdx, nodeW: anchor.w });
        } else {
          cb.onSelectNode(null);
          cb.onShowPopup(null as any); // dismiss
        }
      }
    });

    this.on('mouseleave', () => {
      s.dragging = false;
      s.dragRegion = null;
      s.connectingFrom = null;
      s.connectMousePos = null;
    });

    // Right-click: delete waypoint if clicking one, otherwise prevent default
    this.on('contextmenu', (e: MouseEvent) => {
      e.preventDefault();
      const { mx, my } = this.getMousePos(e);
      const wp = this.hitTestWaypoint(mx, my);
      if (wp) {
        cb.onWaypointDeleted(wp.connKey, wp.wpIdx);
      }
    });

    // Zoom with scroll/pinch
    this.on('wheel', (e: WheelEvent) => {
      e.preventDefault();
      e.stopPropagation();

      const { mx, my } = this.getMousePos(e);
      const { vpW, vpH } = this.getVpSize();

      // Pinch zoom (ctrlKey) vs scroll zoom — normalize to a factor
      let factor: number;
      if (e.ctrlKey) {
        // Pinch gesture — deltaY is already the zoom amount
        factor = 1 - Math.sign(e.deltaY) * Math.min(Math.abs(e.deltaY), 10) * 0.01;
      } else {
        // Scroll wheel
        factor = e.deltaY > 0 ? 0.95 : 1.05;
      }

      const newZoom = Math.max(0.15, Math.min(8, s.zoom * factor));
      if (newZoom === s.zoom) return;
      cb.onShowPopup(null as any); // dismiss popup on zoom

      // Zoom toward cursor: find the world point under the cursor before and after zoom
      const md = s.mapData;
      const gridW = md ? md.width : 60;
      const gridH = md ? md.height : 36;
      const oldTz = 4 * s.zoom;
      const newTz = 4 * newZoom;
      const oldBaseOx = (vpW - gridW * oldTz) / 2 + s.panX;
      const oldBaseOy = (vpH - gridH * oldTz) / 2 + s.panY;
      // World coord under cursor
      const wx = (mx - oldBaseOx) / oldTz;
      const wy = (my - oldBaseOy) / oldTz;
      // New base offset to keep (wx,wy) at (mx,my)
      const newBaseOx = mx - wx * newTz;
      const newBaseOy = my - wy * newTz;
      s.panX = newBaseOx - (vpW - gridW * newTz) / 2;
      s.panY = newBaseOy - (vpH - gridH * newTz) / 2;
      s.zoom = newZoom;
      cb.onRedraw();
    }, { passive: false });
  }

  private hitTestResizeCorner(mx: number, my: number, vpW: number, vpH: number): { nodeId: string | number; originTileX: number; originTileY: number } | null {
    const s = this.state;
    const c = this.campaign;
    const rooms = c.overworld.rooms || c.overworld.fork_chambers || [];
    const tz = 4 * s.zoom;
    const md = s.mapData;
    const gridW = md ? md.width : 60;
    const gridH = md ? md.height : 36;
    const baseOx = (vpW - gridW * tz) / 2 + s.panX;
    const baseOy = (vpH - gridH * tz) / 2 + s.panY;
    const hitR = Math.max(8, tz * 0.8);

    // Check rooms
    for (const room of rooms) {
      const pos = s.regionOverrides[room.id as any] || { ox: 0, oy: -20 };
      const rw = room.w || 10, rh = room.h || 8;
      const fx = baseOx + pos.ox * tz;
      const fy = baseOy + pos.oy * tz;
      if (Math.abs(mx - (fx + rw * tz)) < hitR && Math.abs(my - (fy + rh * tz)) < hitR) {
        return { nodeId: room.id, originTileX: pos.ox, originTileY: pos.oy };
      }
    }
    // Check start room and store
    if (md?.regions) {
      const levels = c.overworld.levels || [];
      for (const r of md.regions) {
        const isStart = r.node_idx === 0;
        const isStore = r.node_idx > levels.length;
        if (!isStart && !isStore) continue;
        const p = s.regionOverrides[r.node_idx] || { ox: r.ox, oy: r.oy };
        const rw = isStart ? (c.overworld.start_room_size?.[0] || r.w) : (c.overworld.store_room_size?.[0] || r.w);
        const rh = isStart ? (c.overworld.start_room_size?.[1] || r.h) : (c.overworld.store_room_size?.[1] || r.h);
        const fx = baseOx + p.ox * tz;
        const fy = baseOy + p.oy * tz;
        if (Math.abs(mx - (fx + rw * tz)) < hitR && Math.abs(my - (fy + rh * tz)) < hitR) {
          return { nodeId: r.node_idx, originTileX: p.ox, originTileY: p.oy };
        }
      }
    }
    return null;
  }

  private hitTestWaypoint(mx: number, my: number): { connKey: string; wpIdx: number } | null {
    const tile = this.screenToTile(mx, my);
    if (!tile) return null;
    const wps = this.campaign.overworld.hallway_waypoints || {};
    for (const [connKey, points] of Object.entries(wps)) {
      for (let i = 0; i < points.length; i++) {
        if (Math.abs(points[i][0] - tile.tx) <= 1 && Math.abs(points[i][1] - tile.ty) <= 1) {
          return { connKey, wpIdx: i };
        }
      }
    }
    return null;
  }

  private getNodeBottomCenter(nodeIdx: number | string, vpW: number, vpH: number): { x: number; y: number; w: number } {
    const s = this.state;
    const md = s.mapData;
    const tz = 4 * s.zoom;
    const gridW = md ? md.width : 60;
    const gridH = md ? md.height : 36;
    const baseOx = (vpW - gridW * tz) / 2 + s.panX;
    const baseOy = (vpH - gridH * tz) / 2 + s.panY;

    if (typeof nodeIdx === 'string') {
      // Room
      const rooms = this.campaign.overworld.rooms || this.campaign.overworld.fork_chambers || [];
      const room = rooms.find(r => r.id === nodeIdx);
      const rw = room?.w || 10, rh = room?.h || 8;
      const pos = s.regionOverrides[nodeIdx as any] || { ox: 0, oy: -20 };
      return { x: baseOx + (pos.ox + rw / 2) * tz, y: baseOy + (pos.oy + rh) * tz, w: rw * tz };
    }
    // Regular region
    const r = md?.regions?.find((r: any) => r.node_idx === nodeIdx);
    if (r) {
      const p = s.regionOverrides[r.node_idx] || { ox: r.ox, oy: r.oy };
      return { x: baseOx + (p.ox + r.w / 2) * tz, y: baseOy + (p.oy + r.h) * tz, w: r.w * tz };
    }
    return { x: vpW / 2, y: vpH / 2, w: 100 };
  }

  updateCampaign(campaign: BundledCampaign) {
    this.campaign = campaign;
  }

  destroy() {
    for (const { event, handler, options } of this.handlers) {
      this.canvas.removeEventListener(event, handler, options);
    }
    this.handlers = [];
  }
}
