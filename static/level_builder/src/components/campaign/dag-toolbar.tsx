import { zoomFit, zoom1to1, owState, getViewCenter, triggerRedraw } from './overworld-canvas';
import { updateOverworld, updateCampaign } from '../../store/actions';
import { showToast } from '../toast';

export function DAGToolbar() {
  function addLevel() {
    const name = prompt('Level name:', 'New Level');
    if (!name) return;
    const center = getViewCenter();
    updateCampaign(c => {
      const levelIdx = c.overworld.levels.length;
      c.overworld.levels.push({
        name,
        description: '',
        theme: '',
        color: '#555555',
        palette: ['#444444', '#666666'],
        budget: 150,
      } as any);
      c.designs.push({
        tile_defs: [{ name: 'wall', char: '#' }, { name: 'floor', char: '.' }],
        boss: { name: '', hp: 0, attack: 0 },
        monster_types: [],
        weapon: { name: '' },
        armor: { name: '' },
        traps: [],
        mode: { root: 'C', scale: 'aeolian' },
      } as any);
      // Add to builder_regions (source of truth for rendering/dragging)
      if (!c.overworld.builder_regions) c.overworld.builder_regions = [];
      c.overworld.builder_regions.push({
        id: `level_${levelIdx}`,
        type: 'level',
        ox: center.ox,
        oy: center.oy,
        w: 20,
        h: 15,
        level_idx: levelIdx,
      });
    });
    owState.hallwayCacheKey = null;
    triggerRedraw();
    showToast('Level added', 'success');
  }

  function addStore() {
    const center = getViewCenter();
    updateOverworld(ow => {
      if (!ow.store) {
        ow.store = { healing_potions: 3, speed_potions: 1, bombs: 1 };
        // Add to builder_regions (source of truth for rendering/dragging)
        if (!ow.builder_regions) ow.builder_regions = [];
        if (!ow.builder_regions.find(r => r.type === 'store')) {
          ow.builder_regions.push({
            id: 'store',
            type: 'store',
            ox: center.ox,
            oy: center.oy,
            w: 10,
            h: 8,
          });
        }
        showToast('Store added', 'success');
      } else {
        showToast('Store already exists', 'info');
      }
    });
    owState.hallwayCacheKey = null;
    triggerRedraw();
  }

  function addRoom() {
    const id = 'room_' + Math.random().toString(36).substr(2, 6);
    const center = getViewCenter();
    updateOverworld(ow => {
      if (!ow.rooms) ow.rooms = [];
      ow.rooms.push({ id, name: '' });
      if (!ow.builder_regions) ow.builder_regions = [];
      ow.builder_regions.push({
        id,
        type: 'room',
        ox: center.ox,
        oy: center.oy,
        w: 10,
        h: 8,
      });
    });
    owState.hallwayCacheKey = null;
    triggerRedraw();
    showToast('Room added', 'success');
  }

  return (
    <div class="dag-toolbar">
      <button style="font-size:11px;padding:4px 8px;" onClick={() => {
        const container = document.querySelector('.dag-container') as HTMLElement;
        if (container) { zoomFit(container); triggerRedraw(); }
      }}>Fit</button>
      <button style="font-size:11px;padding:4px 8px;" onClick={() => {
        zoom1to1();
        triggerRedraw();
      }}>1:1</button>
      <span style="width:1px;height:16px;background:var(--border);margin:0 4px;" />
      <button style="font-size:11px;padding:4px 8px;" onClick={addLevel}>+ Level</button>
      <button style="font-size:11px;padding:4px 8px;" onClick={addStore}>+ Store</button>
      <button style="font-size:11px;padding:4px 8px;" onClick={addRoom}>+ Room</button>
    </div>
  );
}
