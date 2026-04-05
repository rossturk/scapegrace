import { useEffect } from 'preact/hooks';
import { loadPack, navigate } from './store/actions';
import { loadFonts } from './api/fonts';
import { pack, selectedCampaignId, selectedLevelIdx } from './store/state';
import { TitleBar } from './components/title-bar';
import { ToastContainer } from './components/toast';
import { PackOverview } from './screens/pack-overview';
import { CampaignEditor } from './screens/campaign-editor';
import { LevelEditor } from './screens/level-editor';

export function App() {
  useEffect(() => {
    loadPack();
    loadFonts();

    // Handle browser back/forward
    const onPopState = () => {
      const m = location.pathname.match(/^\/campaigns\/([^/]+)(?:\/levels\/(\d+))?/);
      if (m) {
        selectedCampaignId.value = m[1];
        selectedLevelIdx.value = m[2] !== undefined ? Number(m[2]) : null;
      } else {
        selectedCampaignId.value = null;
        selectedLevelIdx.value = null;
      }
    };
    window.addEventListener('popstate', onPopState);
    return () => window.removeEventListener('popstate', onPopState);
  }, []);

  if (!pack.value) {
    return (
      <div class="loading">Loading pack...</div>
    );
  }

  const isLevel = selectedCampaignId.value && selectedLevelIdx.value !== null;
  const isCampaign = selectedCampaignId.value && !isLevel;

  return (
    <div id="app">
      <TitleBar />
      <div
        class="screen"
        style={isLevel || isCampaign ? 'padding:0;overflow:hidden;' : undefined}
      >
        {isLevel ? (
          <LevelEditor />
        ) : isCampaign ? (
          <CampaignEditor />
        ) : (
          <PackOverview />
        )}
      </div>
      <ToastContainer />
    </div>
  );
}
