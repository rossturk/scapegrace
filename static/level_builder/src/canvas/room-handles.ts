// Compute dynamic door handle positions for rooms based on connections.
// Handles appear on whichever wall is closest to the connected node.
// Always includes one free entry + one free exit handle for new connections.

import type { Room } from '../types/pack';

export interface RoomHandle {
  type: 'entry' | 'exit';
  /** Tile position local to the room */
  lx: number;
  ly: number;
  /** Connection key (e.g. "level_2->room_abc") or null for free handle */
  connKey: string | null;
}

// Given a room rect and a target point (tile coords), find the best wall point
function nearestWallPoint(
  roomOx: number, roomOy: number, roomW: number, roomH: number,
  targetOx: number, targetOy: number,
): { lx: number; ly: number } {
  // Clamp target's Y to room range for left/right walls, X for top/bottom
  const clampY = Math.max(1, Math.min(roomH - 2, targetOy - roomOy));
  const clampX = Math.max(1, Math.min(roomW - 2, targetOx - roomOx));

  // Four candidate wall points
  const candidates = [
    { lx: 0, ly: clampY, dist: Math.abs(targetOx - roomOx) },                           // left
    { lx: roomW - 1, ly: clampY, dist: Math.abs(targetOx - (roomOx + roomW - 1)) },     // right
    { lx: clampX, ly: 0, dist: Math.abs(targetOy - roomOy) },                            // top
    { lx: clampX, ly: roomH - 1, dist: Math.abs(targetOy - (roomOy + roomH - 1)) },     // bottom
  ];
  candidates.sort((a, b) => a.dist - b.dist);
  return { lx: candidates[0].lx, ly: candidates[0].ly };
}

export function computeRoomHandles(
  room: Room,
  roomPos: { ox: number; oy: number },
  connections: [string, string][],
  /** Resolve any node ID to its center tile position, or null */
  resolveCenter: (id: string) => { ox: number; oy: number } | null,
): RoomHandle[] {
  const handles: RoomHandle[] = [];
  const roomW = room.w || 10;
  const roomH = room.h || 8;
  const usedWallPoints = new Set<string>();

  for (const [a, b] of connections) {
    const isEntry = b === room.id;  // something connects TO this room
    const isExit = a === room.id;   // this room connects TO something
    if (!isEntry && !isExit) continue;

    const otherId = isEntry ? a : b;
    const otherCenter = resolveCenter(otherId);
    if (!otherCenter) continue;

    const { lx, ly } = nearestWallPoint(roomPos.ox, roomPos.oy, roomW, roomH, otherCenter.ox, otherCenter.oy);
    const key = `${lx},${ly}`;
    // Nudge if this wall point is already taken
    let finalLx = lx, finalLy = ly;
    if (usedWallPoints.has(key)) {
      // Shift along the wall by 2 tiles
      if (lx === 0 || lx === roomW - 1) finalLy = Math.min(roomH - 2, ly + 2);
      else finalLx = Math.min(roomW - 2, lx + 2);
    }
    usedWallPoints.add(`${finalLx},${finalLy}`);

    handles.push({
      type: isEntry ? 'entry' : 'exit',
      lx: finalLx,
      ly: finalLy,
      connKey: `${a}->${b}`,
    });
  }

  // Add one free entry and one free exit handle on walls that aren't crowded
  const freeEntry = findFreeWallPoint(roomW, roomH, usedWallPoints, 'left');
  handles.push({ type: 'entry', lx: freeEntry.lx, ly: freeEntry.ly, connKey: null });
  const freeExit = findFreeWallPoint(roomW, roomH, usedWallPoints, 'right');
  handles.push({ type: 'exit', lx: freeExit.lx, ly: freeExit.ly, connKey: null });

  return handles;
}

function findFreeWallPoint(
  w: number, h: number,
  used: Set<string>,
  preferWall: 'left' | 'right' | 'top' | 'bottom',
): { lx: number; ly: number } {
  // Try preferred wall center first
  const wallCenters: Record<string, { lx: number; ly: number }> = {
    left: { lx: 0, ly: Math.floor(h / 2) },
    right: { lx: w - 1, ly: Math.floor(h / 2) },
    top: { lx: Math.floor(w / 2), ly: 0 },
    bottom: { lx: Math.floor(w / 2), ly: h - 1 },
  };
  const pref = wallCenters[preferWall];
  if (!used.has(`${pref.lx},${pref.ly}`)) {
    used.add(`${pref.lx},${pref.ly}`);
    return pref;
  }
  // Try other positions on the preferred wall
  if (preferWall === 'left' || preferWall === 'right') {
    const x = preferWall === 'left' ? 0 : w - 1;
    for (let y = 1; y < h - 1; y++) {
      if (!used.has(`${x},${y}`)) { used.add(`${x},${y}`); return { lx: x, ly: y }; }
    }
  } else {
    const y = preferWall === 'top' ? 0 : h - 1;
    for (let x = 1; x < w - 1; x++) {
      if (!used.has(`${y === 0 ? x : x},${y}`)) { used.add(`${x},${y}`); return { lx: x, ly: y }; }
    }
  }
  // Fallback
  return pref;
}
