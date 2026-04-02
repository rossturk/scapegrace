// Request/response types for level_builder.rs API endpoints

import type { TileDefSlim } from './pack';

export interface GenerateMapRequest {
  tile_defs: TileDefSlim[];
  palette: string[];
}

export interface OverworldRequest {
  theme?: string;
}

export interface LevelDesignRequest {
  campaign_name: string;
  campaign_desc: string;
  level_config: LevelConfig;
  theme?: string;
}

export interface LevelConfig {
  title: string;
  font: string;
  description: string;
  theme: string;
  palette: string[];
  budget: number;
  floor: number;
  campaign_tier: number;
}

export interface DescriptionRequest {
  context: string;
  target: string;
}

export interface ImageRequest {
  prompt: string;
  width?: number;
  height?: number;
  aspect_ratio?: string;
}

export interface TextResponse {
  text: string;
}

export interface ImageResponse {
  image_base64: string;
}
