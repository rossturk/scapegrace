import { useEffect, useRef } from 'preact/hooks';
import { signal } from '@preact/signals';
import type { Phase2Result, PlacedEntities } from '../../types/pack';
import type { BundledCampaign } from '../../types/pack';
import { drawLevelMap } from '../../canvas/level-renderer';
import { updateDesign } from '../../store/actions';
import { pack } from '../../store/state';
import type { TrayItem } from './entity-tray';

// Shared drag state — written by EntityTray, read by MapCanvas
export const draggedItem = signal<TrayItem | null>(null);

interface Props {
  campaign: BundledCampaign;
  levelIdx: number;
  design: Phase2Result;
  palette: string[];
}

interface MapDragState {
  type: string;
  index?: number;
  origX: number;
  origY: number;
}

export function MapCanvas({ campaign, levelIdx, design, palette }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const redrawTimerRef = useRef<number | null>(null);
  const mapDragRef = useRef<MapDragState | null>(null);
  const highlightRef = useRef<{ tx: number; ty: number } | null>(null);

  useEffect(() => {
    redraw();
    redrawTimerRef.current = window.setTimeout(redraw, 150);
    return () => {
      if (redrawTimerRef.current) clearTimeout(redrawTimerRef.current);
    };
  }, [design, palette, campaign]);

  function redraw(highlightTile?: { tx: number; ty: number; size?: number } | null) {
    const canvas = canvasRef.current;
    if (!canvas || !design.prebuilt_map) return;
    (globalThis as any).__packItemSprites = pack.value?.item_sprites || {};
    drawLevelMap(
      canvas, design.prebuilt_map, palette, design.tile_defs || [],
      design.placed_entities,
      { campaign, levelIdx },
    );
    // Draw highlight overlay
    const ht = highlightTile || highlightRef.current;
    if (ht) {
      const ctx = canvas.getContext('2d')!;
      const tiles = design.prebuilt_map.tiles;
      const cols = tiles[0]?.length || 1;
      const rows = tiles.length;
      const cellW = canvas.width / cols;
      const cellH = canvas.height / rows;
      const size = (ht as any).size || 1;
      ctx.fillStyle = 'rgba(102, 187, 106, 0.35)';
      ctx.fillRect(ht.tx * cellW, ht.ty * cellH, cellW * size, cellH * size);
      ctx.strokeStyle = '#66bb6a';
      ctx.lineWidth = 2;
      ctx.strokeRect(ht.tx * cellW + 1, ht.ty * cellH + 1, cellW * size - 2, cellH * size - 2);
    }
  }

  function tileAt(e: MouseEvent | DragEvent): { tx: number; ty: number } | null {
    if (!design.prebuilt_map) return null;
    const canvas = canvasRef.current!;
    const rect = canvas.getBoundingClientRect();
    const tiles = design.prebuilt_map.tiles;
    const rows = tiles.length;
    const cols = tiles[0]?.length || 0;
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    const cellW = canvas.width / cols;
    const cellH = canvas.height / rows;
    const tx = Math.floor((e.clientX - rect.left) * scaleX / cellW);
    const ty = Math.floor((e.clientY - rect.top) * scaleY / cellH);
    if (tx < 0 || tx >= cols || ty < 0 || ty >= rows) return null;
    return { tx, ty };
  }

  // ── Tray drag-and-drop (HTML5 drag) ──

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy';
    const pos = tileAt(e);
    if (pos) {
      const item = draggedItem.value;
      const size = item?.type === 'boss' ? 2 : 1;
      highlightRef.current = { tx: pos.tx, ty: pos.ty, size } as any;
      redraw(highlightRef.current);
    }
  }

  function handleDragLeave() {
    highlightRef.current = null;
    redraw();
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    highlightRef.current = null;
    let item = draggedItem.value;
    if (!item && e.dataTransfer) {
      try { item = JSON.parse(e.dataTransfer.getData('text/plain')); } catch {}
    }
    if (!item) return;
    const pos = tileAt(e);
    if (!pos) return;
    updateDesign(d => {
      if (!d.placed_entities) d.placed_entities = { monsters: [], items: [], traps: [] };
      const pe = d.placed_entities!;
      if (item!.type === 'boss') pe.boss = [pos.tx, pos.ty];
      else if (item!.type === 'exit_door') pe.exit_door = [pos.tx, pos.ty];
      else if (item!.type === 'entry_door') pe.entry_door = [pos.tx, pos.ty];
      else if (item!.type === 'monster') pe.monsters.push({ name: item!.name, x: pos.tx, y: pos.ty });
      else if (item!.type === 'item') pe.items.push({ name: item!.name, item_type: item!.item_type, x: pos.tx, y: pos.ty });
      else if (item!.type === 'trap') pe.traps.push({ name: item!.name, x: pos.tx, y: pos.ty });
    });
    draggedItem.value = null;
  }

  // ── Entity repositioning (mousedown/move/up on canvas) ──

  function handleMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    const pos = tileAt(e);
    if (!pos) return;
    const { tx, ty } = pos;
    const pe = design.placed_entities;
    if (!pe) return;

    // Check if clicking on an existing entity
    let drag: MapDragState | null = null;

    if (pe.boss) {
      const [bx, by] = pe.boss;
      if (tx >= bx && tx < bx + 2 && ty >= by && ty < by + 2) {
        drag = { type: 'boss', origX: bx, origY: by };
      }
    }
    if (!drag && pe.entry_door) {
      const [ex, ey] = pe.entry_door;
      if (tx === ex && ty === ey) drag = { type: 'entry_door', origX: ex, origY: ey };
    }
    if (!drag && pe.exit_door) {
      const [ex, ey] = pe.exit_door;
      if (tx === ex && ty === ey) drag = { type: 'exit_door', origX: ex, origY: ey };
    }
    if (!drag) {
      for (let i = 0; i < (pe.monsters || []).length; i++) {
        if (pe.monsters[i].x === tx && pe.monsters[i].y === ty) {
          drag = { type: 'monster', index: i, origX: tx, origY: ty }; break;
        }
      }
    }
    if (!drag) {
      for (let i = 0; i < (pe.items || []).length; i++) {
        if (pe.items[i].x === tx && pe.items[i].y === ty) {
          drag = { type: 'item', index: i, origX: tx, origY: ty }; break;
        }
      }
    }
    if (!drag) {
      for (let i = 0; i < (pe.traps || []).length; i++) {
        if (pe.traps[i].x === tx && pe.traps[i].y === ty) {
          drag = { type: 'trap', index: i, origX: tx, origY: ty }; break;
        }
      }
    }

    if (drag) {
      mapDragRef.current = drag;
      const canvas = canvasRef.current!;
      canvas.style.cursor = 'grabbing';
      e.preventDefault();
    }
  }

  function handleMouseMove(e: MouseEvent) {
    if (!mapDragRef.current) return;
    const pos = tileAt(e);
    if (!pos) return;
    const size = mapDragRef.current.type === 'boss' ? 2 : 1;
    highlightRef.current = { tx: pos.tx, ty: pos.ty, size } as any;
    redraw(highlightRef.current);
  }

  function handleMouseUp(e: MouseEvent) {
    const drag = mapDragRef.current;
    if (!drag) return;
    mapDragRef.current = null;
    highlightRef.current = null;
    const canvas = canvasRef.current!;
    canvas.style.cursor = 'default';

    const pos = tileAt(e);
    if (!pos) { redraw(); return; }
    const { tx, ty } = pos;

    if (tx === drag.origX && ty === drag.origY) { redraw(); return; }

    updateDesign(d => {
      const pe = d.placed_entities;
      if (!pe) return;
      if (drag.type === 'boss') pe.boss = [tx, ty];
      else if (drag.type === 'entry_door') pe.entry_door = [tx, ty];
      else if (drag.type === 'exit_door') pe.exit_door = [tx, ty];
      else if (drag.type === 'monster' && drag.index !== undefined) {
        pe.monsters[drag.index].x = tx;
        pe.monsters[drag.index].y = ty;
      } else if (drag.type === 'item' && drag.index !== undefined) {
        pe.items[drag.index].x = tx;
        pe.items[drag.index].y = ty;
      } else if (drag.type === 'trap' && drag.index !== undefined) {
        pe.traps[drag.index].x = tx;
        pe.traps[drag.index].y = ty;
      }
    });
  }

  // ── Right-click to remove ──

  function handleRightClick(e: MouseEvent) {
    e.preventDefault();
    if (!design.placed_entities) return;
    const pos = tileAt(e);
    if (!pos) return;
    const { tx, ty } = pos;

    updateDesign(d => {
      const pe = d.placed_entities;
      if (!pe) return;
      if (pe.boss && tx >= pe.boss[0] && tx < pe.boss[0] + 2 && ty >= pe.boss[1] && ty < pe.boss[1] + 2) { pe.boss = undefined; return; }
      if (pe.exit_door && pe.exit_door[0] === tx && pe.exit_door[1] === ty) { pe.exit_door = undefined; return; }
      if (pe.entry_door && pe.entry_door[0] === tx && pe.entry_door[1] === ty) { pe.entry_door = undefined; return; }
      const mi = pe.monsters.findIndex(m => m.x === tx && m.y === ty);
      if (mi >= 0) { pe.monsters.splice(mi, 1); return; }
      const ii = pe.items.findIndex(it => it.x === tx && it.y === ty);
      if (ii >= 0) { pe.items.splice(ii, 1); return; }
      const ti = pe.traps.findIndex(t => t.x === tx && t.y === ty);
      if (ti >= 0) { pe.traps.splice(ti, 1); return; }
    });
  }

  return (
    <canvas
      ref={canvasRef}
      class="map-canvas"
      width={900}
      height={540}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      onDragOver={handleDragOver as any}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop as any}
      onContextMenu={handleRightClick}
      style="cursor:default"
    />
  );
}
