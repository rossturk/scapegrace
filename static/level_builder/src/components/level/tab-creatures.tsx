import type { Phase2Result } from '../../types/pack';
import { updateDesign } from '../../store/actions';
import { EnhancedInput, EnhancedTextarea } from '../enhanced-input';
import { showToast } from '../toast';
import { generateImage } from '../../api/generation';

interface Props {
  design: Phase2Result;
}

export function TabCreatures({ design }: Props) {
  const boss = design.boss || {} as any;

  return (
    <div>
      <h3>Boss</h3>
      <div class="form-row">
        <label>Name</label>
        <EnhancedInput
          value={boss.name || ''}
          onChange={(v) => updateDesign(d => { d.boss.name = v; })}
          context="boss monster name for roguelike level"
        />
      </div>
      <div class="form-row-stacked">
        <label>Description</label>
        <EnhancedTextarea
          value={boss.description || ''}
          onChange={(v) => updateDesign(d => { d.boss.description = v; })}
          context="five-line physical description of a boss monster"
          rows={5}
        />
      </div>
      <div class="flex gap-8 items-center" style="margin-top:4px;">
        {boss.image ? (
          <>
            <img
              src={`data:image/png;base64,${boss.image}`}
              style="width:64px;height:64px;border-radius:4px;border:1px solid var(--border);image-rendering:pixelated;"
            />
            <button class="ai-btn" onClick={() => genBossSprite(design)} style="font-size:10px;">&#10024; Regen</button>
            <button style="font-size:9px;padding:2px 4px;" onClick={() => updateDesign(d => { delete (d.boss as any).image; })}>Clear</button>
          </>
        ) : (
          <button class="ai-btn" onClick={() => genBossSprite(design)}>&#10024; Generate Sprite</button>
        )}
      </div>
      <div class="note">(Stats are computed by the engine from budget)</div>
    </div>
  );
}

async function genBossSprite(design: Phase2Result) {
  const b = design.boss;
  if (!b) return;
  showToast('Generating boss sprite...', 'info');
  const b64 = await generateImage({
    prompt: `2D pixel art sprite of a roguelike game boss monster called "${b.name}". ${b.description || ''}. Top-down view, 64x64 pixel art, single large creature centered on pure solid BLACK background. No text.`,
    width: 128,
    height: 128,
  });
  if (b64) {
    updateDesign(d => { d.boss.image = b64; });
    showToast('Boss sprite generated!', 'success');
  }
}
