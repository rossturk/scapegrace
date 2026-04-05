import { signal, computed } from '@preact/signals';
import type { BundledPack, BundledCampaign, OverworldResult, Phase2Result, OverworldMap } from '../types/pack';

// ── Core data ────────────────────────────────────────────────
export const pack = signal<BundledPack | null>(null);
export const fonts = signal<string[]>([]);
export const allGoogleFonts = signal<string[]>([]);

// Bumped on every mutation so components re-render even when object refs don't change
export const packVersion = signal(0);
// Version at last save — compare to packVersion to detect unsaved changes
export const savedVersion = signal(0);

// ── Navigation ───────────────────────────────────────────────
export const selectedCampaignId = signal<string | null>(null);
export const selectedLevelIdx = signal<number | null>(null);

// ── UI state ─────────────────────────────────────────────────
export const currentTab = signal('level');
export const campaignTab = signal('overview');
export const packSettingsTab = signal('theme');
export const mapPreviewData = signal<any>(null);

// ── Derived (read packVersion to bust cache on mutations) ────
export const selectedCampaign = computed<BundledCampaign | null>(() => {
  const _v = packVersion.value; // dependency: re-evaluate on any mutation
  const p = pack.value;
  const id = selectedCampaignId.value;
  if (!p || !id) return null;
  const c = p.campaigns.find(c => c.id === id);
  return c ? { ...c } : null; // new ref so signal notifies consumers
});

export const selectedOverworld = computed<OverworldResult | null>(() => {
  const c = selectedCampaign.value;
  return c ? c.overworld : null;
});

export const selectedDesign = computed<Phase2Result | null>(() => {
  const c = selectedCampaign.value;
  const idx = selectedLevelIdx.value;
  if (!c || idx === null) return null;
  const d = c.designs[idx];
  return d ? { ...d } : null; // new ref so signal notifies consumers
});

// ── Constants ────────────────────────────────────────────────
export const ROOTS = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];
export const SCALES = [
  'ionian', 'dorian', 'phrygian', 'lydian', 'mixolydian', 'aeolian', 'locrian',
  'pentatonic_major', 'pentatonic_minor', 'blues', 'whole_tone', 'chromatic',
];
