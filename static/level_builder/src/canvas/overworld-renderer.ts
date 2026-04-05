// Pure overworld canvas rendering — no DOM, no Preact, no side effects.
// Takes data in, draws to canvas context.

import type { BundledCampaign } from '../types/pack';

import { astarHallway, findEdgeTileFromDesign } from './pathfinding';
import { computeRoomHandles, type RoomHandle } from './room-handles';
import { exportOverworldMap, type ExportedOverworldMap } from './overworld-export';

// Compute door position (local to region) for a region.
// Uses: placed_entities > backend entry/exit_pos > auto-computed edge positions.
function getDoorPos(
  r: any,
  type: 'entry' | 'exit',
  designs: any[],
): [number, number] | null {
  // Get design index — handle both numeric node_idx and string IDs
  let di = -1;
  if (r._builderRegion?.level_idx != null) {
    di = r._builderRegion.level_idx;
  } else if (typeof r.node_idx === 'number') {
    di = r.node_idx - 1;
  } else if (typeof r.node_idx === 'string') {
    const m = r.node_idx.match(/^level_(\d+)$/);
    if (m) di = parseInt(m[1]);
  }
  const design = di >= 0 ? designs[di] : null;
  const pe = design?.placed_entities;

  // 1. Placed entity door
  if (type === 'exit' && pe?.exit_door) return pe.exit_door;
  if (type === 'entry' && pe?.entry_door) return pe.entry_door;

  // 2. Backend-provided position (convert from absolute to local)
  if (type === 'exit' && r.exit_pos) return [r.exit_pos[0] - r.ox, r.exit_pos[1] - r.oy];
  if (type === 'entry' && r.entry_pos) return [r.entry_pos[0] - r.ox, r.entry_pos[1] - r.oy];

  // 3. Auto-compute for regions without doors (store, start, fork chambers)
  // Exit: right edge center, Entry: left edge center
  if (type === 'exit') return [r.w - 1, Math.floor(r.h / 2)];
  if (type === 'entry') return [0, Math.floor(r.h / 2)];

  return null;
}

export interface OwCanvasState {
  zoom: number;
  panX: number;
  panY: number;
  connectingFrom: { nodeIdx: number | string; sx: number; sy: number } | null;
  connectMousePos: { x: number; y: number } | null;
  dragging: boolean;
  dragRegion: number | null;
  lastMouse: { x: number; y: number } | null;
  // Cached hallway paths — invalidated when connections or region positions change
  hallwayCache: Map<string, number[][]> | null;
  hallwayCacheKey: string | null;
  // Cached export grid — rebuilt when hallway cache is rebuilt
  exportedMap: ExportedOverworldMap | null;
}

export function createOwCanvasState(): OwCanvasState {
  return {
    zoom: 1,
    panX: 0,
    panY: 0,
    connectingFrom: null,
    connectMousePos: null,
    dragging: false,
    dragRegion: null,
    lastMouse: null,
    hallwayCache: null,
    hallwayCacheKey: null,
    exportedMap: null,
  };
}

// Compute grid bounds from builder_regions
export function computeGridBounds(br: { ox: number; oy: number; w: number; h: number }[]): { width: number; height: number } {
  if (br.length === 0) return { width: 60, height: 36 };
  let maxX = 0, maxY = 0;
  for (const r of br) {
    maxX = Math.max(maxX, r.ox + r.w + 20);
    maxY = Math.max(maxY, r.oy + r.h + 20);
  }
  return { width: maxX, height: maxY };
}

export function drawOverworld(
  ctx: CanvasRenderingContext2D,
  vpW: number,
  vpH: number,
  campaign: BundledCampaign,
  state: OwCanvasState,
  selectedNode: number | string | null,
) {
  const levels = campaign.overworld.levels || [];
  const designs = campaign.designs || [];
  const builderRegions = campaign.overworld.builder_regions || [];

  // Build shim region list for label/selection rendering
  const md = {
    regions: builderRegions.map(br => ({
      node_idx: br.type === 'start' ? 0
        : br.type === 'store' ? levels.length + 1
        : br.type === 'level' ? (br.level_idx ?? 0) + 1
        : br.id, // rooms keep string id
      ox: br.ox, oy: br.oy, w: br.w, h: br.h,
      entry_pos: null, exit_pos: null,
      _builderRegion: br,
    })),
  };

  const TILE = 4;
  const zoom = state.zoom;
  const tz = TILE * zoom;

  const bounds = computeGridBounds(builderRegions);
  const gridW = bounds.width;
  const gridH = bounds.height;
  const mapW = gridW * tz;
  const mapH = gridH * tz;
  const baseOx = (vpW - mapW) / 2 + state.panX;
  const baseOy = (vpH - mapH) / 2 + state.panY;

  // Background
  const bgColor = campaign.overworld.bg_color || '#0d0d1e';
  ctx.fillStyle = bgColor;
  ctx.fillRect(0, 0, vpW, vpH);

  const drawTile = (wx: number, wy: number, color: string, image?: string) => {
    const gap = Math.max(0.5, tz * 0.08);
    const px = baseOx + wx * tz + gap / 2;
    const py = baseOy + wy * tz + gap / 2;
    const sz = tz - gap;
    if (image && sz >= 3) {
      const img = owTileImageCache.get(image);
      if (img?.complete && img.naturalWidth > 0) {
        ctx.drawImage(img, px, py, sz, sz);
        return;
      }
      if (!img) {
        const newImg = new Image();
        newImg.src = `data:image/png;base64,${image}`;
        owTileImageCache.set(image, newImg);
      }
    }
    ctx.fillStyle = color;
    ctx.fillRect(px, py, sz, sz);
  };

  if (builderRegions.length === 0) return;

  const connections = campaign.overworld.connections || [];

  // Resolve any node ID to a region object from builder_regions
  const resolveRegion = (id: string | number): any | null => {
    const s = String(id);
    const br2 = builderRegions.find(r => r.id === s);
    if (br2) {
      return { node_idx: s, ox: br2.ox, oy: br2.oy, w: br2.w, h: br2.h, _isRoom: br2.type === 'room', _builderRegion: br2 };
    }
    if (s === 'end') {
      const store = builderRegions.find(r => r.type === 'store');
      if (store) return { node_idx: store.id, ox: store.ox, oy: store.oy, w: store.w, h: store.h, _builderRegion: store };
    }
    return null;
  };

  // Build cache key from connections + region positions
  const cacheKeyParts: string[] = [];
  for (const [a, b] of connections) cacheKeyParts.push(a + '>' + b);
  for (const br of builderRegions) cacheKeyParts.push(br.id + ':' + br.ox + ',' + br.oy);
  // Resolve any node ID to its center tile position
  const resolveCenter = (id: string): { ox: number; oy: number } | null => {
    const reg = resolveRegion(id);
    if (!reg) return null;
    return { ox: reg.ox + reg.w / 2, oy: reg.oy + reg.h / 2 };
  };
  // Pre-computed handles for all non-level regions (rooms, store, start)
  const roomHandlesMap = new Map<string, RoomHandle[]>();
  for (const br of builderRegions) {
    if (br.type === 'level') continue;
    const fakeRoom = { id: br.id, name: '', w: br.w, h: br.h };
    const rp = { ox: br.ox, oy: br.oy };
    roomHandlesMap.set(br.id, computeRoomHandles(fakeRoom, rp, connections, resolveCenter));
  }

  const cacheKey = cacheKeyParts.join('|');
  if (state.hallwayCacheKey !== cacheKey) {
    state.hallwayCache = new Map();
    state.hallwayCacheKey = cacheKey;

    // Build occupied set from builder_regions
    const occupied = new Set<string>();
    for (const br of builderRegions) {
      for (let y = 0; y < br.h; y++)
        for (let x = 0; x < br.w; x++)
          occupied.add((br.ox + x) + ',' + (br.oy + y));
    }

    // Helper: get door position for connection endpoint
    const getDoorForConn = (region: any, connKey: string, type: 'exit' | 'entry'): number[] | null => {
      const brType = region._builderRegion?.type;
      if (region._isRoom || brType === 'room' || brType === 'store' || brType === 'start') {
        const rid = region._builderRegion?.id || region._room?.id || region.node_idx;
        const handles = roomHandlesMap.get(String(rid));
        const h = handles?.find(h => h.connKey === connKey && h.type === type);
        if (h) return [h.lx, h.ly];
        const free = handles?.find(h => h.connKey === null && h.type === type);
        if (free) return [free.lx, free.ly];
        // Fallback: edge center
        return [type === 'exit' ? region.w - 1 : 0, Math.floor(region.h / 2)];
      }
      return getDoorPos(region, type, designs);
    };

    for (const [a, b] of connections) {
      const ra = resolveRegion(a), rb = resolveRegion(b);
      if (!ra || !rb) continue;
      const raP = { ox: ra.ox, oy: ra.oy };
      const rbP = { ox: rb.ox, oy: rb.oy };
      const connKey = `${a}->${b}`;
      const exitDoor = getDoorForConn(ra, connKey, 'exit');
      const entryDoor = getDoorForConn(rb, connKey, 'entry');
      if (!exitDoor || !entryDoor) continue;
      let sx = raP.ox + exitDoor[0], sy = raP.oy + exitDoor[1];
      let ex2 = rbP.ox + entryDoor[0], ey2 = rbP.oy + entryDoor[1];
      // Push start/end outward from large region edges so A* doesn't cut through
      // Skip push for rooms (small, hallways should connect flush)
      if (!ra._isRoom) {
        if (exitDoor[0] >= (ra.w || 60) - 2) sx += 2;
        else if (exitDoor[0] <= 1) sx -= 2;
        if (exitDoor[1] >= (ra.h || 36) - 2) sy += 2;
        else if (exitDoor[1] <= 1) sy -= 2;
      }
      if (!rb._isRoom) {
        if (entryDoor[0] <= 1) ex2 -= 2;
        else if (entryDoor[0] >= (rb.w || 60) - 2) ex2 += 2;
        if (entryDoor[1] <= 1) ey2 -= 2;
        else if (entryDoor[1] >= (rb.h || 36) - 2) ey2 += 2;
      }
      try {
        // Route through waypoints if any
        const waypoints = campaign.overworld.hallway_waypoints?.[connKey] || [];
        const segments: number[][] = [];
        let curX = sx, curY = sy;
        for (const [wx, wy] of waypoints) {
          const seg = astarHallway(curX, curY, wx, wy, occupied);
          if (seg.length > 0) { segments.push(...seg); curX = wx; curY = wy; }
        }
        const lastSeg = astarHallway(curX, curY, ex2, ey2, occupied);
        if (lastSeg.length > 0) segments.push(...lastSeg);
        // Extend path to actual door positions (bridge the push gap)
        const actualSx = raP.ox + exitDoor[0], actualSy = raP.oy + exitDoor[1];
        const actualEx = rbP.ox + entryDoor[0], actualEy = rbP.oy + entryDoor[1];
        if (segments.length > 0) {
          // Prepend: straight line from actual exit door to first path tile
          const [firstX, firstY] = segments[0];
          const bridgeStart: number[][] = [];
          let bx = actualSx, by = actualSy;
          while (bx !== firstX || by !== firstY) {
            bridgeStart.push([bx, by]);
            if (bx !== firstX) bx += bx < firstX ? 1 : -1;
            else by += by < firstY ? 1 : -1;
          }
          segments.unshift(...bridgeStart);
          // Append: straight line from last path tile to actual entry door
          const [lastX, lastY] = segments[segments.length - 1];
          let ex = lastX, ey = lastY;
          while (ex !== actualEx || ey !== actualEy) {
            if (ex !== actualEx) ex += ex < actualEx ? 1 : -1;
            else ey += ey < actualEy ? 1 : -1;
            segments.push([ex, ey]);
          }
        }
        if (segments.length > 0) {
          state.hallwayCache.set(connKey, segments);
          for (const [px, py] of segments) occupied.add(px + ',' + py);
        }
      } catch (e) { console.error('A* failed for', a, '->', b, e); }
    }
  }

  // Build exported map (single source of truth for tile layout) and render it
  state.exportedMap = exportOverworldMap(campaign, state);
  if (state.exportedMap) {
    const em = state.exportedMap;
    // Recover world-to-export offset from first region
    const firstBr = builderRegions[0];
    const firstExReg = em.regions.find(r => r.node_id === firstBr.id);
    const offsetX = firstExReg ? firstBr.ox - firstExReg.ox : 0;
    const offsetY = firstExReg ? firstBr.oy - firstExReg.oy : 0;
    for (let ey = 0; ey < em.height; ey++) {
      for (let ex = 0; ex < em.width; ex++) {
        const tileId = em.tiles[ey][ex];
        if (tileId === 'void') continue;
        const def = em.tile_defs[tileId];
        if (!def) continue;
        drawTile(ex + offsetX, ey + offsetY, def.color, def.image);
      }
    }
  }

  // Draw door handles — hot-dog shapes along the wall edge, hide if connected
  // Build sets of connected exits/entries
  const connectedExits = new Set<string>(); // node IDs that have an outgoing connection
  const connectedEntries = new Set<string>();
  const storeReg = md.regions.find((r: any) => r.node_idx > levels.length);
  const nodeIdFor = (ni: number) => ni === 0 ? 'start' : ni === storeReg?.node_idx ? 'store' : `level_${ni - 1}`;
  for (const [a, b] of connections) {
    connectedExits.add(a);
    connectedEntries.add(b);
  }

  const drawHandle = (hx: number, hy: number, isVertical: boolean, color: string) => {
    const hw = isVertical ? Math.max(2, tz * 0.3) : Math.max(4, tz * 1.2);
    const hh = isVertical ? Math.max(4, tz * 1.2) : Math.max(2, tz * 0.3);
    const radius = Math.min(hw, hh) / 2;
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.roundRect(hx - hw / 2, hy - hh / 2, hw, hh, radius);
    ctx.fill();
    ctx.strokeStyle = '#fff'; ctx.lineWidth = 0.5; ctx.stroke();
  };

  for (const r of md.regions) {
    const nid = typeof r.node_idx === 'string' ? r.node_idx : nodeIdFor(r.node_idx);
    const rox = r.ox, roy = r.oy;
    // Exit handle — only show if NOT connected
    if (!connectedExits.has(nid)) {
      const exitDoor = getDoorPos(r, 'exit', designs);
      if (exitDoor) {
        const hx = baseOx + (rox + exitDoor[0]) * tz + tz / 2;
        const hy = baseOy + (roy + exitDoor[1]) * tz + tz / 2;
        const isVertical = exitDoor[0] === 0 || exitDoor[0] >= (r.w || 60) - 1;
        drawHandle(hx, hy, isVertical, '#ff4444');
      }
    }
    // Entry handle — only show if NOT connected
    if (!connectedEntries.has(nid)) {
      const entryDoor = getDoorPos(r, 'entry', designs);
      if (entryDoor) {
        const hx = baseOx + (rox + entryDoor[0]) * tz + tz / 2;
        const hy = baseOy + (roy + entryDoor[1]) * tz + tz / 2;
        const isVertical = entryDoor[0] === 0 || entryDoor[0] >= (r.w || 60) - 1;
        drawHandle(hx, hy, isVertical, '#44ccff');
      }
    }
  }

  // Hallway waypoints (draggable yellow diamonds)
  const allWaypoints = campaign.overworld.hallway_waypoints || {};
  for (const [connKey, wps] of Object.entries(allWaypoints)) {
    for (const [wx, wy] of wps) {
      const sx = baseOx + wx * tz + tz / 2;
      const sy = baseOy + wy * tz + tz / 2;
      const sz = Math.max(4, tz * 0.6);
      ctx.fillStyle = '#ffcc00';
      ctx.beginPath();
      ctx.moveTo(sx, sy - sz); ctx.lineTo(sx + sz, sy); ctx.lineTo(sx, sy + sz); ctx.lineTo(sx - sz, sy);
      ctx.closePath(); ctx.fill();
      ctx.strokeStyle = '#000'; ctx.lineWidth = 1; ctx.stroke();
    }
  }

  // One-way hallways: yellow outline around the hallway shape
  const oneWaySet = new Set(campaign.overworld.one_way_connections || []);
  if (oneWaySet.size > 0 && state.hallwayCache) {
    ctx.strokeStyle = '#ffcc00';
    ctx.lineWidth = Math.max(1.5, tz * 0.2);
    ctx.globalAlpha = 0.8;
    for (const [a, b] of connections) {
      const key = `${a}->${b}`;
      if (!oneWaySet.has(key)) continue;
      const path = state.hallwayCache.get(key);
      if (!path || path.length === 0) continue;
      // Build the set of floor tiles for this hallway (same as corridor builder)
      const floorTiles = new Set<string>();
      for (const [px, py] of path) floorTiles.add(px + ',' + py);
      // Trace the outer edges: for each floor tile, draw edges where neighbors are NOT floor
      ctx.beginPath();
      for (const key of floorTiles) {
        const [tx, ty] = key.split(',').map(Number);
        const x = baseOx + tx * tz, y = baseOy + ty * tz;
        if (!floorTiles.has((tx) + ',' + (ty - 1))) { ctx.moveTo(x, y); ctx.lineTo(x + tz, y); }           // top
        if (!floorTiles.has((tx) + ',' + (ty + 1))) { ctx.moveTo(x, y + tz); ctx.lineTo(x + tz, y + tz); } // bottom
        if (!floorTiles.has((tx - 1) + ',' + (ty))) { ctx.moveTo(x, y); ctx.lineTo(x, y + tz); }           // left
        if (!floorTiles.has((tx + 1) + ',' + (ty))) { ctx.moveTo(x + tz, y); ctx.lineTo(x + tz, y + tz); } // right
      }
      ctx.stroke();
    }
    ctx.globalAlpha = 1;
  }

  // Connection drag line
  if (state.connectingFrom && state.connectMousePos) {
    ctx.strokeStyle = '#ffcc00';
    ctx.lineWidth = 2;
    ctx.setLineDash([4, 4]);
    ctx.beginPath();
    ctx.moveTo(state.connectingFrom.sx, state.connectingFrom.sy);
    ctx.lineTo(state.connectMousePos.x, state.connectMousePos.y);
    ctx.stroke();
    ctx.setLineDash([]);
  }

  // Placed signposts
  const placedSigns = campaign.overworld.placed_signposts || [];
  const signDefs = campaign.signposts || [];
  const signSprite = (globalThis as any).__packItemSprites?.['sign'] as string | undefined;
  for (const ps of placedSigns) {
    const def = signDefs[ps.signpost_idx];
    if (!def) continue;
    const px = baseOx + ps.x * tz;
    const py = baseOy + ps.y * tz;
    if (signSprite) {
      const img = owTileImageCache.get('__sign_sprite__');
      if (img?.complete && img.naturalWidth > 0) {
        ctx.imageSmoothingEnabled = false;
        ctx.drawImage(img, px, py, tz, tz);
        ctx.imageSmoothingEnabled = true;
      } else if (!img) {
        const newImg = new Image();
        newImg.src = `data:image/png;base64,${signSprite}`;
        owTileImageCache.set('__sign_sprite__', newImg);
      }
    } else {
      const cx = px + tz / 2;
      const cy = py + tz / 2;
      const r = Math.max(2, tz * 0.4);
      ctx.fillStyle = '#88cc44';
      ctx.beginPath();
      ctx.arc(cx, cy, r, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = '#fff';
      ctx.lineWidth = Math.max(0.5, tz * 0.1);
      ctx.stroke();
    }
    if (tz >= 6) {
      ctx.fillStyle = '#fff';
      ctx.font = `${Math.max(6, tz * 0.7)}px system-ui`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'top';
      ctx.fillText(def.title, px + tz / 2, py + tz + 2);
    }
  }

  // Labels and selection highlights
  for (const r of md.regions) {
    const levelsIdx = typeof r.node_idx === 'number' ? r.node_idx - 1 : -1;
    // Use position directly (builder_regions is the source of truth)
    const rox = r.ox;
    const roy = r.oy;

    // Labels (skip start room and generic rooms)
    const brType = (r as any)._builderRegion?.type;
    if (r.node_idx !== 0 && brType !== 'room') {
      const lv = levels[levelsIdx] || { name: levelsIdx >= levels.length ? 'Store' : `Node ${r.node_idx}` };
      ctx.fillStyle = '#fff';
      ctx.font = `bold ${Math.max(10, 12 * zoom)}px system-ui`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      const label = levelsIdx < levels.length
        ? `${(lv as any).name || `Level ${levelsIdx + 1}`} (${(lv as any).budget || 0})`
        : ((lv as any).name || 'Store');
      ctx.fillText(label, baseOx + (rox + r.w / 2) * tz, baseOy + (roy + r.h / 2) * tz);
    }

    // Selection + resize handles
    if (selectedNode === r.node_idx) {
      ctx.strokeStyle = '#e94560';
      ctx.lineWidth = 2;
      const rx = baseOx + rox * tz, ry2 = baseOy + roy * tz;
      const rfw = r.w * tz, rfh = r.h * tz;
      ctx.strokeRect(rx - 2, ry2 - 2, rfw + 4, rfh + 4);
      const brType2 = (r as any)._builderRegion?.type;
      if (brType2 === 'start' || brType2 === 'store' || brType2 === 'room') {
        const hs = Math.max(5, tz * 0.5);
        ctx.fillStyle = '#fff';
        ctx.fillRect(rx + rfw - hs, ry2 + rfh - hs, hs * 2, hs * 2);
      }
    }
  }

  // Title text in start room
  const titleRegion = md.regions.find(r => r.node_idx === 0);
  if (titleRegion) {
    const tc = campaign.overworld.text_color || '#e8e8e8';
    const trCx = baseOx + (titleRegion.ox + titleRegion.w / 2) * tz;
    const trCy = baseOy + (titleRegion.oy + titleRegion.h / 2) * tz;

    const tfs = Math.max(6, Math.min(20, titleRegion.w * tz * 0.06));
    ctx.fillStyle = tc;
    ctx.font = `bold ${tfs}px ${campaign.overworld.font || 'system-ui'}`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(campaign.overworld.name || '', trCx, trCy - tfs * 0.8);

    if (campaign.overworld.description) {
      const dfs = Math.max(4, tfs * 0.45);
      ctx.font = `${dfs}px ${campaign.overworld.description_font || 'system-ui'}`;
      ctx.globalAlpha = 0.7;
      const maxW = titleRegion.w * tz * 0.85;
      const words = campaign.overworld.description.split(/\s+/);
      const lines: string[] = [];
      let cur = '';
      for (const w of words) {
        const test = cur ? cur + ' ' + w : w;
        if (ctx.measureText(test).width > maxW && cur) { lines.push(cur); cur = w; }
        else cur = test;
      }
      if (cur) lines.push(cur);
      const lineH = dfs * 1.4;
      let dy = trCy + tfs * 0.3;
      for (const line of lines) {
        ctx.fillText(line, trCx, dy);
        dy += lineH;
      }
      ctx.globalAlpha = 1;
    }
  }

  // ── Room/store/start handles (tiles already drawn above) ──
  for (const br of builderRegions) {
    if (br.type === 'level') continue;
    const fx = baseOx + br.ox * tz;
    const fy = baseOy + br.oy * tz;
    const fw = br.w * tz, fh = br.h * tz;
    const nodeId = br.type === 'start' ? 0
      : br.type === 'store' ? levels.length + 1
      : br.id;

    // Door handles — hot dogs, only show unconnected, hidden when selected
    if (selectedNode !== nodeId) {
      const roomHandles = roomHandlesMap.get(br.id);
      if (roomHandles) {
        for (const h of roomHandles) {
          if (h.connKey) continue; // hide connected handles
          const hx = fx + h.lx * tz + tz / 2;
          const hy = fy + h.ly * tz + tz / 2;
          const isVertical = h.lx === 0 || h.lx === br.w - 1;
          ctx.globalAlpha = 0.5;
          drawHandle(hx, hy, isVertical, h.type === 'entry' ? '#44ccff' : '#ff4444');
          ctx.globalAlpha = 1;
        }
      }
    }
  }
}

// Overworld tile image cache (keyed by base64 — shared across frames)
const owTileImageCache = new Map<string, HTMLImageElement>();


// Hit test: find which region (by node_idx or fork id) the mouse is over
export function hitTestRegion(
  mx: number, my: number,
  vpW: number, vpH: number,
  state: OwCanvasState,
  campaign?: BundledCampaign,
): number | string | null {
  const br = campaign?.overworld.builder_regions || [];
  if (br.length === 0) return null;
  const levels = campaign?.overworld.levels || [];
  const bounds = computeGridBounds(br);
  const tz = 4 * state.zoom;
  const baseOx = (vpW - bounds.width * tz) / 2 + state.panX;
  const baseOy = (vpH - bounds.height * tz) / 2 + state.panY;

  for (const region of br) {
    const rx = baseOx + region.ox * tz;
    const ry = baseOy + region.oy * tz;
    if (mx >= rx && mx <= rx + region.w * tz && my >= ry && my <= ry + region.h * tz) {
      if (region.type === 'start') return 0;
      if (region.type === 'store') return levels.length + 1;
      if (region.type === 'level') return (region.level_idx ?? 0) + 1;
      return region.id; // room string ID
    }
  }
  return null;
}

// Hit test door handles (exit/entry circles)
export function hitTestHandle(
  mx: number, my: number,
  vpW: number, vpH: number,
  campaign: BundledCampaign,
  state: OwCanvasState,
): { type: 'exit' | 'entry'; nodeIdx: number | string; sx: number; sy: number } | null {
  const br = campaign.overworld.builder_regions || [];
  if (br.length === 0) return null;
  const designs = campaign.designs || [];
  const levels = campaign.overworld.levels || [];
  const tz = 4 * state.zoom;
  const bounds = computeGridBounds(br);
  const baseOx = (vpW - bounds.width * tz) / 2 + state.panX;
  const baseOy = (vpH - bounds.height * tz) / 2 + state.panY;
  const hitW = Math.max(6, tz * 1.5);

  for (const region of br) {
    const rox = region.ox, roy = region.oy;
    const fakeR = { node_idx: region.type === 'start' ? 0 : region.type === 'store' ? levels.length + 1 : region.type === 'level' ? (region.level_idx ?? 0) + 1 : region.id, ox: rox, oy: roy, w: region.w, h: region.h, _builderRegion: region };
    const exitDoor = getDoorPos(fakeR, 'exit', designs);
    const entryDoor = getDoorPos(fakeR, 'entry', designs);
    const nodeIdx = fakeR.node_idx;

    if (exitDoor) {
      const hx = baseOx + (rox + exitDoor[0]) * tz + tz / 2;
      const hy = baseOy + (roy + exitDoor[1]) * tz + tz / 2;
      if (Math.abs(mx - hx) < hitW && Math.abs(my - hy) < hitW) {
        return { type: 'exit', nodeIdx, sx: hx, sy: hy };
      }
    }
    if (entryDoor) {
      const hx = baseOx + (rox + entryDoor[0]) * tz + tz / 2;
      const hy = baseOy + (roy + entryDoor[1]) * tz + tz / 2;
      if (Math.abs(mx - hx) < hitW && Math.abs(my - hy) < hitW) {
        return { type: 'entry', nodeIdx, sx: hx, sy: hy };
      }
    }
  }
  return null;
}
