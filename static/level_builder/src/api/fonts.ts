import { api } from './client';
import { fonts, allGoogleFonts } from '../store/state';

const loadedGoogleFonts = new Set<string>();

export async function loadFonts() {
  const data = await api<string[]>('/api/fonts', { silent: true });
  if (data) fonts.value = data;
}

export async function loadAllGoogleFonts() {
  if (allGoogleFonts.value.length > 0) return;
  const data = await api<string[]>('/api/google-fonts', { silent: true });
  if (data) allGoogleFonts.value = data;
}

export function loadGoogleFont(name: string) {
  if (!name || loadedGoogleFonts.has(name)) return;
  loadedGoogleFonts.add(name);
  const link = document.createElement('link');
  link.rel = 'stylesheet';
  link.href = `https://fonts.googleapis.com/css2?family=${encodeURIComponent(name)}&display=swap`;
  document.head.appendChild(link);
}
