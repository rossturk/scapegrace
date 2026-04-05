// A* pathfinding for hallway routing between level regions

export function astarHallway(
  sx: number, sy: number,
  ex: number, ey: number,
  occupied: Set<string>,
): number[][] {
  const key = (x: number, y: number) => x + ',' + y;
  const open: { x: number; y: number; g: number; f: number }[] = [
    { x: sx, y: sy, g: 0, f: Math.abs(ex - sx) + Math.abs(ey - sy) },
  ];
  const closed = new Set<string>();
  const cameFrom: Record<string, string> = {};
  const gScore: Record<string, number> = {};
  gScore[key(sx, sy)] = 0;

  const isBlocked = (x: number, y: number) => {
    if (x === sx && y === sy) return false;
    if (x === ex && y === ey) return false;
    return occupied.has(key(x, y));
  };

  let iterations = 0;
  while (open.length > 0 && iterations < 50000) {
    iterations++;
    let bestIdx = 0;
    for (let i = 1; i < open.length; i++) {
      if (open[i].f < open[bestIdx].f) bestIdx = i;
    }
    const cur = open.splice(bestIdx, 1)[0];

    if (cur.x === ex && cur.y === ey) {
      const path: number[][] = [];
      let k: string | undefined = key(cur.x, cur.y);
      while (k) {
        const [px, py] = k.split(',').map(Number);
        path.unshift([px, py]);
        k = cameFrom[k];
      }
      return path;
    }

    closed.add(key(cur.x, cur.y));

    for (const [dx, dy] of [[0, 1], [0, -1], [1, 0], [-1, 0]]) {
      const nx = cur.x + dx, ny = cur.y + dy;
      const nk = key(nx, ny);
      if (closed.has(nk) || isBlocked(nx, ny)) continue;
      const ng = cur.g + 1;
      if (gScore[nk] !== undefined && ng >= gScore[nk]) continue;
      gScore[nk] = ng;
      cameFrom[nk] = key(cur.x, cur.y);
      const h = Math.abs(ex - nx) + Math.abs(ey - ny);
      open.push({ x: nx, y: ny, g: ng, f: ng + h });
    }
  }

  // Fallback: straight line
  const path: number[][] = [];
  let cx = sx, cy = sy;
  while (cx !== ex || cy !== ey) {
    if (cx !== ex) cx += cx < ex ? 1 : -1;
    else cy += cy < ey ? 1 : -1;
    path.push([cx, cy]);
  }
  return path;
}

export function findEdgeTileFromDesign(
  design: any,
  ox: number, oy: number,
  w: number, h: number,
  targetX: number, targetY: number,
): { x: number; y: number } {
  const tiles = design?.prebuilt_map?.tiles;
  if (tiles) {
    const defs = design.tile_defs || [];
    const walkableSet = new Set<string>();
    for (let i = 1; i < defs.length; i++) { walkableSet.add(`t${i}`); walkableSet.add(defs[i].name); }
    const rows = tiles.length, cols = tiles[0]?.length || w;
    let best: { x: number; y: number } | null = null;
    let bestDist = Infinity;
    const edgeTiles: number[][] = [];
    for (let x = 0; x < cols; x++) { edgeTiles.push([x, 0]); edgeTiles.push([x, rows - 1]); }
    for (let y = 1; y < rows - 1; y++) { edgeTiles.push([0, y]); edgeTiles.push([cols - 1, y]); }
    for (const [lx, ly] of edgeTiles) {
      if (!walkableSet.has(tiles[ly][lx])) {
        const hasAdj = [[0, 1], [0, -1], [1, 0], [-1, 0]].some(([dx, dy]) => {
          const nx = lx + dx, ny = ly + dy;
          return nx >= 0 && ny >= 0 && nx < cols && ny < rows && walkableSet.has(tiles[ny][nx]);
        });
        if (hasAdj) {
          const wx = ox + lx, wy = oy + ly;
          const d = Math.abs(wx - targetX) + Math.abs(wy - targetY);
          if (d < bestDist) { bestDist = d; best = { x: wx, y: wy }; }
        }
      }
    }
    if (best) return best;
  }
  // Fallback: closest edge center
  const edges = [
    { x: ox + w - 1, y: oy + Math.floor(h / 2) },
    { x: ox, y: oy + Math.floor(h / 2) },
    { x: ox + Math.floor(w / 2), y: oy + h - 1 },
    { x: ox + Math.floor(w / 2), y: oy },
  ];
  edges.sort((a, b) =>
    (Math.abs(a.x - targetX) + Math.abs(a.y - targetY)) -
    (Math.abs(b.x - targetX) + Math.abs(b.y - targetY))
  );
  return edges[0];
}
