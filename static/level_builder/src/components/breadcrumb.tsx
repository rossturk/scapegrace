import { selectedCampaignId, selectedLevelIdx, selectedCampaign } from '../store/state';
import { navigate } from '../store/actions';

export function Breadcrumb() {
  const campaign = selectedCampaign.value;
  const levelIdx = selectedLevelIdx.value;

  if (selectedCampaignId.value && levelIdx !== null) {
    const level = campaign?.overworld?.levels?.[levelIdx];
    const campName = campaign?.overworld?.name || 'Campaign';
    const lvName = level?.name || 'Level';
    return (
      <span class="breadcrumb">
        <a onClick={() => navigate(null)}>Pack</a>
        <span class="bc-sep">&rsaquo;</span>
        <a onClick={() => navigate(selectedCampaignId.value)}>{campName}</a>
        <span class="bc-sep">&rsaquo;</span>
        <span class="bc-current">{lvName}</span>
      </span>
    );
  }

  if (selectedCampaignId.value) {
    const campName = campaign?.overworld?.name || 'Campaign';
    return (
      <span class="breadcrumb">
        <a onClick={() => navigate(null)}>Pack</a>
        <span class="bc-sep">&rsaquo;</span>
        <span class="bc-current">{campName}</span>
      </span>
    );
  }

  return (
    <span class="breadcrumb">
      <span class="bc-current">Pack</span>
    </span>
  );
}
