// Pure level map canvas rendering — draws tilemap with entity overlays and sprites.

import type { Phase2Result, PlacedEntities, TileDefSlim, BundledCampaign } from '../types/pack';

const imageCache = new Map<string, HTMLImageElement>();

function loadImage(key: string, base64: string): HTMLImageElement | null {
  const cached = imageCache.get(key);
  if (cached?.complete && cached.naturalWidth > 0) return cached;
  if (!cached) {
    const img = new Image();
    img.src = `data:image/png;base64,${base64}`;
    imageCache.set(key, img);
    // Return null — caller will get it on next redraw
  }
  return imageCache.get(key)?.complete ? imageCache.get(key)! : null;
}

export interface LevelRenderContext {
  campaign: BundledCampaign;
  levelIdx: number;
}

export async function drawLevelMap(
  canvas: HTMLCanvasElement,
  mapData: { tiles: string[][]; player_start: [number, number]; boss_position: [number, number]; key_position?: [number, number] },
  palette: string[],
  tileDefs: TileDefSlim[],
  placedEntities?: PlacedEntities,
  renderCtx?: LevelRenderContext,
) {
  const tiles = mapData.tiles;
  if (!tiles || tiles.length === 0) return;

  const rows = tiles.length;
  const cols = tiles[0].length;
  const cellW = canvas.width / cols;
  const cellH = canvas.height / rows;
  const ctx = canvas.getContext('2d')!;

  // Build color/char/image maps
  const nameToColor: Record<string, string> = {};
  const nameToChar: Record<string, string> = {};
  for (let i = 0; i < tileDefs.length; i++) {
    const name = tileDefs[i].name;
    nameToColor[name] = palette[i % palette.length] || (i === 0 ? '#444' : '#333');
    if (tileDefs[i].char) nameToChar[name] = tileDefs[i].char;
    if (tileDefs[i].image) loadImage('tile_' + name, tileDefs[i].image!);
  }
  nameToColor['wall'] = nameToColor['wall'] || '#444';
  nameToColor['locked_door'] = '#aa6622';
  nameToChar['locked_door'] = '\u{1F512}';

  // Background
  ctx.fillStyle = '#000';
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  // Draw tiles
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const name = tiles[r][c];
      const img = imageCache.get('tile_' + name);
      if (img?.complete && img.naturalWidth > 0) {
        ctx.drawImage(img, c * cellW, r * cellH, cellW, cellH);
      } else {
        ctx.fillStyle = nameToColor[name] || '#222';
        ctx.fillRect(c * cellW, r * cellH, cellW, cellH);
        const ch = nameToChar[name];
        if (ch && cellW >= 6) {
          ctx.fillStyle = 'rgba(255,255,255,0.3)';
          ctx.font = `${Math.min(cellW, cellH) * 0.7}px monospace`;
          ctx.textAlign = 'center';
          ctx.textBaseline = 'middle';
          ctx.fillText(ch, c * cellW + cellW / 2, r * cellH + cellH / 2);
        }
      }
    }
  }

  // Player start
  if (mapData.player_start) {
    const [px, py] = mapData.player_start;
    ctx.fillStyle = '#66bb6a';
    ctx.fillRect(px * cellW + 2, py * cellH + 2, cellW - 4, cellH - 4);
  }

  // Boss room tint (suggested position, before placed)
  const pe = placedEntities;
  if (mapData.boss_position && !(pe?.boss)) {
    const [bx, by] = mapData.boss_position;
    const wallName = tileDefs[0]?.name || 'wall';
    const isWalkable = (x: number, y: number) =>
      x >= 0 && y >= 0 && x < cols && y < rows && tiles[y][x] !== wallName && tiles[y][x] !== 'locked_door';
    const isChokepoint = (x: number, y: number) => {
      const n = !isWalkable(x, y - 1), s = !isWalkable(x, y + 1);
      const e = !isWalkable(x + 1, y), w = !isWalkable(x - 1, y);
      return (n && s) || (e && w);
    };
    const visited = new Set<string>();
    const queue = [[bx, by]];
    visited.add(bx + ',' + by);
    while (queue.length > 0) {
      const [cx, cy] = queue.shift()!;
      for (const [dx, dy] of [[0, 1], [0, -1], [1, 0], [-1, 0]]) {
        const nx = cx + dx, ny = cy + dy, key = nx + ',' + ny;
        if (isWalkable(nx, ny) && !visited.has(key) && !isChokepoint(nx, ny)) {
          visited.add(key);
          queue.push([nx, ny]);
        }
      }
    }
    ctx.fillStyle = 'rgba(233,69,96,0.12)';
    for (const key of visited) {
      const [rx, ry] = key.split(',').map(Number);
      ctx.fillRect(rx * cellW, ry * cellH, cellW, cellH);
    }
  }

  // Key position hint
  const keyPlaced = pe?.items?.some(i => i.item_type === 'key');
  if (mapData.key_position && !keyPlaced) {
    const [kx, ky] = mapData.key_position;
    ctx.fillStyle = '#4dd0e1';
    ctx.fillRect(kx * cellW + 2, ky * cellH + 2, cellW - 4, cellH - 4);
  }

  // ── Draw placed entities with sprites ──
  if (!pe) return;

  ctx.font = `${Math.min(cellW, cellH) * 0.7}px sans-serif`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';

  const design = renderCtx
    ? renderCtx.campaign.designs[renderCtx.levelIdx]
    : null;
  const campaignMonsters = renderCtx?.campaign.monster_templates || [];
  const packSprites = renderCtx?.campaign ? (globalThis as any).__packItemSprites : null;

  // Boss
  if (pe.boss) {
    const [bx, by] = pe.boss;
    ctx.fillStyle = 'rgba(233,69,96,0.6)';
    ctx.fillRect(bx * cellW, by * cellH, cellW * 2, cellH * 2);
    const bossImg = design?.boss?.image;
    if (bossImg) {
      const img = loadImage('boss_sprite', bossImg);
      if (img) ctx.drawImage(img, bx * cellW, by * cellH, cellW * 2, cellH * 2);
    } else {
      ctx.fillStyle = '#fff';
      ctx.fillText('\u{1F480}', bx * cellW + cellW, by * cellH + cellH);
    }
    ctx.strokeStyle = '#e94560';
    ctx.lineWidth = 2;
    ctx.strokeRect(bx * cellW, by * cellH, cellW * 2, cellH * 2);
  }

  // Monsters
  for (const m of pe.monsters || []) {
    const tmpl = campaignMonsters.find(t => t.name === m.name);
    ctx.fillStyle = 'rgba(255,105,180,0.6)';
    ctx.fillRect(m.x * cellW, m.y * cellH, cellW, cellH);
    if (tmpl?.image) {
      const img = loadImage('mon_' + m.name, tmpl.image);
      if (img) ctx.drawImage(img, m.x * cellW + 1, m.y * cellH + 1, cellW - 2, cellH - 2);
    } else {
      ctx.fillStyle = '#fff';
      ctx.fillText('\u{1F47E}', m.x * cellW + cellW / 2, m.y * cellH + cellH / 2);
    }
  }

  // Items
  const itemIcons: Record<string, string> = { potion: '\u{1F9EA}', gold: '\u{1F4B0}', weapon: '\u2694', armor: '\u{1F6E1}', key: '\u{1F511}', speed_potion: '\u26A1', bomb: '\u{1F4A3}' };
  const itemColors: Record<string, string> = { key: 'rgba(77,208,225,0.6)' };
  for (const it of pe.items || []) {
    const sprite = packSprites?.[it.item_type] || null;
    ctx.fillStyle = itemColors[it.item_type] || 'rgba(255,167,38,0.5)';
    ctx.fillRect(it.x * cellW, it.y * cellH, cellW, cellH);
    if (sprite) {
      const img = loadImage('item_' + it.item_type, sprite);
      if (img) ctx.drawImage(img, it.x * cellW + 1, it.y * cellH + 1, cellW - 2, cellH - 2);
    } else {
      ctx.fillStyle = '#fff';
      ctx.fillText(itemIcons[it.item_type] || '\u{1F4E6}', it.x * cellW + cellW / 2, it.y * cellH + cellH / 2);
    }
  }

  // Traps
  for (const t of pe.traps || []) {
    const trapDef = design?.traps?.find(td => td.name === t.name);
    ctx.fillStyle = 'rgba(171,71,188,0.5)';
    ctx.fillRect(t.x * cellW, t.y * cellH, cellW, cellH);
    if (trapDef?.image) {
      const img = loadImage('trap_' + t.name, trapDef.image);
      if (img) ctx.drawImage(img, t.x * cellW + 1, t.y * cellH + 1, cellW - 2, cellH - 2);
    } else {
      ctx.fillStyle = '#fff';
      ctx.fillText('\u26A0', t.x * cellW + cellW / 2, t.y * cellH + cellH / 2);
    }
  }

  // Exit door
  if (pe.exit_door) {
    const [ex, ey] = pe.exit_door;
    ctx.fillStyle = 'rgba(255,204,0,0.7)';
    ctx.fillRect(ex * cellW, ey * cellH, cellW, cellH);
    ctx.fillStyle = '#fff';
    ctx.fillText('\u{1F6AA}', ex * cellW + cellW / 2, ey * cellH + cellH / 2);
  }

  // Entry door
  if (pe.entry_door) {
    const [ex, ey] = pe.entry_door;
    ctx.fillStyle = 'rgba(68,204,255,0.7)';
    ctx.fillRect(ex * cellW, ey * cellH, cellW, cellH);
    ctx.fillStyle = '#fff';
    ctx.fillText('\u{1F535}', ex * cellW + cellW / 2, ey * cellH + cellH / 2);
  }
}
