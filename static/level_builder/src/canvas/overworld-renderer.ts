// Pure overworld canvas rendering — no DOM, no Preact, no side effects.
// Takes data in, draws to canvas context.

import type { BundledCampaign, OverworldMapPreview } from '../types/pack';
import { lerpColor } from './image-utils';
import { astarHallway, findEdgeTileFromDesign } from './pathfinding';
import { computeRoomHandles, type RoomHandle } from './room-handles';

// Compute door position (local to region) for a region.
// Uses: placed_entities > backend entry/exit_pos > auto-computed edge positions.
function getDoorPos(
  r: any,
  type: 'entry' | 'exit',
  designs: any[],
): [number, number] | null {
  const di = r.node_idx - 1;
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
  regionOverrides: Record<number, { ox: number; oy: number }>;
  connectingFrom: { nodeIdx: number | string; sx: number; sy: number } | null;
  connectMousePos: { x: number; y: number } | null;
  mapData: OverworldMapPreview | null;
  mapCampaignId: string | null;
  dragging: boolean;
  dragRegion: number | null;
  lastMouse: { x: number; y: number } | null;
  // Cached hallway paths — invalidated when connections or region positions change
  hallwayCache: Map<string, number[][]> | null;
  hallwayCacheKey: string | null;
}

export function createOwCanvasState(): OwCanvasState {
  return {
    zoom: 1,
    panX: 0,
    panY: 0,
    regionOverrides: {},
    connectingFrom: null,
    connectMousePos: null,
    mapData: null,
    mapCampaignId: null,
    dragging: false,
    dragRegion: null,
    lastMouse: null,
    hallwayCache: null,
    hallwayCacheKey: null,
  };
}

export function drawOverworld(
  ctx: CanvasRenderingContext2D,
  vpW: number,
  vpH: number,
  campaign: BundledCampaign,
  state: OwCanvasState,
  selectedNode: number | string | null,
) {
  const md = state.mapData;
  const levels = campaign.overworld.levels || [];
  const designs = campaign.designs || [];

  const TILE = 4;
  const zoom = state.zoom;
  const tz = TILE * zoom;

  const gridW = md ? md.width : 60;
  const gridH = md ? md.height : 36;
  const mapW = gridW * tz;
  const mapH = gridH * tz;
  const baseOx = (vpW - mapW) / 2 + state.panX;
  const baseOy = (vpH - mapH) / 2 + state.panY;

  // Background
  const bgColor = campaign.overworld.bg_color || '#0d0d1e';
  ctx.fillStyle = bgColor;
  ctx.fillRect(0, 0, vpW, vpH);

  const drawTile = (wx: number, wy: number, color: string) => {
    const gap = Math.max(0.5, tz * 0.08);
    ctx.fillStyle = color;
    ctx.fillRect(baseOx + wx * tz + gap / 2, baseOy + wy * tz + gap / 2, tz - gap, tz - gap);
  };

  const regionPos = (r: any) => {
    const o = state.regionOverrides[r.node_idx];
    return o || { ox: r.ox, oy: r.oy };
  };

  if (!md?.regions) return;

  // Draw each region's tiles
  for (const r of md.regions) {
    const { ox: rox, oy: roy } = regionPos(r);
    const di = r.node_idx - 1;
    const design = di >= 0 ? designs[di] : null;

    if (design?.prebuilt_map?.tiles) {
      const tiles = design.prebuilt_map.tiles;
      const defs = design.tile_defs || [];
      const pal = levels[di]?.palette || ['#444'];
      const defMap: Record<string, string> = {};
      for (let i = 0; i < defs.length; i++) defMap[defs[i].name] = pal[i % pal.length] || '#444';
      const pe = design.placed_entities ?? {} as any;
      const doorTiles = new Set<string>();
      if (pe.exit_door) {
        for (let dy = -1; dy <= 1; dy++) doorTiles.add(pe.exit_door[0] + ',' + (pe.exit_door[1] + dy));
      }
      if (pe.entry_door) {
        for (let dy = -1; dy <= 1; dy++) doorTiles.add(pe.entry_door[0] + ',' + (pe.entry_door[1] + dy));
      }
      const floorColor = pal[Math.min(1, pal.length - 1)] || '#333';
      for (let y = 0; y < tiles.length; y++) {
        for (let x = 0; x < tiles[y].length; x++) {
          if (doorTiles.has(x + ',' + y)) {
            drawTile(rox + x, roy + y, floorColor);
          } else {
            drawTile(rox + x, roy + y, defMap[tiles[y][x]] || '#333');
          }
        }
      }
    } else if (md.tiles) {
      // Start/store — use overridden size if set
      const isStart = r.node_idx === 0;
      const storeR = md.regions.find((rr: any) => rr.node_idx > levels.length);
      const isStore = r.node_idx === storeR?.node_idx;
      const effW = isStart ? (campaign.overworld.start_room_size?.[0] || r.w) : isStore ? (campaign.overworld.store_room_size?.[0] || r.w) : r.w;
      const effH = isStart ? (campaign.overworld.start_room_size?.[1] || r.h) : isStore ? (campaign.overworld.store_room_size?.[1] || r.h) : r.h;

      // Get wall/floor colors — from tile_source, backend tile defs, or fallback
      const tileSource = isStart ? campaign.overworld.start_tile_source : isStore ? campaign.overworld.store_tile_source : undefined;
      let wallCol: string, floorCol: string;
      if (tileSource) {
        const m2 = tileSource.match(/^level_(\d+)$/);
        const lv = m2 ? levels[parseInt(m2[1])] : null;
        wallCol = lv?.palette?.[0] || '#444';
        floorCol = lv?.palette?.[1] || '#555';
      } else {
        // Sample colors from the backend tiles
        const sampleWall = Object.values(md.tile_defs).find((d: any) => !d.walkable);
        const sampleFloor = Object.values(md.tile_defs).find((d: any) => d.walkable);
        wallCol = isStart ? ((sampleWall as any)?.color || '#3a2a1a') : '#5d4e37';
        floorCol = isStart ? ((sampleFloor as any)?.color || '#4a3a2a') : '#6d5e47';
      }

      // Draw as a clean tile grid at the effective size
      if (effW !== r.w || effH !== r.h || tileSource) {
        // Resized or tile_source set — draw as wall/floor grid
        for (let y = 0; y < effH; y++) {
          for (let x = 0; x < effW; x++) {
            const isWall = x === 0 || x === effW - 1 || y === 0 || y === effH - 1;
            drawTile(rox + x, roy + y, isWall ? wallCol : floorCol);
          }
        }
      } else {
        // Original size, no tile_source — use backend tiles
        for (let y = 0; y < r.h; y++) {
          for (let x = 0; x < r.w; x++) {
            const gy = r.oy + y, gx = r.ox + x;
            if (md.tiles[gy]?.[gx]) {
              const def = md.tile_defs[md.tiles[gy][gx]];
              if (def && def.color !== '#000000') drawTile(rox + x, roy + y, def.color);
            }
          }
        }
      }
    }
  }

  // Redraw start/store with tile_source palette if configured
  for (const r of md.regions) {
    const isStart = r.node_idx === 0;
    const storeReg = md.regions.find((rr: any) => rr.node_idx > levels.length);
    const isStore = r.node_idx === storeReg?.node_idx;
    const tileSource = isStart ? campaign.overworld.start_tile_source : isStore ? campaign.overworld.store_tile_source : undefined;
    if (!tileSource) continue;
    const m2 = tileSource.match(/^level_(\d+)$/);
    if (!m2) continue;
    const idx = parseInt(m2[1]);
    const lv = levels[idx];
    if (!lv?.palette || lv.palette.length < 2) continue;
    const wallCol = lv.palette[0], floorCol = lv.palette[1];
    const p = regionPos(r);
    const gap = Math.max(0.5, tz * 0.08);
    for (let y = 0; y < r.h; y++) {
      for (let x = 0; x < r.w; x++) {
        const gy = r.oy + y, gx = r.ox + x;
        if (!md.tiles[gy]?.[gx]) continue;
        const tileName = md.tiles[gy][gx];
        const def = md.tile_defs[tileName];
        if (!def) continue;
        if (tileName === 'store_merchant') continue; // keep merchant tile
        const col = def.walkable ? floorCol : wallCol;
        ctx.fillStyle = col;
        ctx.fillRect(baseOx + (p.ox + x) * tz + gap / 2, baseOy + (p.oy + y) * tz + gap / 2, tz - gap, tz - gap);
      }
    }
  }

  // Draw room tiles BEFORE hallways so hallway floors carve through room walls
  const rooms = campaign.overworld.rooms || campaign.overworld.fork_chambers || [];
  const connections = campaign.overworld.connections || [];
  // (resolveCenter and roomHandlesMap need connections, defined below after resolveRegion)
  // We pre-draw room tiles here, handles come later
  {
    // Temporary resolveCenter for room handles (needed to know doorway positions)
    const tmpResolveCenter = (id: string): { ox: number; oy: number } | null => {
      const room = rooms.find(r => r.id === id);
      if (room) {
        const p = state.regionOverrides[id as any] || { ox: 0, oy: -20 };
        return { ox: p.ox + (room.w || ROOM_W) / 2, oy: p.oy + (room.h || ROOM_H) / 2 };
      }
      for (const r of md.regions) {
        const nid = r.node_idx === 0 ? 'start' : r.node_idx > levels.length ? 'store' : `level_${r.node_idx - 1}`;
        if (nid === id) {
          const p = state.regionOverrides[r.node_idx] || { ox: r.ox, oy: r.oy };
          return { ox: p.ox + r.w / 2, oy: p.oy + r.h / 2 };
        }
      }
      return null;
    };
    for (const room of rooms) {
      const pos = state.regionOverrides[room.id as any] || { ox: 0, oy: -20 };
      const rw = room.w || ROOM_W, rh = room.h || ROOM_H;
      const fx = baseOx + pos.ox * tz;
      const fy = baseOy + pos.oy * tz;
      // Use tile_source palette if set, otherwise default purple
      let wallColor = '#2a1a4e';
      let floorColor = '#3d2a6e';
      if (room.tile_source) {
        const m2 = room.tile_source.match(/^level_(\d+)$/);
        if (m2) {
          const idx = parseInt(m2[1]);
          const lv = levels[idx];
          if (lv?.palette && lv.palette.length >= 2) {
            wallColor = lv.palette[0] || wallColor;
            floorColor = lv.palette[1] || floorColor;
          }
        }
      }
      const gap = Math.max(0.5, tz * 0.08);
      const roomDoorHandles = computeRoomHandles(room, pos, connections, tmpResolveCenter);
      const doorTiles = new Set<string>();
      for (const h of roomDoorHandles) {
        if (!h.connKey) continue;
        doorTiles.add(`${h.lx},${h.ly}`);
        if (h.lx === 0 || h.lx === rw - 1) { doorTiles.add(`${h.lx},${h.ly - 1}`); doorTiles.add(`${h.lx},${h.ly + 1}`); }
        else { doorTiles.add(`${h.lx - 1},${h.ly}`); doorTiles.add(`${h.lx + 1},${h.ly}`); }
      }
      for (let ry = 0; ry < rh; ry++) {
        for (let rx = 0; rx < rw; rx++) {
          const isWall = rx === 0 || rx === rw - 1 || ry === 0 || ry === rh - 1;
          const isDoor = doorTiles.has(`${rx},${ry}`);
          ctx.fillStyle = (isWall && !isDoor) ? wallColor : floorColor;
          ctx.fillRect(fx + rx * tz + gap / 2, fy + ry * tz + gap / 2, tz - gap, tz - gap);
        }
      }
    }
  }

  // Draw hallways
  const storeRegion = md.regions.find(r => r.node_idx > levels.length);

  // Build virtual region lookup: resolves any node ID to a region-like object
  const resolveRegion = (id: string | number): any | null => {
    const s = String(id);
    // Rooms (formerly fork chambers)
    const room = rooms.find(r => r.id === s);
    if (room) {
      const pos = state.regionOverrides[s as any] || { ox: 0, oy: -20 };
      return { node_idx: s, ox: pos.ox, oy: pos.oy, w: room.w || ROOM_W, h: room.h || ROOM_H, _isRoom: true, _room: room };
    }
    // Regular regions
    let nodeIdx: number | null = null;
    if (s === 'start') nodeIdx = 0;
    else if (s === 'store' || s === 'end') nodeIdx = storeRegion?.node_idx ?? null;
    else { const m = s.match(/level_(\d+)/); nodeIdx = m ? parseInt(m[1]) + 1 : null; }
    if (nodeIdx == null) return null;
    return md.regions.find(r => r.node_idx === nodeIdx) || null;
  };

  // Build cache key from connections + region positions (only recompute A* when these change)
  const cacheKeyParts: string[] = [];
  for (const [a, b] of connections) cacheKeyParts.push(a + '>' + b);
  for (const [k, v] of Object.entries(state.regionOverrides)) cacheKeyParts.push(k + ':' + v.ox + ',' + v.oy);
  // Resolve any node ID to its center tile position
  const resolveCenter = (id: string): { ox: number; oy: number } | null => {
    const reg = resolveRegion(id);
    if (!reg) return null;
    const p = reg._isRoom ? { ox: reg.ox, oy: reg.oy } : regionPos(reg);
    return { ox: p.ox + reg.w / 2, oy: p.oy + reg.h / 2 };
  };
  // Pre-computed room handles
  const roomHandlesMap = new Map<string, RoomHandle[]>();
  for (const room of rooms) {
    const rp = state.regionOverrides[room.id as any] || { ox: 0, oy: -20 };
    roomHandlesMap.set(room.id, computeRoomHandles(room, rp, connections, resolveCenter));
  }

  const cacheKey = cacheKeyParts.join('|');
  if (state.hallwayCacheKey !== cacheKey) {
    state.hallwayCache = new Map();
    state.hallwayCacheKey = cacheKey;

    // Build occupied set: all regions + rooms
    const occupied = new Set<string>();
    for (const rr of md.regions) {
      const p = regionPos(rr);
      for (let y = 0; y < rr.h; y++)
        for (let x = 0; x < rr.w; x++)
          occupied.add((p.ox + x) + ',' + (p.oy + y));
    }
    for (const room of rooms) {
      const rp = state.regionOverrides[room.id as any] || { ox: 0, oy: -20 };
      const rw = room.w || ROOM_W, rh = room.h || ROOM_H;
      for (let y = 0; y < rh; y++)
        for (let x = 0; x < rw; x++)
          occupied.add((rp.ox + x) + ',' + (rp.oy + y));
    }

    // Helper: get door position for connection endpoint
    const getDoorForConn = (region: any, connKey: string, type: 'exit' | 'entry'): number[] | null => {
      if (region._isRoom) {
        const handles = roomHandlesMap.get(region._room.id);
        // Find the handle matching this connection
        const h = handles?.find(h => h.connKey === connKey && h.type === type);
        if (h) return [h.lx, h.ly];
        // Fall back to a free handle of the right type
        const free = handles?.find(h => h.connKey === null && h.type === type);
        return free ? [free.lx, free.ly] : [type === 'exit' ? region.w - 1 : 0, Math.floor(region.h / 2)];
      }
      return getDoorPos(region, type, designs);
    };

    for (const [a, b] of connections) {
      const ra = resolveRegion(a), rb = resolveRegion(b);
      if (!ra || !rb) continue;
      const raP = ra._isRoom ? { ox: ra.ox, oy: ra.oy } : regionPos(ra);
      const rbP = rb._isRoom ? { ox: rb.ox, oy: rb.oy } : regionPos(rb);
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

  for (const [a, b] of connections) {
    const connKey = `${a}->${b}`;
    const path = state.hallwayCache?.get(connKey);
    if (!path || path.length === 0) continue;

    try {
    const ra = resolveRegion(a), rb = resolveRegion(b);
    if (!ra || !rb) continue;

    // Palette blend
    const aLevelIdx = typeof ra.node_idx === 'number' ? ra.node_idx - 1 : -1;
    const bLevelIdx = typeof rb.node_idx === 'number' ? rb.node_idx - 1 : -1;
    const palA = levels[aLevelIdx]?.palette || levels[0]?.palette || ['#444', '#333'];
    const palB = levels[bLevelIdx]?.palette || levels[0]?.palette || ['#444', '#333'];
    const wallA = palA[0] || '#222', wallB = palB[0] || '#222';
    const floorA = palA[Math.min(1, palA.length - 1)] || '#333';
    const floorB = palB[Math.min(1, palB.length - 1)] || '#333';

    // Organic corridor
    const seed = ((aLevelIdx + 1) * 31 + (bLevelIdx + 1) * 17) | 0;
    const srand = (i: number) => { let v = Math.sin(seed + i * 127.1) * 43758.5453; return v - Math.floor(v); };
    const floorSet = new Set<string>();
    const wallSet = new Set<string>();

    for (let pi = 0; pi < path.length; pi++) {
      const [px, py] = path[pi];
      const prev = pi > 0 ? path[pi - 1] : path[pi];
      const next = pi < path.length - 1 ? path[pi + 1] : path[pi];
      const isHoriz = Math.abs(next[0] - prev[0]) >= Math.abs(next[1] - prev[1]);
      const noiseL = srand(pi * 3) * 0.5 + srand(Math.floor(pi / 8) * 3 + 100) * 0.5;
      const noiseR = srand(pi * 3 + 200) * 0.5 + srand(Math.floor(pi / 8) * 3 + 300) * 0.5;
      const extraL = noiseL > 0.7 ? (noiseL > 0.9 ? 2 : 1) : 0;
      const extraR = noiseR > 0.7 ? (noiseR > 0.9 ? 2 : 1) : 0;
      for (let w = -extraL; w <= extraR; w++) {
        const wx = isHoriz ? px : px + w;
        const wy = isHoriz ? py + w : py;
        floorSet.add(wx + ',' + wy);
      }
      floorSet.add(px + ',' + py);
    }
    // Corners: 3x3 floor
    for (let pi = 1; pi < path.length - 1; pi++) {
      const [cx, cy] = path[pi], [px, py] = path[pi - 1], [nx, ny] = path[pi + 1];
      if (px !== nx && py !== ny) {
        for (let dy = -1; dy <= 1; dy++)
          for (let dx = -1; dx <= 1; dx++)
            floorSet.add((cx + dx) + ',' + (cy + dy));
      }
    }
    // Wall border
    for (const fk of floorSet) {
      const [fx, fy] = fk.split(',').map(Number);
      for (let dy = -1; dy <= 1; dy++)
        for (let dx = -1; dx <= 1; dx++) {
          const nk = (fx + dx) + ',' + (fy + dy);
          if (!floorSet.has(nk)) wallSet.add(nk);
        }
    }
    // Build set of tiles inside rooms AND level regions (don't draw hallway walls over interiors)
    const roomInterior = new Set<string>();
    for (const r of md.regions) {
      const p = regionPos(r);
      for (let ry = 0; ry < r.h; ry++)
        for (let rx = 0; rx < r.w; rx++)
          roomInterior.add((p.ox + rx) + ',' + (p.oy + ry));
    }
    for (const room of rooms) {
      const rp = state.regionOverrides[room.id as any] || { ox: 0, oy: -20 };
      const rrw = room.w || ROOM_W, rrh = room.h || ROOM_H;
      for (let ry = 0; ry < rrh; ry++)
        for (let rx = 0; rx < rrw; rx++)
          roomInterior.add((rp.ox + rx) + ',' + (rp.oy + ry));
    }
    // Draw walls (skip tiles inside rooms)
    for (const wk of wallSet) {
      if (roomInterior.has(wk)) continue;
      const [wx, wy] = wk.split(',').map(Number);
      let best = 0;
      for (let pi = 0; pi < path.length; pi += 5) {
        if (Math.abs(path[pi][0] - wx) + Math.abs(path[pi][1] - wy) < Math.abs(path[best][0] - wx) + Math.abs(path[best][1] - wy)) best = pi;
      }
      drawTile(wx, wy, lerpColor(wallA, wallB, best / (path.length - 1 || 1)));
    }
    // Draw floors (these CAN overwrite room walls for doorway connections)
    for (const fk of floorSet) {
      const [fx, fy] = fk.split(',').map(Number);
      let best = 0;
      for (let pi = 0; pi < path.length; pi += 5) {
        if (Math.abs(path[pi][0] - fx) + Math.abs(path[pi][1] - fy) < Math.abs(path[best][0] - fx) + Math.abs(path[best][1] - fy)) best = pi;
      }
      drawTile(fx, fy, lerpColor(floorA, floorB, best / (path.length - 1 || 1)));
    }
    } catch (err) { console.error('Hallway render error for', a, '->', b, err); }
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
    const nid = nodeIdFor(r.node_idx);
    const { ox: rox, oy: roy } = regionPos(r);
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

  // Labels and selection highlights
  for (const r of md.regions) {
    const levelsIdx = r.node_idx - 1;
    const override = state.regionOverrides[r.node_idx];
    const rox = override ? override.ox : r.ox;
    const roy = override ? override.oy : r.oy;

    // Labels (skip start room — it has its own title text)
    if (r.node_idx !== 0) {
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
      const isStart = r.node_idx === 0;
      const isStore = r.node_idx > levels.length;
      // Use overridden size if available
      const effW = isStart ? (campaign.overworld.start_room_size?.[0] || r.w) : isStore ? (campaign.overworld.store_room_size?.[0] || r.w) : r.w;
      const effH = isStart ? (campaign.overworld.start_room_size?.[1] || r.h) : isStore ? (campaign.overworld.store_room_size?.[1] || r.h) : r.h;
      ctx.strokeStyle = '#e94560';
      ctx.lineWidth = 2;
      const rx = baseOx + rox * tz, ry2 = baseOy + roy * tz;
      const rfw = effW * tz, rfh = effH * tz;
      ctx.strokeRect(rx - 2, ry2 - 2, rfw + 4, rfh + 4);
      if (isStart || isStore) {
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
    const trOverride = state.regionOverrides[titleRegion.node_idx];
    const trOx = trOverride ? trOverride.ox : titleRegion.ox;
    const trOy = trOverride ? trOverride.oy : titleRegion.oy;
    const trCx = baseOx + (trOx + titleRegion.w / 2) * tz;
    const trCy = baseOy + (trOy + titleRegion.h / 2) * tz;

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

  // ── Room handles and selection (tiles already drawn above) ──
  for (const room of rooms) {
    const pos = state.regionOverrides[room.id as any] || { ox: 0, oy: -20 };
    const rw = room.w || ROOM_W, rh = room.h || ROOM_H;
    const fx = baseOx + pos.ox * tz;
    const fy = baseOy + pos.oy * tz;
    const fw = rw * tz, fh = rh * tz;

    // Door handles — hot dogs, only show unconnected, hidden when selected
    if (selectedNode !== room.id) {
      const roomHandles = roomHandlesMap?.get(room.id) || computeRoomHandles(room, pos, connections, resolveCenter);
      for (const h of roomHandles) {
        if (h.connKey) continue; // hide connected handles
        const hx = fx + h.lx * tz + tz / 2;
        const hy = fy + h.ly * tz + tz / 2;
        const isVertical = h.lx === 0 || h.lx === rw - 1;
        ctx.globalAlpha = 0.5;
        drawHandle(hx, hy, isVertical, h.type === 'entry' ? '#44ccff' : '#ff4444');
        ctx.globalAlpha = 1;
      }
    }

    if (selectedNode === room.id) {
      ctx.strokeStyle = '#e94560';
      ctx.lineWidth = 2;
      ctx.strokeRect(fx - 2, fy - 2, fw + 4, fh + 4);
      // Resize handle at bottom-right corner
      const hs = Math.max(4, tz * 0.4);
      ctx.fillStyle = '#fff';
      ctx.fillRect(fx + fw - hs, fy + fh - hs, hs * 2, hs * 2);
    }
  }
}

// Room default dimensions (shared with hit testing)
const ROOM_W = 10, ROOM_H = 8;

// Hit test: find which region (by node_idx or fork id) the mouse is over
export function hitTestRegion(
  mx: number, my: number,
  vpW: number, vpH: number,
  state: OwCanvasState,
  campaign?: BundledCampaign,
): number | string | null {
  const md = state.mapData;
  if (!md?.regions) return null;
  const tz = 4 * state.zoom;
  const mapW = md.width * tz;
  const mapH = md.height * tz;
  const baseOx = (vpW - mapW) / 2 + state.panX;
  const baseOy = (vpH - mapH) / 2 + state.panY;

  for (const r of md.regions) {
    const override = state.regionOverrides[r.node_idx];
    const rox = override ? override.ox : r.ox;
    const roy = override ? override.oy : r.oy;
    const rx = baseOx + rox * tz;
    const ry = baseOy + roy * tz;
    if (mx >= rx && mx <= rx + r.w * tz && my >= ry && my <= ry + r.h * tz) {
      return r.node_idx;
    }
  }

  // Check rooms
  if (campaign) {
    const allRooms = campaign.overworld.rooms || campaign.overworld.fork_chambers || [];
    for (const room of allRooms) {
      const pos = state.regionOverrides[room.id as any] || { ox: 0, oy: -20 };
      const rw = room.w || ROOM_W, rh = room.h || ROOM_H;
      const fx = baseOx + pos.ox * tz;
      const fy = baseOy + pos.oy * tz;
      if (mx >= fx && mx <= fx + rw * tz && my >= fy && my <= fy + rh * tz) {
        return room.id;
      }
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
  const md = state.mapData;
  if (!md?.regions) return null;
  const designs = campaign.designs || [];
  const tz = 4 * state.zoom;
  const mapW = md.width * tz;
  const mapH = md.height * tz;
  const baseOx = (vpW - mapW) / 2 + state.panX;
  const baseOy = (vpH - mapH) / 2 + state.panY;
  const hitR = Math.max(8, tz * 1.5);

  for (const r of md.regions) {
    const o = state.regionOverrides[r.node_idx];
    const rox = o ? o.ox : r.ox;
    const roy = o ? o.oy : r.oy;

    const exitDoor = getDoorPos(r, 'exit', designs);
    if (exitDoor) {
      const hx = baseOx + (rox + exitDoor[0]) * tz + tz / 2;
      const hy = baseOy + (roy + exitDoor[1]) * tz + tz / 2;
      if (Math.abs(mx - hx) < hitR && Math.abs(my - hy) < hitR) {
        return { type: 'exit', nodeIdx: r.node_idx, sx: hx, sy: hy };
      }
    }
    const entryDoor = getDoorPos(r, 'entry', designs);
    if (entryDoor) {
      const hx = baseOx + (rox + entryDoor[0]) * tz + tz / 2;
      const hy = baseOy + (roy + entryDoor[1]) * tz + tz / 2;
      if (Math.abs(mx - hx) < hitR && Math.abs(my - hy) < hitR) {
        return { type: 'entry', nodeIdx: r.node_idx, sx: hx, sy: hy };
      }
    }
  }

  // Check room handles (dynamic positions)
  const allRooms = campaign.overworld.rooms || campaign.overworld.fork_chambers || [];
  const connections = campaign.overworld.connections || [];
  const resolveCenter = (id: string): { ox: number; oy: number } | null => {
    const allR = campaign.overworld.rooms || campaign.overworld.fork_chambers || [];
    const room = allR.find(r => r.id === id);
    if (room) {
      const p = state.regionOverrides[id as any] || { ox: 0, oy: -20 };
      return { ox: p.ox + (room.w || ROOM_W) / 2, oy: p.oy + (room.h || ROOM_H) / 2 };
    }
    // Regular region
    const levels = campaign.overworld.levels || [];
    const storeReg = md.regions.find(r => r.node_idx > levels.length);
    let nodeIdx: number | null = null;
    const s = String(id);
    if (s === 'start') nodeIdx = 0;
    else if (s === 'store' || s === 'end') nodeIdx = storeReg?.node_idx ?? null;
    else { const m2 = s.match(/level_(\d+)/); nodeIdx = m2 ? parseInt(m2[1]) + 1 : null; }
    if (nodeIdx == null) return null;
    const r = md.regions.find(r => r.node_idx === nodeIdx);
    if (!r) return null;
    const p = state.regionOverrides[r.node_idx] || { ox: r.ox, oy: r.oy };
    return { ox: p.ox + r.w / 2, oy: p.oy + r.h / 2 };
  };
  for (const room of allRooms) {
    const pos = state.regionOverrides[room.id as any] || { ox: 0, oy: -20 };
    const handles = computeRoomHandles(room, pos, connections, resolveCenter);
    const fx = baseOx + pos.ox * tz;
    const fy = baseOy + pos.oy * tz;
    for (const h of handles) {
      const hx = fx + h.lx * tz + tz / 2;
      const hy = fy + h.ly * tz + tz / 2;
      if (Math.abs(mx - hx) < hitR && Math.abs(my - hy) < hitR) {
        return { type: h.type, nodeIdx: room.id, sx: hx, sy: hy };
      }
    }
  }

  return null;
}
