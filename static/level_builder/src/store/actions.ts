import { batch } from '@preact/signals';
import { pack, selectedCampaignId, selectedLevelIdx, mapPreviewData, packVersion, savedVersion } from './state';
import { api } from '../api/client';
import type { BundledPack, BundledCampaign, OverworldResult, Phase2Result } from '../types/pack';

// ── Pack CRUD ────────────────────────────────────────────────

export async function loadPack() {
  const data = await api<BundledPack>('/api/pack');
  if (!data) return;
  if (!data.strings) data.strings = {} as any;
  if (!data.campaigns) data.campaigns = [];
  pack.value = data;

  // Restore nav from URL
  const m = location.pathname.match(/^\/campaigns\/([^/]+)(?:\/levels\/(\d+))?/);
  if (m) {
    const cId = m[1];
    const lIdx = m[2] !== undefined ? Number(m[2]) : null;
    if (data.campaigns.find(c => c.id === cId)) {
      batch(() => {
        selectedCampaignId.value = cId;
        selectedLevelIdx.value = lIdx;
      });
    }
  }
}

export async function savePack() {
  if (!pack.value) return;
  // Export WYSIWYG overworld maps before saving
  const { exportOverworldMap } = await import('../canvas/overworld-export');
  const { owState } = await import('../components/campaign/overworld-canvas');
  for (const campaign of pack.value.campaigns) {
    if (owState.mapData && owState.mapCampaignId === campaign.id) {
      const exported = exportOverworldMap(campaign, owState);
      if (exported) campaign.prebuilt_overworld_map = exported;
    }
  }
  await api('/api/pack', { method: 'PUT', body: pack.value });
  mapPreviewData.value = null;
  savedVersion.value = packVersion.value;
  const { showToast } = await import('../components/toast');
  showToast('Pack saved', 'success');
}

export async function createCampaign(): Promise<any> {
  const campaign = await api('/api/campaigns', { method: 'POST' });
  if (campaign && pack.value) {
    pack.value.campaigns.push(campaign);
    pack.value = { ...pack.value };
    packVersion.value++;
    return campaign;
  }
  return null;
}

export async function deleteCampaign(id: string) {
  await api(`/api/campaigns/${id}`, { method: 'DELETE' });
  if (pack.value) {
    pack.value.campaigns = pack.value.campaigns.filter(c => c.id !== id);
    pack.value = { ...pack.value };
    packVersion.value++;
    if (selectedCampaignId.value === id) {
      navigate(null);
    }
  }
}

// ── Navigation ───────────────────────────────────────────────

export function navigate(campaignId: string | null, levelIdx: number | null = null) {
  batch(() => {
    selectedCampaignId.value = campaignId;
    selectedLevelIdx.value = levelIdx;
  });

  let path = '/';
  if (campaignId && levelIdx !== null) {
    path = `/campaigns/${campaignId}/levels/${levelIdx}`;
  } else if (campaignId) {
    path = `/campaigns/${campaignId}`;
  }
  if (location.pathname !== path) history.pushState(null, '', path);
}

// ── Pack mutations (in-memory only — call savePack() to persist) ──

export function updatePack(updater: (p: BundledPack) => void) {
  const p = pack.value;
  if (!p) return;
  updater(p);
  pack.value = { ...p };
  packVersion.value++;
}

export function updateCampaign(updater: (c: BundledCampaign) => void) {
  const p = pack.value;
  const id = selectedCampaignId.value;
  if (!p || !id) return;
  const c = p.campaigns.find(c => c.id === id);
  if (!c) return;
  updater(c);
  pack.value = { ...p };
  packVersion.value++;
}

export function updateOverworld(updater: (ow: OverworldResult) => void) {
  updateCampaign(c => updater(c.overworld));
}

export function updateDesign(updater: (d: Phase2Result) => void) {
  const idx = selectedLevelIdx.value;
  if (idx === null) return;
  updateCampaign(c => {
    if (c.designs[idx]) updater(c.designs[idx]);
  });
}
