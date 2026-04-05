import type { BundledCampaign } from '../../types/pack';
import { updateCampaign } from '../../store/actions';
import { EnhancedInput, EnhancedTextarea } from '../enhanced-input';
import { FontSelect } from '../font-picker';
import { loadGoogleFont } from '../../api/fonts';

interface Props {
  campaign: BundledCampaign;
}

export function TabSignposts({ campaign }: Props) {
  const signposts = campaign.signposts || [];

  // Load fonts for preview
  for (const sp of signposts) {
    if (sp.title_font) loadGoogleFont(sp.title_font);
    if (sp.description_font) loadGoogleFont(sp.description_font);
  }

  function addSignpost() {
    updateCampaign(c => {
      if (!c.signposts) c.signposts = [];
      c.signposts.push({ title: 'New Sign', description: '' });
    });
  }

  function removeSignpost(idx: number) {
    updateCampaign(c => {
      c.signposts?.splice(idx, 1);
    });
  }

  return (
    <div>
      <p class="note mb-8">Signs the player can read. Place them on the overworld map.</p>
      {signposts.map((sp, i) => (
        <div key={i} style="background:var(--input-bg);border:1px solid var(--border);border-radius:6px;padding:8px;margin-bottom:8px;">
          <div class="flex gap-4 items-center mb-8">
            <EnhancedInput
              value={sp.title}
              onChange={(v) => updateCampaign(c => { if (c.signposts?.[i]) c.signposts[i].title = v; })}
              context="title text for a signpost in a roguelike dungeon"
              style="font-size:12px;font-weight:600;"
            />
            <button
              class="danger"
              onClick={() => removeSignpost(i)}
              style="padding:2px 6px;font-size:10px;"
            >
              x
            </button>
          </div>
          <div class="form-row">
            <label>Title Font</label>
            <FontSelect
              current={sp.title_font || ''}
              onChange={(v) => updateCampaign(c => { if (c.signposts?.[i]) c.signposts[i].title_font = v || undefined; })}
            />
          </div>
          <EnhancedTextarea
            value={sp.description}
            onChange={(v) => updateCampaign(c => { if (c.signposts?.[i]) c.signposts[i].description = v; })}
            context="text on a signpost in a roguelike dungeon"
            rows={3}
          />
          <div class="form-row">
            <label>Desc Font</label>
            <FontSelect
              current={sp.description_font || ''}
              onChange={(v) => updateCampaign(c => { if (c.signposts?.[i]) c.signposts[i].description_font = v || undefined; })}
            />
          </div>
        </div>
      ))}
      <button onClick={addSignpost} style="font-size:12px;">+ Add Signpost</button>
    </div>
  );
}
