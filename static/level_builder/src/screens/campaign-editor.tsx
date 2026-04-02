import { selectedCampaign } from '../store/state';
import { navigate } from '../store/actions';
import { OverworldCanvas } from '../components/campaign/overworld-canvas';
import { DAGToolbar } from '../components/campaign/dag-toolbar';
import { LeftPanel } from '../components/campaign/left-panel';

export function CampaignEditor() {
  const campaign = selectedCampaign.value;
  if (!campaign) {
    navigate(null);
    return null;
  }

  return (
    <div class="campaign-layout">
      <div class="center-panel">
        <DAGToolbar />
        <OverworldCanvas campaign={campaign} />
      </div>
      <LeftPanel campaign={campaign} />
    </div>
  );
}
