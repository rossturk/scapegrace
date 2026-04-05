import { api } from './client';
import type {
  GenerateMapRequest,
  LevelDesignRequest,
  DescriptionRequest,
  ImageRequest,
  TextResponse,
  ImageResponse,
} from '../types/api';
import type { MapGenResult } from '../types/pack';

export async function generateMap(req: GenerateMapRequest): Promise<MapGenResult | null> {
  return api<MapGenResult>('/api/generate-map', { method: 'POST', body: req });
}

export async function generateLevelDesign(req: LevelDesignRequest) {
  return api('/api/generate/level-design', { method: 'POST', body: req });
}

export async function generateDescription(context: string, target: string): Promise<string | null> {
  const res = await api<TextResponse>('/api/generate/description', {
    method: 'POST',
    body: { context, target } satisfies DescriptionRequest,
  });
  return res?.text ?? null;
}

export async function generateImage(req: ImageRequest): Promise<string | null> {
  const res = await api<ImageResponse>('/api/generate-image', { method: 'POST', body: req });
  return res?.image_base64 ?? null;
}
