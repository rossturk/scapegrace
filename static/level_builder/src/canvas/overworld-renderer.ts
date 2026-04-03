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

// Build renderer-compatible region data from builder_regions
function buildRendererRegions(campaign: BundledCampaign, state: OwCanvasState): {
  regions: any[];
  width: number;
  height: number;
} {
  const br = campaign.overworld.builder_regions;
  if (!br || br.length === 0) {
    // Fallback to old mapData if no builder_regions yet
    const md = state.mapData;
    if (md?.regions) {
      return { regions: md.regions, width: md.width, height: md.height };
    }
    return { regions: [], width: 60, height: 36 };
  }

  // Convert builder_regions to the format the renderer expects
  const regions = br.map(r => ({
    node_idx: r.id, // string ID now (was number before)
    ox: r.ox,
    oy: r.oy,
    w: r.w,
    h: r.h,
    _builderRegion: r,
    // Fake entry/exit pos for backward compat
    entry_pos: null,
    exit_pos: null,
  }));

  // Compute bounds
  let maxX = 0, maxY = 0;
  for (const r of br) {
    maxX = Math.max(maxX, r.ox + r.w + 20);
    maxY = Math.max(maxY, r.oy + r.h + 20);
  }

  return { regions, width: maxX, height: maxY };
}

export function drawOverworld(
  ctx: CanvasRenderingContext2D,
  vpW: number,
  vpH: number,
  campaign: BundledCampaign,
  state: OwCanvasState,
  selectedNode: number | string | null,
) {
  const rendererData = buildRendererRegions(campaign, state);
  const levels = campaign.overworld.levels || [];
  const designs = campaign.designs || [];
  const builderRegions = campaign.overworld.builder_regions || [];

  // Build md-compatible shim from builder_regions so legacy code works
  const md = builderRegions.length > 0 ? {
    width: rendererData.width,
    height: rendererData.height,
    regions: builderRegions.map(br => ({
      node_idx: br.type === 'start' ? 0
        : br.type === 'store' ? levels.length + 1
        : br.type === 'level' ? (br.level_idx ?? 0) + 1
        : br.id, // rooms keep string id
      ox: br.ox, oy: br.oy, w: br.w, h: br.h,
      entry_pos: null, exit_pos: null,
      _builderRegion: br,
    })),
    tiles: [] as any,
    tile_defs: {} as any,
  } : state.mapData || { width: 60, height: 36, regions: [], tiles: [], tile_defs: {} };
  // Store on state so hit test functions can use it
  (state as any)._renderedMd = md;

  const TILE = 4;
  const zoom = state.zoom;
  const tz = TILE * zoom;

  const gridW = rendererData.width;
  const gridH = rendererData.height;
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

  const regionPos = (r: any) => {
    // Always use the region's own position (builder_regions is source of truth)
    return { ox: r.ox, oy: r.oy };
  };

  if (builderRegions.length === 0 && !md?.regions) return;

  // Draw each region's tiles
  for (const br of builderRegions) {
    const rox = br.ox, roy = br.oy, rw = br.w, rh = br.h;
    const levelIdx = br.level_idx ?? -1;
    const design = levelIdx >= 0 ? designs[levelIdx] : null;

    if (br.type === 'level' && design?.prebuilt_map?.tiles) {
      // Level: draw from prebuilt_map tiles
      const tiles = design.prebuilt_map.tiles;
      const defs = design.tile_defs || [];
      const pal = levels[levelIdx]?.palette || ['#444'];
      const defMap: Record<string, string> = {};
      const imgMap: Record<string, string | undefined> = {};
      for (let i = 0; i < defs.length; i++) {
        defMap[defs[i].name] = pal[i % pal.length] || '#444';
        imgMap[defs[i].name] = defs[i].image;
      }
      const pe = design.placed_entities ?? {} as any;
      const doorTiles = new Set<string>();
      if (pe.exit_door) for (let dy = -1; dy <= 1; dy++) doorTiles.add(pe.exit_door[0] + ',' + (pe.exit_door[1] + dy));
      if (pe.entry_door) for (let dy = -1; dy <= 1; dy++) doorTiles.add(pe.entry_door[0] + ',' + (pe.entry_door[1] + dy));
      const floorColor = pal[Math.min(1, pal.length - 1)] || '#333';
      const floorImg = defs.length > 1 ? defs[1].image : undefined;
      for (let y = 0; y < tiles.length; y++) {
        for (let x = 0; x < tiles[y].length; x++) {
          const tname = tiles[y][x];
          if (doorTiles.has(x + ',' + y)) drawTile(rox + x, roy + y, floorColor, floorImg);
          else drawTile(rox + x, roy + y, defMap[tname] || '#333', imgMap[tname]);
        }
      }
    } else {
      // Room/store/start: draw as wall/floor tile grid with optional sprites
      let wallCol = '#333', floorCol = '#555';
      let wallImg: string | undefined, floorImg: string | undefined;
      if (br.tile_source) {
        const m2 = br.tile_source.match(/^level_(\d+)$/);
        if (m2) {
          const srcIdx = parseInt(m2[1]);
          const lv = levels[srcIdx];
          if (lv?.palette && lv.palette.length >= 2) {
            wallCol = lv.palette[0]; floorCol = lv.palette[1];
          }
          const srcDefs = designs[srcIdx]?.tile_defs || [];
          wallImg = srcDefs[0]?.image;
          floorImg = srcDefs.length > 1 ? srcDefs[1]?.image : undefined;
        }
      } else if (br.type === 'room') {
        wallCol = '#2a1a4e'; floorCol = '#3d2a6e';
      } else if (br.type === 'store') {
        wallCol = '#5d4e37'; floorCol = '#6d5e47';
      } else if (br.type === 'start') {
        wallCol = '#3a2a1a'; floorCol = '#4a3a2a';
      }
      for (let y = 0; y < rh; y++) {
        for (let x = 0; x < rw; x++) {
          const isWall = x === 0 || x === rw - 1 || y === 0 || y === rh - 1;
          drawTile(rox + x, roy + y, isWall ? wallCol : floorCol, isWall ? wallImg : floorImg);
        }
      }
    }
  }
  // Draw hallways
  const connections = campaign.overworld.connections || [];
  const rooms = campaign.overworld.rooms || campaign.overworld.fork_chambers || [];

  // Resolve any node ID to a region object from builder_regions
  const resolveRegion = (id: string | number): any | null => {
    const s = String(id);
    // Look up in builder_regions first
    const br2 = builderRegions.find(r => r.id === s);
    if (br2) {
      return { node_idx: s, ox: br2.ox, oy: br2.oy, w: br2.w, h: br2.h, _isRoom: br2.type === 'room', _builderRegion: br2 };
    }
    // Handle 'end' as alias for 'store'
    if (s === 'end') {
      const store = builderRegions.find(r => r.type === 'store');
      if (store) return { node_idx: store.id, ox: store.ox, oy: store.oy, w: store.w, h: store.h, _builderRegion: store };
    }
    // Legacy fallback to md.regions
    if (md?.regions) {
      const storeReg = md.regions.find((r: any) => r.node_idx > levels.length);
      let nodeIdx: number | null = null;
      if (s === 'start') nodeIdx = 0;
      else if (s === 'store') nodeIdx = (storeReg?.node_idx as any) ?? null;
      else { const m = s.match(/level_(\d+)/); nodeIdx = m ? parseInt(m[1]) + 1 : null; }
      if (nodeIdx != null) return md.regions.find((r: any) => r.node_idx === nodeIdx) || null;
    }
    return null;
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
  // Pre-computed handles for all non-level regions (rooms, store, start)
  const roomHandlesMap = new Map<string, RoomHandle[]>();
  for (const br of builderRegions) {
    if (br.type === 'level') continue;
    const fakeRoom = { id: br.id, name: '', w: br.w, h: br.h };
    const rp = { ox: br.ox, oy: br.oy };
    roomHandlesMap.set(br.id, computeRoomHandles(fakeRoom, rp, connections, resolveCenter));
  }
  // Legacy: also add from old rooms array if no builder_regions
  if (builderRegions.length === 0) {
    for (const room of rooms) {
      const rp = state.regionOverrides[room.id as any] || { ox: 0, oy: -20 };
      roomHandlesMap.set(room.id, computeRoomHandles(room, rp, connections, resolveCenter));
    }
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

    // Palette + sprite blend between connected levels
    const aLi = ra._builderRegion?.level_idx ?? (typeof ra.node_idx === 'number' ? ra.node_idx - 1 : -1);
    const bLi = rb._builderRegion?.level_idx ?? (typeof rb.node_idx === 'number' ? rb.node_idx - 1 : -1);
    const palA = levels[aLi]?.palette || levels[0]?.palette || ['#444', '#333'];
    const palB = levels[bLi]?.palette || levels[0]?.palette || ['#444', '#333'];
    const wallA = palA[0] || '#222', wallB = palB[0] || '#222';
    const floorA = palA[Math.min(1, palA.length - 1)] || '#333';
    const floorB = palB[Math.min(1, palB.length - 1)] || '#333';
    // Get tile sprites from each level's defs
    const defsA = designs[aLi]?.tile_defs || [];
    const defsB = designs[bLi]?.tile_defs || [];
    const wallImgA = defsA[0]?.image;
    const wallImgB = defsB[0]?.image;
    const floorImgsA = defsA.filter((_: any, i: number) => i > 0).map((d: any) => d.image).filter(Boolean);
    const floorImgsB = defsB.filter((_: any, i: number) => i > 0).map((d: any) => d.image).filter(Boolean);

    // Organic corridor
    const seed = ((aLi + 1) * 31 + (bLi + 1) * 17) | 0;
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
    // Seeded random for picking sprites
    const pickRand = (hash: number) => { let v = Math.sin(seed + hash * 73.1) * 43758.5453; return v - Math.floor(v); };

    // Short blend zone: A tiles for first 40%, B tiles for last 40%, mix only in middle 20%
    const blendStart = 0.4, blendEnd = 0.6;
    const pickSide = (t: number, hash: number): boolean => {
      if (t < blendStart) return false; // use A
      if (t > blendEnd) return true;    // use B
      // Middle 20%: probabilistic blend
      const blendT = (t - blendStart) / (blendEnd - blendStart);
      return pickRand(hash) < blendT;
    };

    // Draw walls (skip tiles inside rooms/levels)
    for (const wk of wallSet) {
      if (roomInterior.has(wk)) continue;
      const [wx, wy] = wk.split(',').map(Number);
      let best = 0;
      for (let pi = 0; pi < path.length; pi += 5) {
        if (Math.abs(path[pi][0] - wx) + Math.abs(path[pi][1] - wy) < Math.abs(path[best][0] - wx) + Math.abs(path[best][1] - wy)) best = pi;
      }
      const t = best / (path.length - 1 || 1);
      const useB = pickSide(t, wx * 31 + wy * 17);
      drawTile(wx, wy, useB ? wallB : wallA, useB ? wallImgB : wallImgA);
    }
    // Draw floors (these CAN overwrite room walls for doorway connections)
    for (const fk of floorSet) {
      const [fx, fy] = fk.split(',').map(Number);
      let best = 0;
      for (let pi = 0; pi < path.length; pi += 5) {
        if (Math.abs(path[pi][0] - fx) + Math.abs(path[pi][1] - fy) < Math.abs(path[best][0] - fx) + Math.abs(path[best][1] - fy)) best = pi;
      }
      const t = best / (path.length - 1 || 1);
      const useB = pickSide(t, fx * 31 + fy * 17);
      const pool = useB ? floorImgsB : floorImgsA;
      const floorImg = pool.length > 0 ? pool[Math.floor(pickRand(fx * 13 + fy * 7) * pool.length)] : undefined;
      drawTile(fx, fy, useB ? floorB : floorA, floorImg);
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
    const nid = typeof r.node_idx === 'string' ? r.node_idx : nodeIdFor(r.node_idx);
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
      const isStart = r.node_idx === 0;
      const isStore = typeof r.node_idx === 'number' && r.node_idx > levels.length;
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

// Overworld tile image cache (keyed by base64 — shared across frames)
const owTileImageCache = new Map<string, HTMLImageElement>();

// Room default dimensions (shared with hit testing)
const ROOM_W = 10, ROOM_H = 8;

// Hit test: find which region (by node_idx or fork id) the mouse is over
export function hitTestRegion(
  mx: number, my: number,
  vpW: number, vpH: number,
  state: OwCanvasState,
  campaign?: BundledCampaign,
): number | string | null {
  // Use builder_regions directly for hit testing (always up-to-date)
  const br = campaign?.overworld.builder_regions;
  const md = (state as any)._renderedMd || state.mapData;
  const levels = campaign?.overworld.levels || [];
  const gridW = br && br.length > 0
    ? Math.max(...br.map(r => r.ox + r.w)) + 20
    : md ? md.width : 60;
  const gridH = br && br.length > 0
    ? Math.max(...br.map(r => r.oy + r.h)) + 20
    : md ? md.height : 36;
  const tz = 4 * state.zoom;
  const baseOx = (vpW - gridW * tz) / 2 + state.panX;
  const baseOy = (vpH - gridH * tz) / 2 + state.panY;

  if (br && br.length > 0) {
    for (const region of br) {
      const rx = baseOx + region.ox * tz;
      const ry = baseOy + region.oy * tz;
      if (mx >= rx && mx <= rx + region.w * tz && my >= ry && my <= ry + region.h * tz) {
        // Return node_idx compatible with the md shim
        if (region.type === 'start') return 0;
        if (region.type === 'store') return levels.length + 1;
        if (region.type === 'level') return (region.level_idx ?? 0) + 1;
        return region.id; // room string ID
      }
    }
    return null;
  }

  // Legacy fallback
  if (!md?.regions) return null;
  for (const r of md.regions) {
    const rx = baseOx + r.ox * tz;
    const ry = baseOy + r.oy * tz;
    if (mx >= rx && mx <= rx + r.w * tz && my >= ry && my <= ry + r.h * tz) {
      return r.node_idx;
    }
  }

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
  const br = campaign.overworld.builder_regions || [];
  const designs = campaign.designs || [];
  const levels = campaign.overworld.levels || [];
  const tz = 4 * state.zoom;
  const gridW = br.length > 0 ? Math.max(...br.map(r => r.ox + r.w)) + 20 : 60;
  const gridH = br.length > 0 ? Math.max(...br.map(r => r.oy + r.h)) + 20 : 36;
  const baseOx = (vpW - gridW * tz) / 2 + state.panX;
  const baseOy = (vpH - gridH * tz) / 2 + state.panY;
  // Hot dog hit area: wider along the wall edge
  const hitW = Math.max(6, tz * 1.5);  // along wall
  const hitH = Math.max(4, tz * 0.5);  // perpendicular

  // Check builder_regions handles
  if (br.length > 0) {
    for (const region of br) {
      const rox = region.ox, roy = region.oy;
      const rw = region.w, rh = region.h;

      // Get door positions
      const fakeR = { node_idx: region.type === 'start' ? 0 : region.type === 'store' ? levels.length + 1 : region.type === 'level' ? (region.level_idx ?? 0) + 1 : region.id, ox: rox, oy: roy, w: rw, h: rh, _builderRegion: region };
      const exitDoor = getDoorPos(fakeR, 'exit', designs);
      const entryDoor = getDoorPos(fakeR, 'entry', designs);

      const nodeIdx = region.type === 'start' ? 0 : region.type === 'store' ? levels.length + 1 : region.type === 'level' ? (region.level_idx ?? 0) + 1 : region.id;

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

  // Legacy fallback
  const md = (state as any)._renderedMd || state.mapData;
  if (!md?.regions) return null;
  const hitR = Math.max(8, tz * 1.5);
  for (const r of md.regions) {
    const rox = r.ox;
    const roy = r.oy;

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
