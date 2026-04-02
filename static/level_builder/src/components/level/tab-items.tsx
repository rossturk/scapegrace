import type { Phase2Result } from '../../types/pack';
import { updateDesign } from '../../store/actions';
import { EnhancedInput, EnhancedTextarea } from '../enhanced-input';
import { showToast } from '../toast';
import { generateImage } from '../../api/generation';

interface Props {
  design: Phase2Result;
}

export function TabItems({ design }: Props) {
  const weapon = design.weapon || {} as any;
  const armor = design.armor || {} as any;
  const traps = design.traps || [];

  return (
    <div>
      <h3>Weapon</h3>
      <ItemEditor
        item={weapon}
        onChangeName={(v) => updateDesign(d => { d.weapon.name = v; })}
        onChangeDesc={(v) => updateDesign(d => { d.weapon.description = v; })}
        onGenSprite={() => genItemSprite(design, 'weapon')}
        onClearSprite={() => updateDesign(d => { delete (d.weapon as any).image; })}
        nameContext="weapon name for roguelike game"
        descContext="five-line weapon description"
        fallbackIcon="&#9876;"
      />

      <hr class="section-divider" />
      <h3>Armor</h3>
      <ItemEditor
        item={armor}
        onChangeName={(v) => updateDesign(d => { d.armor.name = v; })}
        onChangeDesc={(v) => updateDesign(d => { d.armor.description = v; })}
        onGenSprite={() => genItemSprite(design, 'armor')}
        onClearSprite={() => updateDesign(d => { delete (d.armor as any).image; })}
        nameContext="armor name for roguelike game"
        descContext="five-line armor description"
        fallbackIcon="&#128737;"
      />

      <hr class="section-divider" />
      <h3>Traps</h3>
      {traps.map((trap, i) => (
        <div key={i} style="background:var(--input-bg);border:1px solid var(--border);border-radius:6px;padding:8px;margin-bottom:8px;">
          <div class="flex gap-4 items-center mb-8">
            <EnhancedInput
              value={trap.name || ''}
              onChange={(v) => updateDesign(d => { if (d.traps?.[i]) d.traps[i].name = v; })}
              context="trap name for roguelike dungeon"
              style="font-size:12px;font-weight:600;"
            />
            <button class="danger" onClick={() => updateDesign(d => { d.traps?.splice(i, 1); })} style="padding:2px 6px;font-size:10px;">x</button>
          </div>
        </div>
      ))}
      <button style="font-size:12px;" onClick={() => updateDesign(d => {
        if (!d.traps) d.traps = [];
        d.traps.push({ name: '' });
      })}>
        Add Trap
      </button>
    </div>
  );
}

function ItemEditor({ item, onChangeName, onChangeDesc, onGenSprite, onClearSprite, nameContext, descContext, fallbackIcon }: {
  item: any;
  onChangeName: (v: string) => void;
  onChangeDesc: (v: string) => void;
  onGenSprite: () => void;
  onClearSprite: () => void;
  nameContext: string;
  descContext: string;
  fallbackIcon: string;
}) {
  return (
    <div>
      <div class="form-row">
        <label>Name</label>
        <EnhancedInput value={item.name || ''} onChange={onChangeName} context={nameContext} />
      </div>
      <div class="form-row-stacked">
        <label>Description</label>
        <EnhancedTextarea value={item.description || ''} onChange={onChangeDesc} context={descContext} rows={5} />
      </div>
      <div class="flex gap-8 items-center" style="margin-top:6px;">
        {item.image ? (
          <>
            <img src={`data:image/png;base64,${item.image}`} style="width:48px;height:48px;border-radius:4px;border:1px solid var(--border);image-rendering:pixelated;" />
            <button class="ai-btn" onClick={onGenSprite} style="font-size:11px;">&#10024; Regen</button>
            <button style="font-size:10px;padding:2px 6px;" onClick={onClearSprite}>Clear</button>
          </>
        ) : (
          <>
            <div style="width:48px;height:48px;border-radius:4px;border:1px dashed var(--border);display:flex;align-items:center;justify-content:center;font-size:20px;color:#555;" dangerouslySetInnerHTML={{ __html: fallbackIcon }} />
            <button class="ai-btn" onClick={onGenSprite}>&#10024; Generate Sprite</button>
          </>
        )}
      </div>
    </div>
  );
}

async function genItemSprite(design: Phase2Result, type: 'weapon' | 'armor') {
  const item = design[type];
  if (!item) return;
  const typeLabel = type === 'weapon' ? 'weapon' : 'armor/shield';
  showToast(`Generating ${type} sprite...`, 'info');
  const b64 = await generateImage({
    prompt: `2D pixel art sprite of a roguelike game ${typeLabel} called "${item.name}". ${item.description || ''}. Top-down view, 32x32 pixel art, single item centered on pure solid BLACK background. No text.`,
    width: 64,
    height: 64,
  });
  if (b64) {
    updateDesign(d => { (d[type] as any).image = b64; });
    showToast('Sprite generated!', 'success');
  }
}
