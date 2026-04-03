import { useEffect, useRef, useState } from 'preact/hooks';
import { signal } from '@preact/signals';
import type { BundledCampaign } from '../../types/pack';
import { drawOverworld, createOwCanvasState } from '../../canvas/overworld-renderer';
import { OverworldInteraction, type PopupInfo } from '../../canvas/overworld-interaction';
import { api } from '../../api/client';
import { navigate, updateOverworld, updateCampaign } from '../../store/actions';
import { showToast } from '../toast';

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

  const _mdv = mapDataVersion.value;
  useEffect(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container) return;

    // Restore region overrides from saved data
    if (campaign.overworld.ow_region_offsets && Object.keys(owState.regionOverrides).length === 0) {
      for (const [k, v] of Object.entries(campaign.overworld.ow_region_offsets)) {
        owState.regionOverrides[k as any] = { ox: (v as any).ox, oy: (v as any).oy };
      }
    }

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

    if (owState.mapData && owState.zoom === 1 && owState.panX === 0 && owState.panY === 0) {
      zoomFit(container);
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
          // Convert nodeId to builder_region ID
          const levels = ow.levels || [];
          let brId: string;
          if (typeof nodeId === 'string') brId = nodeId;
          else if (nodeId === 0) brId = 'start';
          else brId = nodeId > levels.length ? 'store' : `level_${nodeId - 1}`;
          // Update builder_region directly
          const br = ow.builder_regions?.find(r => r.id === brId);
          if (br) { br.w = w; br.h = h; }
          // Also update legacy fields
          if (typeof nodeId === 'string') {
            const rooms = ow.rooms || ow.fork_chambers || [];
            const room = rooms.find(r => r.id === nodeId);
            if (room) { room.w = w; room.h = h; }
          } else if (nodeId === 0) {
            ow.start_room_size = [w, h];
          } else {
            ow.store_room_size = [w, h];
          }
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
        // Remove connections referencing this fork
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
          // Clean up connections referencing this level
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

      // Size controls for rooms, start, store
      const room = isRoom ? (campaign.overworld.rooms || campaign.overworld.fork_chambers || []).find(r => r.id === ni) : null;
      const canResize = isRoom || isStart || isStore;
      let curW = 10, curH = 8;
      if (room) { curW = room.w || 10; curH = room.h || 8; }
      else if (isStart) { const ss = campaign.overworld.start_room_size; curW = ss?.[0] || 40; curH = ss?.[1] || 40; }
      else if (isStore) { const ss = campaign.overworld.store_room_size; curW = ss?.[0] || 15; curH = ss?.[1] || 10; }

      const setSize = (w: number, h: number) => {
        if (room) {
          updateOverworld(ow => {
            const r = (ow.rooms || ow.fork_chambers || []).find(r => r.id === ni);
            if (r) { r.w = w; r.h = h; }
          });
        } else if (isStart) {
          updateOverworld(ow => { ow.start_room_size = [w, h]; });
        } else if (isStore) {
          updateOverworld(ow => { ow.store_room_size = [w, h]; });
        }
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
                  value={(() => {
                    const brId2 = isRoom ? String(ni) : isStart ? 'start' : 'store';
                    const brReg2 = campaign.overworld.builder_regions?.find(r => r.id === brId2);
                    return brReg2?.tile_source || '';
                  })()}
                  onChange={(e) => {
                    const val = (e.target as HTMLSelectElement).value || undefined;
                    const brId = isRoom ? String(ni) : isStart ? 'start' : 'store';
                    updateOverworld(ow => {
                      // Update builder_region
                      const br2 = ow.builder_regions?.find(r => r.id === brId);
                      if (br2) br2.tile_source = val;
                      // Also update legacy fields
                      if (isRoom) {
                        const r = (ow.rooms || ow.fork_chambers || []).find(r => r.id === ni);
                        if (r) r.tile_source = val;
                      } else if (isStart) {
                        ow.start_tile_source = val;
                      } else if (isStore) {
                        ow.store_tile_source = val;
                      }
                    });
                    // Also update the live campaignRef copy for immediate rendering
                    const brReg = campaignRef.current.overworld.builder_regions?.find(r => r.id === brId);
                    if (brReg) brReg.tile_source = val;
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

  return (
    <div ref={containerRef} class="dag-container" onClick={(e) => {
      // Dismiss popup when clicking container background (not canvas)
      if (e.target === containerRef.current) setPopup(null);
    }}>
      <canvas ref={canvasRef} />
      {renderPopup()}
    </div>
  );
}

async function loadMapData(campaign: BundledCampaign) {
  if (owState.mapData && owState.mapCampaignId === campaign.id) {
    mapDataVersion.value++;
    return;
  }
  try {
    const data = await api(`/api/overworld-map?id=${campaign.id}`);
    if (data) {
      owState.mapData = data;
      owState.mapCampaignId = campaign.id;
      owState.regionOverrides = {};
      owState.zoom = 1;
      owState.panX = 0;
      owState.panY = 0;
      mapDataVersion.value++;
    }
  } catch (e) {
    console.error('Failed to load overworld map:', e);
  }
}

export function zoomFit(container: HTMLElement) {
  const md = owState.mapData;
  if (!md?.regions) return;
  const rect = container.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) return;
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const r of md.regions) {
    const o = owState.regionOverrides[r.node_idx];
    const rx = o ? o.ox : r.ox;
    const ry = o ? o.oy : r.oy;
    minX = Math.min(minX, rx); minY = Math.min(minY, ry);
    maxX = Math.max(maxX, rx + r.w); maxY = Math.max(maxY, ry + r.h);
  }
  const totalW = (maxX - minX) || 1;
  const totalH = (maxY - minY) || 1;
  owState.zoom = Math.min(rect.width / (totalW * 4), rect.height / (totalH * 4)) * 0.85;
  owState.panX = -(minX * 4 * owState.zoom);
  owState.panY = -(minY * 4 * owState.zoom);
}

export function zoom1to1() {
  // Match in-game overworld scale: ~13px per tile (sw/120 at 1600w)
  // Builder base TILE = 4, so zoom = 13/4 ≈ 3.25
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
  const md = owState.mapData;
  const gridW = md ? md.width : 60;
  const gridH = md ? md.height : 36;
  const baseOx = (rect.width - gridW * tz) / 2 + owState.panX;
  const baseOy = (rect.height - gridH * tz) / 2 + owState.panY;
  const cx = (rect.width / 2 - baseOx) / tz;
  const cy = (rect.height / 2 - baseOy) / tz;
  return { ox: Math.round(cx), oy: Math.round(cy) };
}
