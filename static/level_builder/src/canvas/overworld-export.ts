// Builds a serializable overworld tile grid from the builder's state.
// This is the WYSIWYG export — what you see in the builder IS the game map.

import type { BundledCampaign, OverworldMapPreview } from '../types/pack';
import type { OwCanvasState } from './overworld-renderer';
import { computeRoomHandles } from './room-handles';

const ROOM_W = 10, ROOM_H = 8;

export interface ExportedOverworldMap {
  width: number;
  height: number;
  tiles: string[][];
  tile_defs: Record<string, { name: string; color: string; walkable: boolean }>;
  regions: {
    node_id: string;
    ox: number; oy: number;
    w: number; h: number;
    entry_pos?: [number, number];
    exit_pos?: [number, number];
  }[];
  player_pos: [number, number];
}

export function exportOverworldMap(
  campaign: BundledCampaign,
  state: OwCanvasState,
): ExportedOverworldMap | null {
  const md = state.mapData;
  if (!md?.regions) return null;

  const levels = campaign.overworld.levels || [];
  const designs = campaign.designs || [];
  const connections = campaign.overworld.connections || [];
  const rooms = campaign.overworld.rooms || campaign.overworld.fork_chambers || [];

  // Compute bounds: all regions + rooms + hallway paths
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;

  const regionPos = (r: any) => {
    const o = state.regionOverrides[r.node_idx];
    return o || { ox: r.ox, oy: r.oy };
  };

  for (const r of md.regions) {
    const p = regionPos(r);
    minX = Math.min(minX, p.ox); minY = Math.min(minY, p.oy);
    maxX = Math.max(maxX, p.ox + r.w); maxY = Math.max(maxY, p.oy + r.h);
  }
  for (const room of rooms) {
    const p = state.regionOverrides[room.id as any] || { ox: 0, oy: 0 };
    const rw = room.w || ROOM_W, rh = room.h || ROOM_H;
    minX = Math.min(minX, p.ox); minY = Math.min(minY, p.oy);
    maxX = Math.max(maxX, p.ox + rw); maxY = Math.max(maxY, p.oy + rh);
  }
  // Include hallway paths
  if (state.hallwayCache) {
    for (const path of state.hallwayCache.values()) {
      for (const [px, py] of path) {
        minX = Math.min(minX, px - 2); minY = Math.min(minY, py - 2);
        maxX = Math.max(maxX, px + 3); maxY = Math.max(maxY, py + 3);
      }
    }
  }

  // Add margin
  minX -= 3; minY -= 3; maxX += 3; maxY += 3;
  const width = maxX - minX;
  const height = maxY - minY;

  // Initialize grid with empty
  const tiles: string[][] = [];
  for (let y = 0; y < height; y++) {
    tiles.push(new Array(width).fill('void'));
  }

  const setTile = (wx: number, wy: number, name: string) => {
    const x = wx - minX, y = wy - minY;
    if (x >= 0 && y >= 0 && x < width && y < height) tiles[y][x] = name;
  };

  const getTile = (wx: number, wy: number): string => {
    const x = wx - minX, y = wy - minY;
    if (x >= 0 && y >= 0 && x < width && y < height) return tiles[y][x];
    return 'void';
  };

  // Collect tile defs from all levels
  const bgColor = campaign.overworld.bg_color || '#000000';
  const tileDefs: Record<string, { name: string; color: string; walkable: boolean }> = {
    void: { name: 'void', color: bgColor, walkable: false },
    room_wall: { name: 'room_wall', color: '#2a1a4e', walkable: false },
    room_floor: { name: 'room_floor', color: '#3d2a6e', walkable: true },
  };

  // Helper: resolve tile names from a tile_source (level_N id)
  const resolveTileSource = (tileSource?: string): { wall: string; floor: string } => {
    if (!tileSource) return { wall: 'room_wall', floor: 'room_floor' };
    const m2 = tileSource.match(/^level_(\d+)$/);
    if (m2) {
      const idx = parseInt(m2[1]);
      const design = designs[idx];
      if (design?.tile_defs && design.tile_defs.length >= 2) {
        return { wall: design.tile_defs[0].name, floor: design.tile_defs[1].name };
      }
    }
    return { wall: 'room_wall', floor: 'room_floor' };
  };

  // Write level region tiles
  for (const r of md.regions) {
    const p = regionPos(r);
    const di = r.node_idx - 1;
    const design = di >= 0 ? designs[di] : null;

    if (design?.prebuilt_map?.tiles) {
      const mapTiles = design.prebuilt_map.tiles;
      const defs = design.tile_defs || [];
      const pal = levels[di]?.palette || ['#444'];

      // Register tile defs for this level
      for (let i = 0; i < defs.length; i++) {
        const name = defs[i].name;
        if (!tileDefs[name]) {
          tileDefs[name] = { name, color: pal[i % pal.length] || '#333', walkable: i > 0 };
        }
      }

      // Write tiles, carving door openings
      const pe = design.placed_entities || {} as any;
      const doorTiles = new Set<string>();
      if (pe.exit_door) for (let dy = -1; dy <= 1; dy++) doorTiles.add(pe.exit_door[0] + ',' + (pe.exit_door[1] + dy));
      if (pe.entry_door) for (let dy = -1; dy <= 1; dy++) doorTiles.add(pe.entry_door[0] + ',' + (pe.entry_door[1] + dy));
      const floorName = defs.length > 1 ? defs[1].name : 'floor';

      for (let y = 0; y < mapTiles.length; y++) {
        for (let x = 0; x < mapTiles[y].length; x++) {
          const tileName = doorTiles.has(x + ',' + y) ? floorName : mapTiles[y][x];
          setTile(p.ox + x, p.oy + y, tileName);
        }
      }
    } else if (md.tiles) {
      // Start room / store — copy from backend grid, optionally remap tiles
      const isStartRegion = r.node_idx === 0;
      const storeRegion = md.regions.find((rr: any) => rr.node_idx > levels.length);
      const isStoreRegion = r.node_idx === storeRegion?.node_idx;
      const tileSource = isStartRegion ? campaign.overworld.start_tile_source
        : isStoreRegion ? campaign.overworld.store_tile_source
        : undefined;
      const remapTiles = tileSource ? resolveTileSource(tileSource) : null;

      for (let y = 0; y < r.h; y++) {
        for (let x = 0; x < r.w; x++) {
          const gy = r.oy + y, gx = r.ox + x;
          if (md.tiles[gy]?.[gx]) {
            let tileName = md.tiles[gy][gx];
            const def = md.tile_defs[tileName];
            if (def && !tileDefs[tileName]) {
              tileDefs[tileName] = { name: def.name, color: def.color, walkable: def.walkable };
            }
            // Remap to tile_source if configured
            if (remapTiles) {
              if (def && !def.walkable && tileName !== 'store_merchant') tileName = remapTiles.wall;
              else if (def && def.walkable && tileName !== 'store_merchant') tileName = remapTiles.floor;
            }
            setTile(p.ox + x, p.oy + y, tileName);
          }
        }
      }
    }
  }

  // Write room tiles (wall border + floor interior, with doorways at handle positions)
  const resolveCenter = (id: string): { ox: number; oy: number } | null => {
    // Check rooms
    const room = rooms.find(r => r.id === id);
    if (room) {
      const rp = state.regionOverrides[id as any] || { ox: 0, oy: 0 };
      return { ox: rp.ox + (room.w || ROOM_W) / 2, oy: rp.oy + (room.h || ROOM_H) / 2 };
    }
    // Check regions
    for (const r of md.regions) {
      const nodeId = r.node_idx === 0 ? 'start' : r.node_idx > levels.length ? 'store' : `level_${r.node_idx - 1}`;
      if (nodeId === id) {
        const p = regionPos(r);
        return { ox: p.ox + r.w / 2, oy: p.oy + r.h / 2 };
      }
    }
    return null;
  };

  for (const room of rooms) {
    const p = state.regionOverrides[room.id as any] || { ox: 0, oy: 0 };
    const rw = room.w || ROOM_W, rh = room.h || ROOM_H;
    // Compute doorway positions from connections
    const handles = computeRoomHandles(room, p, connections, resolveCenter);
    const doorTiles = new Set<string>();
    for (const h of handles) {
      if (!h.connKey) continue;
      doorTiles.add(`${h.lx},${h.ly}`);
      if (h.lx === 0 || h.lx === rw - 1) {
        doorTiles.add(`${h.lx},${h.ly - 1}`);
        doorTiles.add(`${h.lx},${h.ly + 1}`);
      } else {
        doorTiles.add(`${h.lx - 1},${h.ly}`);
        doorTiles.add(`${h.lx + 1},${h.ly}`);
      }
    }
    for (let y = 0; y < rh; y++) {
      for (let x = 0; x < rw; x++) {
        const isWall = x === 0 || x === rw - 1 || y === 0 || y === rh - 1;
        const isDoor = doorTiles.has(`${x},${y}`);
        const ts = resolveTileSource(room.tile_source);
        setTile(p.ox + x, p.oy + y, (isWall && !isDoor) ? ts.wall : ts.floor);
      }
    }
  }

  // Write hallway tiles from cached paths
  if (state.hallwayCache) {
    // Determine hallway tile names from connected levels' palettes
    // Helper: get tile def names for a level (wall = first tile def, floors = rest)
    const getLevelTiles = (id: string): { walls: string[]; floors: string[] } => {
      const m2 = id.match(/^level_(\d+)$/);
      if (m2) {
        const idx = parseInt(m2[1]);
        const design = designs[idx];
        if (design?.tile_defs) {
          const defs = design.tile_defs;
          const walls = defs.filter((_d: any, i: number) => i === 0).map((d: any) => d.name);
          const floors = defs.filter((_d: any, i: number) => i > 0).map((d: any) => d.name);
          if (walls.length > 0 && floors.length > 0) return { walls, floors };
        }
      }
      return { walls: ['hallway_wall'], floors: ['hallway_floor'] };
    };

    // Keep generic fallbacks
    if (!tileDefs['hallway_wall']) tileDefs['hallway_wall'] = { name: 'hallway_wall', color: '#333', walkable: false };
    if (!tileDefs['hallway_floor']) tileDefs['hallway_floor'] = { name: 'hallway_floor', color: '#555', walkable: true };

    for (const [connKey, path] of state.hallwayCache.entries()) {
      if (path.length === 0) continue;

      // Get actual tile types from connected levels
      const [fromId, toId] = connKey.split('->');
      const tilesA = getLevelTiles(fromId);
      const tilesB = getLevelTiles(toId);

      // Build floor + wall sets (same organic corridor logic as renderer)
      const floorSet = new Set<string>();
      const wallSet = new Set<string>();

      const seed = connKey.split('').reduce((a, c) => a + c.charCodeAt(0), 0);
      const srand = (i: number) => { let v = Math.sin(seed + i * 127.1) * 43758.5453; return v - Math.floor(v); };

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
      // Corners
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

      // Seeded random for deterministic tile picking
      const pickRand = (i: number) => {
        let v = Math.sin(seed + i * 73.1 + 91.7) * 43758.5453;
        return v - Math.floor(v);
      };

      // Helper: find closest path position (0..1 blend factor)
      const closestPathT = (tx: number, ty: number): number => {
        let bestDist = Infinity, bestIdx = 0;
        for (let pi = 0; pi < path.length; pi += Math.max(1, Math.floor(path.length / 20))) {
          const d = Math.abs(path[pi][0] - tx) + Math.abs(path[pi][1] - ty);
          if (d < bestDist) { bestDist = d; bestIdx = pi; }
        }
        return bestIdx / (path.length - 1 || 1);
      };

      // Pick a tile name from level A or B based on blend factor t
      const pickWall = (t: number, hash: number): string => {
        const useB = pickRand(hash) < t;
        const pool = useB ? tilesB.walls : tilesA.walls;
        return pool[Math.floor(pickRand(hash + 999) * pool.length)] || 'hallway_wall';
      };
      const pickFloor = (t: number, hash: number): string => {
        const useB = pickRand(hash) < t;
        const pool = useB ? tilesB.floors : tilesA.floors;
        return pool[Math.floor(pickRand(hash + 777) * pool.length)] || 'hallway_floor';
      };

      // Write walls (only if tile is currently void)
      for (const wk of wallSet) {
        const [wx, wy] = wk.split(',').map(Number);
        if (getTile(wx, wy) === 'void') {
          const t = closestPathT(wx, wy);
          setTile(wx, wy, pickWall(t, wx * 31 + wy * 17));
        }
      }
      // Write floors (overwrite void and walls)
      for (const fk of floorSet) {
        const [fx, fy] = fk.split(',').map(Number);
        const cur = getTile(fx, fy);
        // Don't overwrite level/room tiles, only void and hallway walls
        const isHallwayWall = tilesA.walls.includes(cur) || tilesB.walls.includes(cur) || cur === 'hallway_wall';
        if (cur === 'void' || isHallwayWall) {
          const t = closestPathT(fx, fy);
          setTile(fx, fy, pickFloor(t, fx * 31 + fy * 17));
        }
      }
    }
  }

  // Post-pass: carve doorways wherever a non-walkable non-level tile is adjacent to a walkable hallway tile
  // Build set of tiles that are "level interior" (from prebuilt_map) — these should NOT be carved
  const levelWalls = new Set<string>();
  for (const r of md.regions) {
    const di = r.node_idx - 1;
    if (di >= 0 && designs[di]?.tile_defs?.[0]) {
      levelWalls.add(designs[di].tile_defs[0].name); // each level's wall tile
    }
  }

  // Generic wall→floor mapping: find a walkable neighbor's tile type for replacement
  for (let y = 1; y < height - 1; y++) {
    for (let x = 1; x < width - 1; x++) {
      const cur = tiles[y][x];
      const curDef = tileDefs[cur];
      if (!curDef || curDef.walkable) continue; // skip walkable tiles
      if (cur === 'void' || cur === 'exit_door_locked' || cur === 'store_merchant') continue;
      if (levelWalls.has(cur)) continue; // don't carve level walls (they have exit doors)

      const neighbors: [number,number][] = [[x-1,y],[x+1,y],[x,y-1],[x,y+1]];
      let adjacentFloor: string | null = null;
      for (const [nx2, ny2] of neighbors) {
        if (nx2 < 0 || ny2 < 0 || nx2 >= width || ny2 >= height) continue;
        const adj = tiles[ny2][nx2];
        const adjDef = tileDefs[adj];
        if (adjDef?.walkable && adj !== cur) {
          adjacentFloor = adj;
          break;
        }
      }
      if (!adjacentFloor) continue;

      // This wall tile is adjacent to a walkable tile — carve it
      tiles[y][x] = adjacentFloor;
      // Widen doorway ±1
      for (const [dx, dy] of [[0,1],[0,-1],[1,0],[-1,0]]) {
        const nx2 = x + dx, ny2 = y + dy;
        if (nx2 >= 0 && ny2 >= 0 && nx2 < width && ny2 < height && tiles[ny2][nx2] === cur) {
          tiles[ny2][nx2] = adjacentFloor;
        }
      }
    }
  }

  // Build region metadata
  const exportRegions: ExportedOverworldMap['regions'] = [];
  for (const r of md.regions) {
    const p = regionPos(r);
    const nodeId = r.node_idx === 0 ? 'start'
      : r.node_idx > levels.length ? 'store'
      : `level_${r.node_idx - 1}`;
    exportRegions.push({
      node_id: nodeId,
      ox: p.ox - minX, oy: p.oy - minY,
      w: r.w, h: r.h,
      entry_pos: r.entry_pos ? [r.entry_pos[0] - minX, r.entry_pos[1] - minY] : undefined,
      exit_pos: r.exit_pos ? [r.exit_pos[0] - minX, r.exit_pos[1] - minY] : undefined,
    });
  }
  for (const room of rooms) {
    const p = state.regionOverrides[room.id as any] || { ox: 0, oy: 0 };
    exportRegions.push({
      node_id: room.id,
      ox: p.ox - minX, oy: p.oy - minY,
      w: room.w || ROOM_W, h: room.h || ROOM_H,
    });
  }

  // Player start position
  const startRegion = md.regions.find(r => r.node_idx === 0);
  const sp = startRegion ? regionPos(startRegion) : { ox: 0, oy: 0 };
  const playerPos: [number, number] = [
    Math.floor(sp.ox + (startRegion?.w || 40) / 2 - minX),
    Math.floor(sp.oy + (startRegion?.h || 40) / 2 - minY),
  ];

  return { width, height, tiles, tile_defs: tileDefs, regions: exportRegions, player_pos: playerPos };
}
