import type { Phase2Result } from '../../types/pack';
import { updateDesign } from '../../store/actions';
import { ROOTS, SCALES } from '../../store/state';

interface Props {
  design: Phase2Result;
}

export function TabMusic({ design }: Props) {
  const mode = design.mode || { root: 'C', scale: 'aeolian' };

  return (
    <div>
      <div class="form-row">
        <label>Root</label>
        <select
          value={mode.root || 'C'}
          onChange={(e) => updateDesign(d => {
            if (!d.mode) d.mode = { root: 'C', scale: 'aeolian' };
            d.mode.root = (e.target as HTMLSelectElement).value;
          })}
        >
          {ROOTS.map(r => <option key={r} value={r}>{r}</option>)}
        </select>
      </div>
      <div class="form-row">
        <label>Scale</label>
        <select
          value={mode.scale || 'aeolian'}
          onChange={(e) => updateDesign(d => {
            if (!d.mode) d.mode = { root: 'C', scale: 'aeolian' };
            d.mode.scale = (e.target as HTMLSelectElement).value;
          })}
        >
          {SCALES.map(s => <option key={s} value={s}>{s}</option>)}
        </select>
      </div>
    </div>
  );
}
