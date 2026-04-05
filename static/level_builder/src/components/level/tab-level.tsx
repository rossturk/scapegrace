import type { OverworldNode, Phase2Result } from '../../types/pack';
import { updateOverworld, updateDesign } from '../../store/actions';
import { selectedLevelIdx } from '../../store/state';
import { EnhancedInput, EnhancedTextarea } from '../enhanced-input';
import { FontSelect } from '../font-picker';
import { loadGoogleFont } from '../../api/fonts';

interface Props {
  level: OverworldNode;
  design: Phase2Result;
}

export function TabLevel({ level, design }: Props) {
  const idx = selectedLevelIdx.value!;
  if (level.font) loadGoogleFont(level.font);

  return (
    <div>
      <div class="form-row-stacked">
        <label>Name</label>
        <EnhancedInput
          value={level.name}
          onChange={(v) => updateOverworld(ow => { ow.levels[idx].name = v; })}
          context="level name for roguelike dungeon"
          style="font-size:16px;font-weight:600;"
        />
      </div>
      <hr class="section-divider" />
      <h3 style="margin-bottom:8px;color:#888;text-transform:uppercase;font-size:10px;letter-spacing:1px;">Typography</h3>
      <div class="form-row">
        <label>Font</label>
        <FontSelect
          current={level.font || ''}
          onChange={(v) => {
            updateOverworld(ow => { ow.levels[idx].font = v || undefined; });
            if (v) loadGoogleFont(v);
          }}
        />
      </div>
      <hr class="section-divider" />
      <div class="form-row-stacked">
        <label>Description</label>
        <EnhancedTextarea
          value={level.description}
          onChange={(v) => updateOverworld(ow => { ow.levels[idx].description = v; })}
          context="five-line level description"
          rows={5}
        />
      </div>
      <hr class="section-divider" />
      <div class="form-row-stacked">
        <label>Victory Message</label>
        <EnhancedInput
          value={design.victory_message || ''}
          onChange={(v) => updateDesign(d => { d.victory_message = v; })}
          context="one-line victory message for roguelike level"
        />
      </div>
      <div class="form-row-stacked">
        <label>Defeat Message</label>
        <EnhancedInput
          value={design.defeat_message || ''}
          onChange={(v) => updateDesign(d => { d.defeat_message = v; })}
          context="one-line defeat message for roguelike level"
        />
      </div>
    </div>
  );
}
