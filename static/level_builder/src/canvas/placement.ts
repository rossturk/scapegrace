// Smart entity placement logic — ported from the original level builder.
// Handles bosses (2x2, farthest from player), doors (edge walls), keys (designated position).

import type { PlacedEntities, Phase2Result, MapGenResult } from '../types/pack';
import type { TrayItem } from '../components/level/entity-tray';

interface MapData {
  tiles: string[][];
  player_start: [number, number];
  boss_position: [number, number];
  key_position?: [number, number];
}

function buildOccupied(pe: PlacedEntities, mapData: MapData): Set<string> {
  const occupied = new Set<string>();
  for (const m of pe.monsters || []) occupied.add(m.x + ',' + m.y);
  for (const i of pe.items || []) occupied.add(i.x + ',' + i.y);
  for (const t of pe.traps || []) occupied.add(t.x + ',' + t.y);
  if (mapData.player_start) occupied.add(mapData.player_start[0] + ',' + mapData.player_start[1]);
  if (pe.boss) {
    const [bx, by] = pe.boss;
    for (let dy = 0; dy < 2; dy++)
      for (let dx = 0; dx < 2; dx++)
        occupied.add((bx + dx) + ',' + (by + dy));
  }
  return occupied;
}

// Edge wall tile adjacent to walkable, nearest to target position
function findDoorPosition(
  mapData: MapData,
  walkable: Set<string>,
  target: [number, number],
): [number, number] | null {
  const tiles = mapData.tiles;
  const rows = tiles.length;
  const cols = tiles[0].length;
  let best: [number, number] | null = null;
  let bestScore = Infinity;

  for (let y = 0; y < rows; y++) {
    for (let x = 0; x < cols; x++) {
      if (walkable.has(tiles[y][x])) continue; // skip walkable tiles, we want walls
      const onEdge = x === 0 || x === cols - 1 || y === 0 || y === rows - 1;
      const adj = [[x - 1, y], [x + 1, y], [x, y - 1], [x, y + 1]].some(([ax, ay]) =>
        ax >= 0 && ay >= 0 && ax < cols && ay < rows && walkable.has(tiles[ay][ax])
      );
      if (!adj) continue;
      const d = Math.abs(x - target[0]) + Math.abs(y - target[1]);
      const score = onEdge ? d - 1000 : d; // strong preference for edge tiles
      if (score < bestScore) { bestScore = score; best = [x, y]; }
    }
  }
  return best;
}

// Place a single tray item intelligently
function buildWalkableSet(design: Phase2Result): Set<string> {
  const defs = design.tile_defs || [];
  const walkable = new Set<string>();
  for (let i = 1; i < defs.length; i++) {
    walkable.add(`t${i}`);
    walkable.add(defs[i].name);
  }
  return walkable;
}

export function autoPlaceItem(
  item: TrayItem,
  mapData: MapData,
  pe: PlacedEntities,
  design: Phase2Result,
): boolean {
  const tiles = mapData.tiles;
  const rows = tiles.length;
  const cols = tiles[0].length;
  const walkable = buildWalkableSet(design);
  const occupied = buildOccupied(pe, mapData);

  const isOpen = (x: number, y: number) =>
    x >= 0 && y >= 0 && x < cols && y < rows &&
    walkable.has(tiles[y][x]) && !occupied.has(x + ',' + y);

  if (item.type === 'exit_door') {
    const bp = pe.boss || mapData.boss_position || [Math.floor(cols / 2), Math.floor(rows / 2)];
    pe.exit_door = findDoorPosition(mapData, walkable, bp as [number, number]);
    return !!pe.exit_door;
  }

  if (item.type === 'entry_door') {
    const ps = mapData.player_start || [Math.floor(cols / 2), Math.floor(rows / 2)];
    pe.entry_door = findDoorPosition(mapData, walkable, ps);
    return !!pe.entry_door;
  }

  if (item.type === 'boss') {
    const [px, py] = mapData.player_start || [0, 0];
    const spots: [number, number, number][] = [];
    for (let y = 0; y < rows - 1; y++) {
      for (let x = 0; x < cols - 1; x++) {
        if (isOpen(x, y) && isOpen(x + 1, y) && isOpen(x, y + 1) && isOpen(x + 1, y + 1)) {
          spots.push([x, y, (x - px) ** 2 + (y - py) ** 2]);
        }
      }
    }
    if (spots.length === 0) return false;
    spots.sort((a, b) => b[2] - a[2]); // farthest from player
    pe.boss = [spots[0][0], spots[0][1]];
    return true;
  }

  // Key: use designated position if available
  if (item.type === 'item' && item.item_type === 'key' && mapData.key_position) {
    const [kx, ky] = mapData.key_position;
    if (!occupied.has(kx + ',' + ky)) {
      pe.items.push({ name: item.name, item_type: item.item_type, x: kx, y: ky });
      return true;
    }
  }

  // Generic: random walkable tile
  const candidates: [number, number][] = [];
  for (let y = 0; y < rows; y++) {
    for (let x = 0; x < cols; x++) {
      if (isOpen(x, y)) candidates.push([x, y]);
    }
  }
  if (candidates.length === 0) return false;
  const [rx, ry] = candidates[Math.floor(Math.random() * candidates.length)];

  if (item.type === 'monster') pe.monsters.push({ name: item.name, x: rx, y: ry });
  else if (item.type === 'item') pe.items.push({ name: item.name, item_type: item.item_type, x: rx, y: ry });
  else if (item.type === 'trap') pe.traps.push({ name: item.name, x: rx, y: ry });

  return true;
}

// Place all tray items in smart order: boss first, then doors, then key, then rest
export function autoPlaceAll(
  tray: TrayItem[],
  mapData: MapData,
  pe: PlacedEntities,
  design: Phase2Result,
): number {
  const tiles = mapData.tiles;
  const rows = tiles.length;
  const cols = tiles[0].length;
  const walkable = buildWalkableSet(design);
  const occupied = buildOccupied(pe, mapData);

  const isOpen = (x: number, y: number) =>
    x >= 0 && y >= 0 && x < cols && y < rows &&
    walkable.has(tiles[y][x]) && !occupied.has(x + ',' + y);

  let placed = 0;

  // 1. Boss first (2x2, farthest from player)
  const bossTray = tray.find(t => t.type === 'boss');
  if (bossTray) {
    const [px, py] = mapData.player_start || [0, 0];
    const spots: [number, number, number][] = [];
    for (let y = 0; y < rows - 1; y++) {
      for (let x = 0; x < cols - 1; x++) {
        if (isOpen(x, y) && isOpen(x + 1, y) && isOpen(x, y + 1) && isOpen(x + 1, y + 1)) {
          spots.push([x, y, (x - px) ** 2 + (y - py) ** 2]);
        }
      }
    }
    if (spots.length > 0) {
      spots.sort((a, b) => b[2] - a[2]);
      const [bx, by] = spots[0];
      pe.boss = [bx, by];
      for (let dy = 0; dy < 2; dy++)
        for (let dx = 0; dx < 2; dx++)
          occupied.add((bx + dx) + ',' + (by + dy));
      placed++;
    }
  }

  // 2. Build open-tile list and shuffle
  const openTiles: [number, number][] = [];
  for (let y = 0; y < rows; y++) {
    for (let x = 0; x < cols; x++) {
      if (isOpen(x, y)) openTiles.push([x, y]);
    }
  }
  for (let i = openTiles.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [openTiles[i], openTiles[j]] = [openTiles[j], openTiles[i]];
  }

  let wi = 0;
  const keyPos = design.prebuilt_map?.key_position;

  for (const t of tray) {
    if (t.type === 'boss') continue; // already placed

    if (t.type === 'exit_door') {
      const bp = pe.boss || mapData.boss_position || [Math.floor(cols / 2), Math.floor(rows / 2)];
      pe.exit_door = findDoorPosition(mapData, walkable, bp as [number, number]);
      if (pe.exit_door) placed++;
      continue;
    }

    if (t.type === 'entry_door') {
      const ps = mapData.player_start || [Math.floor(cols / 2), Math.floor(rows / 2)];
      pe.entry_door = findDoorPosition(mapData, walkable, ps);
      if (pe.entry_door) placed++;
      continue;
    }

    // Key at designated position
    if (t.type === 'item' && t.item_type === 'key' && keyPos && !occupied.has(keyPos[0] + ',' + keyPos[1])) {
      pe.items.push({ name: t.name, item_type: t.item_type, x: keyPos[0], y: keyPos[1] });
      occupied.add(keyPos[0] + ',' + keyPos[1]);
      placed++;
      continue;
    }

    if (wi >= openTiles.length) break;
    const [rx, ry] = openTiles[wi++];
    if (t.type === 'monster') pe.monsters.push({ name: t.name, x: rx, y: ry });
    else if (t.type === 'item') pe.items.push({ name: t.name, item_type: t.item_type, x: rx, y: ry });
    else if (t.type === 'trap') pe.traps.push({ name: t.name, x: rx, y: ry });
    placed++;
  }

  return placed;
}
