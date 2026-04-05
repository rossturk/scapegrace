import { useEffect, useRef, useState } from 'preact/hooks';
import { signal } from '@preact/signals';
import type { BundledCampaign } from '../../types/pack';
import { drawOverworld, createOwCanvasState, computeGridBounds } from '../../canvas/overworld-renderer';
import { OverworldInteraction, type PopupInfo } from '../../canvas/overworld-interaction';
import { navigate, updateOverworld, updateCampaign } from '../../store/actions';
import { showToast } from '../toast';
import { pack } from '../../store/state';
import { OverworldTray, generateOverworldTray, draggedOverworldItem } from './overworld-tray';

export const owState = createOwCanvasState();
export const selectedNode = signal<number | string | null>(null);
const mapDataVersion = signal(0);

// Module-level redraw function and campaign ref, set by the component
let _moduleRedraw: (() => void) | null = null;
export function triggerRedraw() { _moduleRedraw?.(); }
export const campaignRef: { current: BundledCampaign | null } = { current: null };

interface Props {
  campaign: BundledCampaign;
}

export function OverworldCanvas({ campaign }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const interactionRef = useRef<OverworldInteraction | null>(null);
  const redrawRef = useRef<(() => void) | null>(null);
  const [popup, setPopup] = useState<PopupInfo | null>(null);

  // Always keep module-level ref current
  campaignRef.current = campaign;

  // Initialize builder_regions if not present (migrate or create default)
  useEffect(() => {
    if (!campaign.overworld.builder_regions) {
      import('../../canvas/region-migration').then(async ({ migrateToBuilderRegions, createDefaultRegions }) => {
        let regions;
        try {
          const { api } = await import('../../api/client');
          const data = await api(`/api/overworld-map?id=${campaign.id}`);
          regions = data ? migrateToBuilderRegions(campaign, data) : createDefaultRegions();
        } catch {
          regions = createDefaultRegions();
        }
        // Persist to pack via updateOverworld so it saves
        console.log('Migration: persisting builder_regions, count:', regions.length);
        updateOverworld(ow => {
          console.log('updateOverworld callback fired, setting builder_regions');
          ow.builder_regions = regions;
        });
        // Also set on campaignRef for immediate rendering
        campaignRef.current.overworld.builder_regions = regions;
        owState.hallwayCacheKey = null;
        mapDataVersion.value++;
      });
    } else {
      owState.hallwayCacheKey = null;
      mapDataVersion.value++;
    }
  }, [campaign.id]);

  // Ensure pack sprites are accessible to the renderer
  (globalThis as any).__packItemSprites = pack.value?.item_sprites || {};

  const _mdv = mapDataVersion.value;
  useEffect(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container) return;

    // redraw always reads the latest campaign from the ref
    const redraw = () => {
      const rect = container.getBoundingClientRect();
      if (rect.width === 0 || rect.height === 0) return;
      canvas.width = rect.width * devicePixelRatio;
      canvas.height = rect.height * devicePixelRatio;
      canvas.style.width = rect.width + 'px';
      canvas.style.height = rect.height + 'px';
      const ctx = canvas.getContext('2d')!;
      ctx.scale(devicePixelRatio, devicePixelRatio);
      drawOverworld(ctx, rect.width, rect.height, campaignRef.current, owState, selectedNode.value as any);
    };

    redrawRef.current = redraw;
    _moduleRedraw = redraw;
    redraw();

    // Auto zoom-fit on first render
    const br = campaign.overworld.builder_regions || [];
    if (br.length > 0 && owState.zoom === 1 && owState.panX === 0 && owState.panY === 0) {
      zoomFit(container, campaign);
      redraw();
    }

    if (interactionRef.current) {
      interactionRef.current.updateCampaign(campaign);
    } else {
      interactionRef.current = new OverworldInteraction(canvas, campaign, owState, {
        onRedraw: redraw,
        onSelectNode: (nodeIdx) => { selectedNode.value = nodeIdx; redraw(); },
        onOpenLevel: (levelIdx) => navigate(campaign.id, levelIdx),
        onNodeResized: (nodeId, w, h) => {
          const ow = campaignRef.current.overworld;
          const levels = ow.levels || [];
          let brId: string;
          if (typeof nodeId === 'string') brId = nodeId;
          else if (nodeId === 0) brId = 'start';
          else brId = nodeId > levels.length ? 'store' : `level_${nodeId - 1}`;
          const br = ow.builder_regions?.find(r => r.id === brId);
          if (br) { br.w = w; br.h = h; }
        },
        onWaypointDeleted: (connKey, wpIdx) => {
          updateOverworld(ow => {
            if (ow.hallway_waypoints?.[connKey]) {
              ow.hallway_waypoints[connKey].splice(wpIdx, 1);
              if (ow.hallway_waypoints[connKey].length === 0) delete ow.hallway_waypoints[connKey];
            }
          });
          owState.hallwayCacheKey = null;
          redrawRef.current?.();
        },
        onWaypointMoved: (connKey, wpIdx, tx, ty) => {
          const c = campaignRef.current;
          if (!c.overworld.hallway_waypoints?.[connKey]) return;
          c.overworld.hallway_waypoints[connKey][wpIdx] = [tx, ty];
        },
        onConnectionCreated: (fromId, toId) => {
          updateOverworld(ow => {
            if (!ow.connections) ow.connections = [];
            const exists = ow.connections.some(([a, b]) =>
              (a === fromId && b === toId) || (a === toId && b === fromId));
            if (!exists) {
              ow.connections.push([fromId, toId]);
              showToast(`Connected ${fromId} \u2192 ${toId}`, 'success');
            }
          });
          owState.hallwayCacheKey = null; // invalidate
          redraw();
        },
        onShowPopup: (p) => { setPopup(p); },
        onRegionMoved: () => {
          // Sync builder_regions positions to the pack for persistence
          updateOverworld(ow => {
            const liveBr = campaignRef.current.overworld.builder_regions;
            if (liveBr) ow.builder_regions = liveBr.map(r => ({ ...r }));
          });
        },
        onSignpostMoved: (idx, x, y) => {
          updateOverworld(ow => {
            if (ow.placed_signposts?.[idx]) {
              ow.placed_signposts[idx].x = x;
              ow.placed_signposts[idx].y = y;
            }
          });
        },
        onToast: showToast,
      });
    }

    const ro = new ResizeObserver(() => redraw());
    ro.observe(container);
    return () => { ro.disconnect(); };
  }, [campaign, _mdv]);

  useEffect(() => {
    return () => { interactionRef.current?.destroy(); interactionRef.current = null; };
  }, []);

  function deleteConnection(connIdx: number) {
    const conns = campaign.overworld.connections || [];
    const removed = conns[connIdx];
    updateOverworld(ow => {
      if (ow.connections) {
        const conn = ow.connections[connIdx];
        if (conn && ow.one_way_connections) {
          const key = `${conn[0]}->${conn[1]}`;
          ow.one_way_connections = ow.one_way_connections.filter(k => k !== key);
        }
        ow.connections.splice(connIdx, 1);
      }
    });
    if (removed) showToast(`Removed ${removed[0]} \u2192 ${removed[1]}`, 'info');
    setPopup(null);
    owState.hallwayCacheKey = null;
    redrawRef.current?.();
  }

  function toggleOneWay(connIdx: number) {
    const conns = campaign.overworld.connections || [];
    const conn = conns[connIdx];
    if (!conn) return;
    const key = `${conn[0]}->${conn[1]}`;
    updateOverworld(ow => {
      if (!ow.one_way_connections) ow.one_way_connections = [];
      const idx = ow.one_way_connections.indexOf(key);
      if (idx >= 0) {
        ow.one_way_connections.splice(idx, 1);
        showToast('Two-way', 'info');
      } else {
        ow.one_way_connections.push(key);
        showToast('One-way', 'success');
      }
    });
    setPopup(null);
    owState.hallwayCacheKey = null;
    redrawRef.current?.();
  }

  function deleteNode(nodeIdx: number | string) {
    if (typeof nodeIdx === 'string') {
      updateOverworld(ow => {
        if (ow.rooms) {
          ow.rooms = ow.rooms.filter(f => f.id !== nodeIdx);
        }
        if (ow.builder_regions) {
          ow.builder_regions = ow.builder_regions.filter(r => r.id !== nodeIdx);
        }
        if (ow.connections) {
          ow.connections = ow.connections.filter(([a, b]) => a !== nodeIdx && b !== nodeIdx);
        }
      });
      showToast('Room deleted', 'info');
    } else if (typeof nodeIdx === 'number' && nodeIdx > 0) {
      const levels = campaign.overworld.levels || [];
      const levelsIdx = nodeIdx - 1;
      if (levelsIdx < levels.length) {
        const levelId = `level_${levelsIdx}`;
        updateCampaign(c => {
          c.overworld.levels.splice(levelsIdx, 1);
          c.designs.splice(levelsIdx, 1);
          if (c.overworld.builder_regions) {
            c.overworld.builder_regions = c.overworld.builder_regions.filter(r => r.id !== levelId);
            // Re-index remaining levels
            for (const br of c.overworld.builder_regions) {
              if (br.type === 'level' && br.level_idx !== undefined && br.level_idx > levelsIdx) {
                br.level_idx--;
                br.id = `level_${br.level_idx}`;
              }
            }
          }
          if (c.overworld.connections) {
            c.overworld.connections = c.overworld.connections.filter(
              ([a, b]) => a !== levelId && b !== levelId
            );
          }
        });
        showToast(`Deleted level ${levelsIdx + 1}`, 'info');
      }
    }
    setPopup(null);
    owState.hallwayCacheKey = null;
    redrawRef.current?.();
  }

  function addWaypoint(connIdx: number) {
    const conns = campaign.overworld.connections || [];
    const conn = conns[connIdx];
    if (!conn) return;
    const connKey = `${conn[0]}->${conn[1]}`;
    // Find midpoint of the cached hallway path
    const path = owState.hallwayCache?.get(connKey);
    let wx: number, wy: number;
    if (path && path.length > 0) {
      const mid = path[Math.floor(path.length / 2)];
      wx = mid[0]; wy = mid[1];
    } else {
      wx = 0; wy = 0;
    }
    updateOverworld(ow => {
      if (!ow.hallway_waypoints) ow.hallway_waypoints = {};
      if (!ow.hallway_waypoints[connKey]) ow.hallway_waypoints[connKey] = [];
      ow.hallway_waypoints[connKey].push([wx, wy]);
    });
    owState.hallwayCacheKey = null;
    setPopup(null);
    redrawRef.current?.();
  }

  function renderPopup() {
    if (!popup) return null;
    const levels = campaign.overworld.levels || [];
    const oneWaySet = new Set(campaign.overworld.one_way_connections || []);

    const sx = popup.x;
    const sy = popup.y;

    if (popup.type === 'connection' && popup.connIdx !== undefined) {
      const conn = (campaign.overworld.connections || [])[popup.connIdx];
      if (!conn) return null;
      const key = `${conn[0]}->${conn[1]}`;
      const isOneWay = oneWaySet.has(key);
      return (
        <div class="dag-popup" style={`left:${sx}px;top:${sy}px;`}>
          <div class="dag-popup-conn">
            <div class="flex gap-4">
              <button style="font-size:11px;padding:4px 10px;" class="danger" onClick={() => deleteConnection(popup.connIdx!)}>Delete</button>
              <button style="font-size:11px;padding:4px 10px;" onClick={() => toggleOneWay(popup.connIdx!)}>
                {isOneWay ? '\u2194 Two-way' : '\u2192 One-way'}
              </button>
              <button style="font-size:11px;padding:4px 10px;" onClick={() => addWaypoint(popup.connIdx!)}>+ Bend</button>
            </div>
          </div>
        </div>
      );
    }

    if (popup.type === 'node' && popup.nodeIdx !== undefined) {
      const ni = popup.nodeIdx;
      const isRoom = typeof ni === 'string';
      const isLevel = typeof ni === 'number' && ni > 0;
      const isStart = ni === 0;
      const isStore = typeof ni === 'number' && ni > 0 && ni > levels.length;
      const levelsIdx = typeof ni === 'number' ? ni - 1 : -1;
      const level = isLevel && !isStore && levelsIdx < levels.length ? levels[levelsIdx] : null;

      // Size controls — read from builder_regions
      const brId = isRoom ? String(ni) : isStart ? 'start' : 'store';
      const brReg = campaign.overworld.builder_regions?.find(r => r.id === brId);
      const canResize = isRoom || isStart || isStore;
      const curW = brReg?.w || 10;
      const curH = brReg?.h || 8;

      const setSize = (w: number, h: number) => {
        updateOverworld(ow => {
          const br2 = ow.builder_regions?.find(r => r.id === brId);
          if (br2) { br2.w = w; br2.h = h; }
        });
        // Also update live ref for immediate rendering
        const liveBr = campaignRef.current.overworld.builder_regions?.find(r => r.id === brId);
        if (liveBr) { liveBr.w = w; liveBr.h = h; }
        owState.hallwayCacheKey = null;
        redrawRef.current?.();
      };

      const pw = Math.max(popup.nodeW || 100, 220);
      return (
        <div class="dag-popup" style={`left:${sx - pw / 2}px;top:${sy}px;width:${pw}px;`}>
          <div class="dag-popup-node">
            <div class="flex gap-4 items-center" style="justify-content:center;flex-wrap:wrap;">
            {level && (
              <>
                <span style="font-size:11px;color:#888;">Budget</span>
                <input type="number" value={level.budget} min={50} max={500}
                  style="width:55px;font-size:11px;padding:2px 4px;"
                  onChange={(e) => {
                    updateOverworld(ow => { ow.levels[levelsIdx].budget = +(e.target as HTMLInputElement).value; });
                    redrawRef.current?.();
                  }} />
              </>
            )}
            {canResize && (
              <>
                <span style="font-size:11px;color:#888;">Tiles</span>
                <select style="font-size:11px;padding:2px;max-width:100px;"
                  value={brReg?.tile_source || ''}
                  onChange={(e) => {
                    const val = (e.target as HTMLSelectElement).value || undefined;
                    updateOverworld(ow => {
                      const br2 = ow.builder_regions?.find(r => r.id === brId);
                      if (br2) br2.tile_source = val;
                    });
                    // Also update the live campaignRef copy for immediate rendering
                    const liveBr = campaignRef.current.overworld.builder_regions?.find(r => r.id === brId);
                    if (liveBr) liveBr.tile_source = val;
                    owState.hallwayCacheKey = null;
                    redrawRef.current?.();
                  }}
                >
                  <option value="">Default</option>
                  {levels.map((lv, i) => (
                    <option key={i} value={`level_${i}`}>{lv.name}</option>
                  ))}
                </select>
              </>
            )}
            {(isLevel || isRoom) && (
              <button style="font-size:11px;padding:4px 10px;" class="danger" onClick={() => deleteNode(ni)}>Delete</button>
            )}
            </div>
          </div>
        </div>
      );
    }

    return null;
  }

  const owTray = generateOverworldTray(campaign);

  function screenToTile(e: MouseEvent | DragEvent): { tx: number; ty: number } | null {
    const canvas = canvasRef.current;
    if (!canvas) return null;
    const rect = canvas.getBoundingClientRect();
    const mx = e.clientX - rect.left;
    const my = e.clientY - rect.top;
    const br = campaign.overworld.builder_regions || [];
    if (br.length === 0) return null;
    const bounds = computeGridBounds(br);
    const tz = 4 * owState.zoom;
    const baseOx = (rect.width - bounds.width * tz) / 2 + owState.panX;
    const baseOy = (rect.height - bounds.height * tz) / 2 + owState.panY;
    return { tx: Math.floor((mx - baseOx) / tz), ty: Math.floor((my - baseOy) / tz) };
  }

  function hitSignpost(e: MouseEvent): number {
    const tile = screenToTile(e);
    if (!tile) return -1;
    const placed = campaign.overworld.placed_signposts || [];
    return placed.findIndex(p => Math.abs(p.x - tile.tx) <= 1 && Math.abs(p.y - tile.ty) <= 1);
  }

  function handleCanvasDrop(e: DragEvent) {
    e.preventDefault();
    const item = draggedOverworldItem.value;
    if (!item) return;
    draggedOverworldItem.value = null;

    const tile = screenToTile(e);
    if (!tile) return;

    if (item.type === 'signpost' && item.signpost_idx !== undefined) {
      updateOverworld(ow => {
        if (!ow.placed_signposts) ow.placed_signposts = [];
        ow.placed_signposts.push({ signpost_idx: item.signpost_idx!, x: tile.tx, y: tile.ty });
      });
      showToast(`Placed "${item.name}"`, 'success');
      redrawRef.current?.();
    }
  }

  return (
    <div ref={containerRef} class="dag-container" onClick={(e) => {
      // Dismiss popup when clicking container background (not canvas)
      if (e.target === containerRef.current) setPopup(null);
    }}>
      <OverworldTray items={owTray} />
      <canvas
        ref={canvasRef}
        onDragOver={(e) => e.preventDefault()}
        onDrop={handleCanvasDrop as any}
        onContextMenu={(e) => {
          e.preventDefault();
          const idx = hitSignpost(e as any);
          if (idx >= 0) {
            const placed = campaign.overworld.placed_signposts || [];
            const sign = campaign.signposts?.[placed[idx].signpost_idx];
            if (confirm(`Remove signpost "${sign?.title || 'Unknown'}"?`)) {
              updateOverworld(ow => { ow.placed_signposts?.splice(idx, 1); });
              redrawRef.current?.();
            }
          }
        }}
      />
      {renderPopup()}
    </div>
  );
}

export function zoomFit(container: HTMLElement, campaign?: BundledCampaign) {
  const br = campaign?.overworld.builder_regions || campaignRef.current?.overworld.builder_regions || [];
  if (br.length === 0) return;
  const rect = container.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) return;
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const r of br) {
    minX = Math.min(minX, r.ox); minY = Math.min(minY, r.oy);
    maxX = Math.max(maxX, r.ox + r.w); maxY = Math.max(maxY, r.oy + r.h);
  }
  const totalW = (maxX - minX) || 1;
  const totalH = (maxY - minY) || 1;
  owState.zoom = Math.min(rect.width / (totalW * 4), rect.height / (totalH * 4)) * 0.85;
  owState.panX = -(minX * 4 * owState.zoom);
  owState.panY = -(minY * 4 * owState.zoom);
}

export function zoom1to1() {
  owState.zoom = 3.25;
  owState.panX = 0;
  owState.panY = 0;
}

// Get center of current view in tile coordinates (for placing new nodes)
export function getViewCenter(): { ox: number; oy: number } {
  const container = document.querySelector('.dag-container') as HTMLElement;
  if (!container) return { ox: 0, oy: 0 };
  const rect = container.getBoundingClientRect();
  const tz = 4 * owState.zoom;
  const br = campaignRef.current?.overworld.builder_regions || [];
  const bounds = computeGridBounds(br);
  const baseOx = (rect.width - bounds.width * tz) / 2 + owState.panX;
  const baseOy = (rect.height - bounds.height * tz) / 2 + owState.panY;
  const cx = (rect.width / 2 - baseOx) / tz;
  const cy = (rect.height / 2 - baseOy) / tz;
  return { ox: Math.round(cx), oy: Math.round(cy) };
}
