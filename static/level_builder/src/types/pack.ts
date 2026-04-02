// Mirrored from Rust serde structs in src/gen.rs, src/mapgen.rs, src/game.rs

// ── Pack (root) ──────────────────────────────────────────────

export interface BundledPack {
  theme?: string;
  campaigns: BundledCampaign[];
  strings: PackStrings;
  item_sprites: Record<string, string>;
  item_names: Record<string, string>;
  item_descriptions: Record<string, string>;
}

export interface PackStrings {
  title: string;
  subtitle: string;
  intro: string[];
  campaign_cleared: string;
  campaign_conquered: string;
  prompt_first: string;
  prompt_next: string;
  prompt_resume: string;
  prompt_restart: string;
  prompt_after_clear: string;
}

// ── Campaign ─────────────────────────────────────────────────

export interface BundledCampaign {
  id: string;
  overworld: OverworldResult;
  designs: Phase2Result[];
  quality: CampaignQuality;
  settings: CampaignSettings;
  monster_templates?: MonsterTemplateRaw[];
  prebuilt_overworld_map?: any;
}

export interface CampaignSettings {
  locked_doors_from_level: number;
  traps_from_level: number;
  damage_tiles_from_level: number;
  damage_tile_damage: number;
}

export interface CampaignQuality {
  score: number;
  breakdown: QualityBreakdown;
}

export interface QualityBreakdown {
  completeness: number;
  tile_variety: number;
  monster_variety: number;
  color_quality: number;
  name_quality: number;
  description_quality: number;
  mode_validity: number;
  budget_distribution: number;
  theme_coherence: number;
}

// ── Overworld ────────────────────────────────────────────────

export interface OverworldResult {
  name: string;
  font?: string;
  description_font?: string;
  label_font?: string;
  description: string;
  bg_color?: string;
  text_color?: string;
  bg_image?: string;
  bg_gradient?: string;
  bg_mode?: string;
  bg_prompt?: string;
  levels: OverworldNode[];
  store?: StoreConfig;
  boss_level?: number;
  connections?: [string, string][];
  one_way_connections?: string[];
  rooms?: Room[];
  /** @deprecated use rooms */ fork_chambers?: Room[];
  hallway_waypoints?: Record<string, [number, number][]>;
  start_room_size?: [number, number];
  store_room_size?: [number, number];
  start_tile_source?: string; // level_N — tiles for start room
  store_tile_source?: string; // level_N — tiles for store
  node_positions?: Record<string, NodePosition>;
  ow_region_offsets?: Record<string, RegionOffset>;
  terrain_seed?: number;
}

export interface OverworldNode {
  name: string;
  font?: string;
  description: string;
  theme: string;
  color?: string;
  palette?: string[];
  budget: number;
  exit_direction?: string;
}

export interface Room {
  id: string;
  name: string;
  w?: number;
  h?: number;
  tile_source?: string; // level_N id — use that level's tiles for this room
}

export interface StoreConfig {
  healing_potions?: number;
  speed_potions?: number;
  bombs?: number;
}

export interface NodePosition {
  x: number;
  y: number;
  w?: number;
  h?: number;
}

export interface RegionOffset {
  ox: number;
  oy: number;
}

// ── Level Design (Phase2Result) ──────────────────────────────

export interface Phase2Result {
  tile_defs: TileDefSlim[];
  boss: MonsterRaw;
  monster_types: MonsterTemplateRaw[];
  weapon: ItemTemplateRaw;
  armor: ItemTemplateRaw;
  traps?: TrapRaw[];
  budget_spent?: any;
  mode?: ModeRaw;
  victory_message?: string;
  defeat_message?: string;
  prebuilt_map?: MapGenResult;
  placed_entities?: PlacedEntities;
}

export interface TileDefSlim {
  name: string;
  char?: string;
  image?: string;
}

export interface MonsterRaw {
  name: string;
  hp: number;
  attack: number;
  defense?: number;
  xp_value?: number;
  description?: string;
  image?: string;
}

export interface MonsterTemplateRaw {
  name: string;
  hp: number;
  attack: number;
  defense?: number;
  xp_value?: number;
  description?: string;
  image?: string;
}

export interface ItemTemplateRaw {
  name: string;
  description?: string;
  image?: string;
}

export interface TrapRaw {
  x?: number;
  y?: number;
  damage?: number;
  name?: string;
  image?: string;
}

export interface ModeRaw {
  root: string;
  scale: string;
}

// ── Placed Entities ──────────────────────────────────────────

export interface PlacedEntities {
  monsters: PlacedMonster[];
  items: PlacedItem[];
  traps: PlacedTrap[];
  boss?: [number, number];
  exit_door?: [number, number];
  entry_door?: [number, number];
}

export interface PlacedMonster {
  name: string;
  x: number;
  y: number;
}

export interface PlacedItem {
  name: string;
  item_type: string;
  x: number;
  y: number;
  value?: number;
  image?: string;
}

export interface PlacedTrap {
  name: string;
  x: number;
  y: number;
}

// ── Map Generation ───────────────────────────────────────────

export interface MapGenResult {
  tiles: string[][];
  player_start: [number, number];
  boss_position: [number, number];
  key_position?: [number, number];
  exit_door_position?: [number, number];
  boss_room_bounds?: [number, number, number, number];
}

// ── Overworld Map (builder API response from /api/overworld-map) ──

export interface OverworldMapPreview {
  width: number;
  height: number;
  tiles: string[][];
  tile_defs: Record<string, OverworldTileDef>;
  regions: LevelRegion[];
  player_pos: [number, number];
}

// Full overworld map from game.rs (not used in builder directly)
export interface OverworldMap {
  width: number;
  height: number;
  tiles: string[][];
  tile_defs: Record<string, OverworldTileDef>;
  level_regions: LevelRegion[];
  hallways: HallwaySegment[];
  player_pos: [number, number];
}

export interface OverworldTileDef {
  name: string;
  color: string;
  walkable: boolean;
  char_display: string;
  damage: number;
  image?: string;
}

export interface LevelRegion {
  node_idx: number;
  ox: number;
  oy: number;
  w: number;
  h: number;
  entry_pos?: [number, number];
  exit_pos?: [number, number];
}

export interface HallwaySegment {
  from_level: number;
  to_level: number;
  tiles: [number, number][];
}
