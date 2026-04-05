// Builds a serializable overworld tile grid from the builder's state.
// This is the WYSIWYG export — what you see in the builder IS the game map.

import type { BundledCampaign, OverworldMapPreview } from '../types/pack';
import type { OwCanvasState } from './overworld-renderer';
import { computeRoomHandles } from './room-handles';


export interface ExportedSignpost {
  title: string;
  description: string;
  title_font?: string;
  description_font?: string;
  x: number;
  y: number;
}

export interface ExportedOverworldMap {
  width: number;
  height: number;
  tiles: string[][];
  tile_defs: Record<string, { name: string; color: string; walkable: boolean; image?: string }>;
  regions: {
    node_id: string;
    ox: number; oy: number;
    w: number; h: number;
    entry_pos?: [number, number];
    exit_pos?: [number, number];
  }[];
  signposts?: ExportedSignpost[];
  player_pos: [number, number];
}

export function exportOverworldMap(
  campaign: BundledCampaign,
  state: OwCanvasState,
): ExportedOverworldMap | null {
  const builderRegions = campaign.overworld.builder_regions || [];
  if (builderRegions.length === 0) return null;

  const levels = campaign.overworld.levels || [];
  const designs = campaign.designs || [];
  const connections = campaign.overworld.connections || [];

  // Compute bounds from builder_regions
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const br of builderRegions) {
    minX = Math.min(minX, br.ox); minY = Math.min(minY, br.oy);
    maxX = Math.max(maxX, br.ox + br.w); maxY = Math.max(maxY, br.oy + br.h);
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

  // Helper: resolve tile IDs from a tile_source (level_N id) and register their defs
  const resolveTileSource = (tileSource?: string): { wall: string; floor: string } => {
    if (!tileSource) return { wall: 'room_wall', floor: 'room_floor' };
    const m2 = tileSource.match(/^level_(\d+)$/);
    if (m2) {
      const idx = parseInt(m2[1]);
      const design = designs[idx];
      const pal = levels[idx]?.palette || ['#444'];
      if (design?.tile_defs && design.tile_defs.length >= 2) {
        // Register tile defs keyed by mechanical ID
        for (let i = 0; i < design.tile_defs.length; i++) {
          const id = `L${idx}_t${i}`;
          const d = design.tile_defs[i];
          if (!tileDefs[id]) {
            const entry: ExportedOverworldMap['tile_defs'][string] = { name: d.name, color: pal[i % pal.length] || '#333', walkable: i > 0 };
            if (d.image) entry.image = d.image;
            tileDefs[id] = entry;
          }
        }
        return { wall: `L${idx}_t0`, floor: `L${idx}_t1` };
      }
    }
    return { wall: 'room_wall', floor: 'room_floor' };
  };

  // Write region tiles from builder_regions
  for (const br of builderRegions) {
    const p = { ox: br.ox, oy: br.oy };
    const di = br.level_idx ?? -1;
    const design = di >= 0 ? designs[di] : null;

    if (br.type === 'level' && design?.prebuilt_map?.tiles) {
      const mapTiles = design.prebuilt_map.tiles;
      const defs = design.tile_defs || [];
      const pal = levels[di]?.palette || ['#444'];

      // Register tile defs keyed by mechanical ID and build name→ID mapping
      const nameToId: Record<string, string> = {};
      for (let i = 0; i < defs.length; i++) {
        const id = `L${di}_t${i}`;
        nameToId[defs[i].name] = id;
        // Also map mechanical IDs to themselves (for maps already using IDs)
        nameToId[`t${i}`] = id;
        if (!tileDefs[id]) {
          const entry: ExportedOverworldMap['tile_defs'][string] = { name: defs[i].name, color: pal[i % pal.length] || '#333', walkable: i > 0 };
          if (defs[i].image) entry.image = defs[i].image;
          tileDefs[id] = entry;
        }
      }
      // System tiles pass through
      for (const sysId of ['locked_door', 'exit_door_locked', 'exit_door', 'entry_door']) {
        nameToId[sysId] = sysId;
      }

      // Write tiles, carving door openings
      const pe = design.placed_entities || {} as any;
      const doorTiles = new Set<string>();
      if (pe.exit_door) for (let dy = -1; dy <= 1; dy++) doorTiles.add(pe.exit_door[0] + ',' + (pe.exit_door[1] + dy));
      if (pe.entry_door) for (let dy = -1; dy <= 1; dy++) doorTiles.add(pe.entry_door[0] + ',' + (pe.entry_door[1] + dy));
      const floorId = `L${di}_t1`;

      for (let y = 0; y < mapTiles.length; y++) {
        for (let x = 0; x < mapTiles[y].length; x++) {
          const raw = doorTiles.has(x + ',' + y) ? defs[1]?.name || 'floor' : mapTiles[y][x];
          const tileId = nameToId[raw] || raw; // map creative name → ID, or pass through if already an ID
          setTile(p.ox + x, p.oy + y, tileId);
        }
      }
    } else {
      // Room/store/start: draw as wall/floor grid with optional tile_source
      const ts = resolveTileSource(br.tile_source);
      let wallName = ts.wall, floorName = ts.floor;
      if (!br.tile_source) {
        if (br.type === 'store') {
          wallName = 'store_wall'; floorName = 'store_floor';
          if (!tileDefs['store_wall']) tileDefs['store_wall'] = { name: 'store_wall', color: '#5d4e37', walkable: false };
          if (!tileDefs['store_floor']) tileDefs['store_floor'] = { name: 'store_floor', color: '#6d5e47', walkable: true };
        } else if (br.type === 'start') {
          wallName = 'title_wall'; floorName = 'title_floor';
          if (!tileDefs['title_wall']) tileDefs['title_wall'] = { name: 'title_wall', color: '#3a2a1a', walkable: false };
          if (!tileDefs['title_floor']) tileDefs['title_floor'] = { name: 'title_floor', color: '#4a3a2a', walkable: true };
        } else {
          wallName = 'room_wall'; floorName = 'room_floor';
        }
      }
      if (br.type === 'store') {
        if (!tileDefs['store_merchant']) tileDefs['store_merchant'] = { name: 'store_merchant', color: '#ffd700', walkable: false };
      }
      // Compute door openings from connections
      const resolveCenter = (id: string): { ox: number; oy: number } | null => {
        const br2 = builderRegions.find(r => r.id === id);
        if (br2) return { ox: br2.ox + br2.w / 2, oy: br2.oy + br2.h / 2 };
        return null;
      };
      const fakeRoom = { id: br.id, name: '', w: br.w, h: br.h };
      const handles = computeRoomHandles(fakeRoom, p, connections, resolveCenter);
      const doorTiles = new Set<string>();
      for (const h of handles) {
        if (!h.connKey) continue;
        doorTiles.add(`${h.lx},${h.ly}`);
        if (h.lx === 0 || h.lx === br.w - 1) {
          doorTiles.add(`${h.lx},${h.ly - 1}`);
          doorTiles.add(`${h.lx},${h.ly + 1}`);
        } else {
          doorTiles.add(`${h.lx - 1},${h.ly}`);
          doorTiles.add(`${h.lx + 1},${h.ly}`);
        }
      }
      for (let y = 0; y < br.h; y++) {
        for (let x = 0; x < br.w; x++) {
          const isWall = x === 0 || x === br.w - 1 || y === 0 || y === br.h - 1;
          const isDoor = doorTiles.has(`${x},${y}`);
          if (br.type === 'store' && x === Math.floor(br.w / 2) && y === Math.floor(br.h / 2)) {
            setTile(p.ox + x, p.oy + y, 'store_merchant');
          } else {
            setTile(p.ox + x, p.oy + y, (isWall && !isDoor) ? wallName : floorName);
          }
        }
      }
    }
  }

  // Build set of tiles inside regions (hallways must not overwrite these)
  const regionInterior = new Set<string>();
  for (const br of builderRegions) {
    for (let y = 0; y < br.h; y++)
      for (let x = 0; x < br.w; x++)
        regionInterior.add((br.ox + x) + ',' + (br.oy + y));
  }

  // Write hallway tiles from cached paths
  if (state.hallwayCache) {
    // Helper: get tile IDs for any node (level, room, store, start)
    const getNodeTiles = (id: string): { walls: string[]; floors: string[] } => {
      // Check if it's a level
      const m2 = id.match(/^level_(\d+)$/);
      if (m2) {
        const idx = parseInt(m2[1]);
        const design = designs[idx];
        if (design?.tile_defs && design.tile_defs.length >= 2) {
          const walls = [`L${idx}_t0`];
          const floors = design.tile_defs.slice(1).map((_: any, i: number) => `L${idx}_t${i + 1}`);
          resolveTileSource(`level_${idx}`);
          return { walls, floors };
        }
      }
      // Check if it's a room/store/start with a tile_source
      const br = builderRegions.find(r => r.id === id);
      if (br) {
        if (br.tile_source) {
          const ts = resolveTileSource(br.tile_source);
          return { walls: [ts.wall], floors: [ts.floor] };
        }
        if (br.type === 'store') return { walls: ['store_wall'], floors: ['store_floor'] };
        if (br.type === 'start') return { walls: ['title_wall'], floors: ['title_floor'] };
        return { walls: ['room_wall'], floors: ['room_floor'] };
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
      const tilesA = getNodeTiles(fromId);
      const tilesB = getNodeTiles(toId);

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

      // Short blend zone: A for first 40%, B for last 40%, mix in middle 20%
      const pickSide = (t: number, hash: number): boolean => {
        if (t < 0.4) return false;
        if (t > 0.6) return true;
        return pickRand(hash) < (t - 0.4) / 0.2;
      };
      const pickWall = (t: number, hash: number): string => {
        const useB = pickSide(t, hash);
        const pool = useB ? tilesB.walls : tilesA.walls;
        return pool[Math.floor(pickRand(hash + 999) * pool.length)] || 'hallway_wall';
      };
      const pickFloor = (t: number, hash: number): string => {
        const useB = pickSide(t, hash);
        const pool = useB ? tilesB.floors : tilesA.floors;
        return pool[Math.floor(pickRand(hash + 777) * pool.length)] || 'hallway_floor';
      };

      // Write walls (only on void tiles — room/level tiles are already filled)
      for (const wk of wallSet) {
        const [wx, wy] = wk.split(',').map(Number);
        if (getTile(wx, wy) === 'void') {
          const t = closestPathT(wx, wy);
          setTile(wx, wy, pickWall(t, wx * 31 + wy * 17));
        }
      }
      // Write floors (overwrite void and hallway walls, but never region tiles)
      for (const fk of floorSet) {
        const [fx, fy] = fk.split(',').map(Number);
        if (regionInterior.has(fx + ',' + fy)) continue;
        const cur = getTile(fx, fy);
        const isHallwayWall = tilesA.walls.includes(cur) || tilesB.walls.includes(cur) || cur === 'hallway_wall';
        if (cur === 'void' || isHallwayWall) {
          const t = closestPathT(fx, fy);
          setTile(fx, fy, pickFloor(t, fx * 31 + fy * 17));
        }
      }

      // Carve doorway openings where hallway floors meet region walls
      // For each hallway floor tile adjacent to a region interior, check if the
      // region tile is a wall and carve it to floor
      for (const fk of floorSet) {
        const [fx, fy] = fk.split(',').map(Number);
        for (const [dx, dy] of [[0, 1], [0, -1], [1, 0], [-1, 0]]) {
          const nx = fx + dx, ny = fy + dy;
          const nk = nx + ',' + ny;
          if (!regionInterior.has(nk)) continue;
          const cur = getTile(nx, ny);
          const def = tileDefs[cur];
          if (def && !def.walkable) {
            // Find the floor tile for this region
            const br = builderRegions.find(r =>
              nx >= r.ox && nx < r.ox + r.w && ny >= r.oy && ny < r.oy + r.h);
            if (br) {
              const ts = getNodeTiles(br.id);
              const floorId = ts.floors[0] || 'hallway_floor';
              setTile(nx, ny, floorId);
              // Also carve adjacent wall tiles for a 3-wide opening
              for (const [dx2, dy2] of (dx === 0 ? [[1, 0], [-1, 0]] : [[0, 1], [0, -1]])) {
                const ax = nx + dx2, ay = ny + dy2;
                const ac = getTile(ax, ay);
                const ad = tileDefs[ac];
                if (ad && !ad.walkable) setTile(ax, ay, floorId);
              }
            }
          }
        }
      }
    }
  }

  // Fill wall gaps at doorway junctions: void tiles adjacent (cardinal + diagonal)
  // to both wall and floor tiles get filled with wall to close gaps.
  // Iterate until stable since filling one gap may expose another.
  let gapFilled = true;
  while (gapFilled) {
    gapFilled = false;
    for (let y = 1; y < height - 1; y++) {
      for (let x = 1; x < width - 1; x++) {
        if (tiles[y][x] !== 'void') continue;
        let adjWall = 0, adjFloor = 0;
        let wallId = '';
        for (let dy = -1; dy <= 1; dy++) {
          for (let dx = -1; dx <= 1; dx++) {
            if (dx === 0 && dy === 0) continue;
            const n = tiles[y + dy]?.[x + dx];
            if (!n || n === 'void') continue;
            const nd = tileDefs[n];
            if (nd) {
              if (nd.walkable) adjFloor++;
              else { adjWall++; if (!wallId) wallId = n; }
            }
          }
        }
        if (adjWall >= 1 && adjFloor >= 1 && wallId) {
          tiles[y][x] = wallId;
          gapFilled = true;
        }
      }
    }
  }

  // Build region metadata from builder_regions
  const exportRegions: ExportedOverworldMap['regions'] = [];
  for (const br of builderRegions) {
    exportRegions.push({
      node_id: br.id,
      ox: br.ox - minX, oy: br.oy - minY,
      w: br.w, h: br.h,
    });
  }

  // Player start position
  const startBr = builderRegions.find(r => r.type === 'start');
  const sp = startBr || { ox: 0, oy: 0, w: 20, h: 15 };
  const playerPos: [number, number] = [
    Math.floor(sp.ox + sp.w / 2 - minX),
    Math.floor(sp.oy + sp.h / 2 - minY),
  ];

  // Export signposts with adjusted coordinates
  const signDefs = campaign.signposts || [];
  const placedSigns = campaign.overworld.placed_signposts || [];
  const exportedSignposts: ExportedSignpost[] = [];
  for (const ps of placedSigns) {
    const def = signDefs[ps.signpost_idx];
    if (!def) continue;
    exportedSignposts.push({
      title: def.title,
      description: def.description,
      title_font: def.title_font,
      description_font: def.description_font,
      x: ps.x - minX,
      y: ps.y - minY,
    });
  }

  return { width, height, tiles, tile_defs: tileDefs, regions: exportRegions, signposts: exportedSignposts.length > 0 ? exportedSignposts : undefined, player_pos: playerPos };
}
