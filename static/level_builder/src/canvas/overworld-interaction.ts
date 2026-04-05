// Encapsulates all mouse/keyboard interaction for the overworld canvas.

import type { BundledCampaign } from '../types/pack';
import type { OwCanvasState } from './overworld-renderer';
import { hitTestRegion, hitTestHandle, computeGridBounds } from './overworld-renderer';

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
  onSignpostMoved?: (idx: number, x: number, y: number) => void;
  onToast: (msg: string, type: string) => void;
}

export class OverworldInteraction {
  private canvas: HTMLCanvasElement;
  private campaign: BundledCampaign;
  private state: OwCanvasState;
  private callbacks: OwInteractionCallbacks;
  private handlers: { event: string; handler: any; options?: any }[] = [];
  private dragWaypoint: { connKey: string; wpIdx: number } | null = null;
  private dragSignpost: { idx: number } | null = null;
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

  private getBr() { return this.campaign.overworld.builder_regions || []; }

  private resolveNodeId(nodeIdx: number | string): string {
    if (typeof nodeIdx === 'string') return nodeIdx;
    const levels = this.campaign.overworld.levels || [];
    if (nodeIdx === 0) return 'start';
    if (nodeIdx > levels.length) return 'store';
    return 'level_' + (nodeIdx - 1);
  }

  private resolveNodeIdx(id: string | number): number | null {
    const str = String(id);
    if (str === 'start') return 0;
    const levels = this.campaign.overworld.levels || [];
    if (str === 'store' || str === 'end') return levels.length + 1;
    const m = str.match(/level_(\d+)/);
    return m ? parseInt(m[1]) + 1 : null;
  }

  // Convert screen position to tile coordinate
  private screenToTile(mx: number, my: number): { tx: number; ty: number } | null {
    const s = this.state;
    const br = this.getBr();
    if (br.length === 0) return null;
    const { vpW, vpH } = this.getVpSize();
    const tz = 4 * s.zoom;
    const bounds = computeGridBounds(br);
    const baseOx = (vpW - bounds.width * tz) / 2 + s.panX;
    const baseOy = (vpH - bounds.height * tz) / 2 + s.panY;
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
        const bounds = computeGridBounds(this.getBr());
        const gridW = bounds.width;
        const gridH = bounds.height;
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

      // Check signpost drag (before region hit test to prevent room dragging)
      const signIdx = this.hitTestSignpost(mx, my);
      if (signIdx >= 0) {
        this.dragSignpost = { idx: signIdx };
        s.dragRegion = null; // prevent region drag
        s.dragging = false;
        s.lastMouse = { x: e.clientX, y: e.clientY };
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
        cb.onShowPopup(null as any);
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

      if (this.dragSignpost) {
        const { mx, my } = this.getMousePos(e);
        const tile = this.screenToTile(mx, my);
        if (tile) {
          dragMoved = true;
          const placed = this.campaign.overworld.placed_signposts;
          if (placed?.[this.dragSignpost.idx]) {
            placed[this.dragSignpost.idx].x = tile.tx;
            placed[this.dragSignpost.idx].y = tile.ty;
            cb.onRedraw();
          }
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
        dragMoved = true;
        cb.onShowPopup(null as any); // dismiss popup while dragging
        const tz = 4 * s.zoom;
        // Update builder_region position
        const br = this.campaign.overworld.builder_regions;
        if (br) {
          // Convert node_idx to builder_region ID
          const dragVal = s.dragRegion;
          let dragId: string;
          if (typeof dragVal === 'string') {
            dragId = dragVal; // room ID
          } else if (dragVal === 0) {
            dragId = 'start';
          } else {
            const levels = this.campaign.overworld.levels || [];
            dragId = dragVal > levels.length ? 'store' : `level_${dragVal - 1}`;
          }
          const region = br.find(r => r.id === dragId);
          if (region) {
            region.ox += dx / tz;
            region.oy += dy / tz;
            // Snap to integer tiles for rendering
            region.ox = Math.round(region.ox);
            region.oy = Math.round(region.oy);
          }
        }
        s.hallwayCacheKey = null;
        cb.onRedraw();
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

      if (this.dragSignpost) {
        const placed = this.campaign.overworld.placed_signposts;
        if (placed?.[this.dragSignpost.idx] && cb.onSignpostMoved) {
          const ps = placed[this.dragSignpost.idx];
          cb.onSignpostMoved(this.dragSignpost.idx, ps.x, ps.y);
        }
        this.dragSignpost = null;
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
      const bounds = computeGridBounds(this.getBr());
      const gridW = bounds.width;
      const gridH = bounds.height;
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
    const br = this.getBr();
    const levels = this.campaign.overworld.levels || [];
    const tz = 4 * s.zoom;
    const bounds = computeGridBounds(br);
    const baseOx = (vpW - bounds.width * tz) / 2 + s.panX;
    const baseOy = (vpH - bounds.height * tz) / 2 + s.panY;
    const hitR = Math.max(8, tz * 0.8);

    for (const region of br) {
      if (region.type === 'level') continue; // only rooms, start, store are resizable
      const fx = baseOx + region.ox * tz;
      const fy = baseOy + region.oy * tz;
      if (Math.abs(mx - (fx + region.w * tz)) < hitR && Math.abs(my - (fy + region.h * tz)) < hitR) {
        const nodeId = region.type === 'start' ? 0
          : region.type === 'store' ? levels.length + 1
          : region.id;
        return { nodeId, originTileX: region.ox, originTileY: region.oy };
      }
    }
    return null;
  }

  private hitTestSignpost(mx: number, my: number): number {
    const tile = this.screenToTile(mx, my);
    if (!tile) return -1;
    const placed = this.campaign.overworld.placed_signposts || [];
    return placed.findIndex(p => Math.abs(p.x - tile.tx) <= 1 && Math.abs(p.y - tile.ty) <= 1);
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
    const tz = 4 * s.zoom;
    const br = this.getBr();
    const levels = this.campaign.overworld.levels || [];
    const bounds = computeGridBounds(br);
    const baseOx = (vpW - bounds.width * tz) / 2 + s.panX;
    const baseOy = (vpH - bounds.height * tz) / 2 + s.panY;

    let dragId: string;
    if (typeof nodeIdx === 'string') {
      dragId = nodeIdx;
    } else if (nodeIdx === 0) {
      dragId = 'start';
    } else {
      dragId = nodeIdx > levels.length ? 'store' : `level_${nodeIdx - 1}`;
    }
    const region = br.find(r => r.id === dragId);
    if (region) {
      return {
        x: baseOx + (region.ox + region.w / 2) * tz,
        y: baseOy + (region.oy + region.h) * tz,
        w: region.w * tz,
      };
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
