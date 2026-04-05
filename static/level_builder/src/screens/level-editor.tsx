import { selectedCampaign, selectedLevelIdx, selectedDesign, currentTab } from '../store/state';
import { navigate, updateDesign } from '../store/actions';
import { generateMap } from '../api/generation';
import { showToast } from '../components/toast';
import { EntityTray, generateTray, type TrayItem } from '../components/level/entity-tray';
import { MapCanvas, draggedItem } from '../components/level/map-canvas';
import { TabLevel } from '../components/level/tab-level';
import { TabCreatures } from '../components/level/tab-creatures';
import { TabItems } from '../components/level/tab-items';
import { TabTiles } from '../components/level/tab-tiles';
import { TabMusic } from '../components/level/tab-music';
import { autoPlaceItem, autoPlaceAll } from '../canvas/placement';

export function LevelEditor() {
  const campaign = selectedCampaign.value;
  const levelIdx = selectedLevelIdx.value;
  const design = selectedDesign.value;
  const tab = currentTab.value;

  if (!campaign || levelIdx === null) {
    navigate(null);
    return null;
  }

  const level = campaign.overworld.levels[levelIdx];
  if (!level) {
    navigate(campaign.id);
    return null;
  }

  const pe = design?.placed_entities ?? { monsters: [], items: [], traps: [] };
  const tray = design ? generateTray(campaign, levelIdx, pe as any) : [];

  const tabs = [
    { id: 'level', label: 'Level' },
    { id: 'tiles', label: 'Tiles' },
    { id: 'creatures', label: 'Creatures' },
    { id: 'items', label: 'Items' },
    { id: 'music', label: 'Music' },
  ];

  function handleTrayDragStart(idx: number, e: DragEvent) {
    const item = tray[idx];
    if (!item) return;
    // Set the shared signal so MapCanvas can read it on drop
    draggedItem.value = item;
    if (e.dataTransfer) {
      e.dataTransfer.setData('text/plain', JSON.stringify(item));
      e.dataTransfer.effectAllowed = 'copy';
    }
  }

  function handleAutoPlace(idx: number) {
    const item = tray[idx];
    if (!item || !design?.prebuilt_map) return;
    updateDesign(d => {
      if (!d.placed_entities) d.placed_entities = { monsters: [], items: [], traps: [] };
      const ok = autoPlaceItem(item, d.prebuilt_map! as any, d.placed_entities!, d);
      if (!ok) showToast(`Could not place ${item.name}`, 'error');
    });
  }

  function handleAutoplaceAll() {
    if (!design?.prebuilt_map) return;
    updateDesign(d => {
      if (!d.placed_entities) d.placed_entities = { monsters: [], items: [], traps: [] };
      const count = autoPlaceAll(tray, d.prebuilt_map! as any, d.placed_entities!, d);
      showToast(`Placed ${count} entities`, 'success');
    });
  }

  async function handleRegenMap() {
    if (!design) return;
    showToast('Regenerating map...', 'info');
    const result = await generateMap({
      tile_defs: design.tile_defs || [],
      palette: level.palette || [],
    });
    if (result) {
      updateDesign(d => { d.prebuilt_map = result as any; });
      showToast('Map regenerated!', 'success');
    }
  }

  function handleClearAll() {
    updateDesign(d => {
      d.placed_entities = { monsters: [], items: [], traps: [] };
    });
  }

  return (
    <div class="level-screen">
      <div class="level-map-area">
        <EntityTray items={tray} onDragStart={handleTrayDragStart} onAutoPlace={handleAutoPlace} />
        {design && (
          <MapCanvas campaign={campaign} levelIdx={levelIdx} design={design} palette={level.palette || []} />
        )}
        <div class="map-controls">
          <button class="primary" onClick={handleRegenMap}>Regen Map</button>
          <button onClick={handleAutoplaceAll}>Autoplace All</button>
          <button onClick={handleClearAll}>Clear All</button>
          <span id="map-status" style="font-size:12px;color:#888;" />
          <span style="flex:1;" />
          <span style="font-size:11px;color:#555;">
            <span style="color:#66bb6a">&#9632;</span> Player{' '}
            <span style="color:#e94560">&#9632;</span> Boss{' '}
            <span style="color:#4dd0e1">&#9632;</span> Key{' '}
            <span style="color:#ff69b4">&#9632;</span> Monster{' '}
            <span style="color:#ffa726">&#9632;</span> Item{' '}
            <span style="color:#ab47bc">&#9632;</span> Trap{' '}
            <span style="color:#ffcc00">&#9632;</span> Exit
          </span>
        </div>
      </div>

      <div class="level-sidebar">
        <div class="tab-bar">
          {tabs.map(t => (
            <button
              key={t.id}
              class={tab === t.id ? 'active' : ''}
              onClick={() => { currentTab.value = t.id; }}
            >
              {t.label}
            </button>
          ))}
        </div>
        <div class="tab-content">
          {design ? (
            <>
              {tab === 'level' && <TabLevel level={level} design={design} />}
              {tab === 'tiles' && <TabTiles design={design} palette={level.palette || []} />}
              {tab === 'creatures' && <TabCreatures design={design} />}
              {tab === 'items' && <TabItems design={design} />}
              {tab === 'music' && <TabMusic design={design} />}
            </>
          ) : (
            <p style="color:#888;">No design data for this level.</p>
          )}
        </div>
      </div>
    </div>
  );
}
