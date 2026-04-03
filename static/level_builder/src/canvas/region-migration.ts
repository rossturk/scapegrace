// Migrate old campaigns to builder_regions format.
// Reads from ow_region_offsets + backend map data to create the canonical region list.

import type { BundledCampaign, BuilderRegion, OverworldMapPreview } from '../types/pack';

const ROOM_W = 10, ROOM_H = 8;

export function migrateToBuilderRegions(
  campaign: BundledCampaign,
  backendMapData?: OverworldMapPreview | null,
): BuilderRegion[] {
  const ow = campaign.overworld;
  const regions: BuilderRegion[] = [];
  const offsets = ow.ow_region_offsets || {};

  if (backendMapData?.regions) {
    const levels = ow.levels || [];
    for (const r of backendMapData.regions) {
      const override = offsets[r.node_idx];
      const ox = override ? (override as any).ox : r.ox;
      const oy = override ? (override as any).oy : r.oy;

      if (r.node_idx === 0) {
        // Start room
        const ss = ow.start_room_size;
        regions.push({
          id: 'start',
          type: 'start',
          ox, oy,
          w: ss?.[0] || r.w,
          h: ss?.[1] || r.h,
          tile_source: ow.start_tile_source,
        });
      } else if (r.node_idx > levels.length) {
        // Store
        const ss = ow.store_room_size;
        regions.push({
          id: 'store',
          type: 'store',
          ox, oy,
          w: ss?.[0] || r.w,
          h: ss?.[1] || r.h,
          tile_source: ow.store_tile_source,
        });
      } else {
        // Level
        const levelIdx = r.node_idx - 1;
        regions.push({
          id: `level_${levelIdx}`,
          type: 'level',
          ox, oy,
          w: r.w,
          h: r.h,
          level_idx: levelIdx,
        });
      }
    }
  }

  // Add rooms (formerly fork_chambers)
  const rooms = ow.rooms || ow.fork_chambers || [];
  for (const room of rooms) {
    const pos = offsets[room.id as any];
    regions.push({
      id: room.id,
      type: 'room',
      ox: pos ? (pos as any).ox : 0,
      oy: pos ? (pos as any).oy : 0,
      w: room.w || ROOM_W,
      h: room.h || ROOM_H,
      tile_source: room.tile_source,
    });
  }

  return regions;
}

// Create default regions for a brand new campaign
export function createDefaultRegions(): BuilderRegion[] {
  return [
    { id: 'start', type: 'start', ox: 0, oy: 0, w: 20, h: 15 },
  ];
}
