import { useState } from 'preact/hooks';
import { pack, packVersion } from '../store/state';
import { navigate, updatePack, createCampaign, savePack } from '../store/actions';
import { EnhancedInput, EnhancedTextarea } from '../components/enhanced-input';
import { showToast } from '../components/toast';
import { generateImage } from '../api/generation';
import { api } from '../api/client';
import type { BundledPack } from '../types/pack';

export function PackOverview() {
  const p = pack.value!;
  const [settingsTab, setSettingsTab] = useState('general');

  return (
    <div style="display:flex;gap:24px;padding:20px;">
      {/* Left: campaigns */}
      <div style="flex:1;min-width:0;">
        <div style="margin-bottom:16px;display:flex;gap:8px;">
          <button class="primary" onClick={handleCreateCampaign}>New Campaign</button>
          <button class="ai-btn" onClick={handleGenerateAI}>Generate Campaign (AI) &#10024;</button>
        </div>
        <div class="card-grid">
          {p.campaigns.map(c => {
            const ow = c.overworld;
            const lvCount = ow.levels ? ow.levels.length : 0;
            return (
              <div key={c.id} class="campaign-card" onClick={() => navigate(c.id)}>
                <div class="card-name">{ow.name}</div>
                <div class="card-desc">{ow.description}</div>
                <div class="card-meta">
                  <span>{lvCount} level{lvCount !== 1 ? 's' : ''}</span>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Right: pack settings */}
      <div style="width:350px;flex-shrink:0;background:var(--panel);border:1px solid var(--border);border-radius:8px;overflow:hidden;max-height:calc(100vh - 120px);display:flex;flex-direction:column;">
        <div class="tab-bar" style="margin:0;">
          {[
            { id: 'general', label: 'General' },
            { id: 'prompts', label: 'Prompts' },
            { id: 'items', label: 'Items' },
          ].map(t => (
            <button
              key={t.id}
              class={settingsTab === t.id ? 'active' : ''}
              onClick={() => setSettingsTab(t.id)}
            >
              {t.label}
            </button>
          ))}
        </div>
        <div style="padding:16px;overflow-y:auto;flex:1;">
          <PackSettingsBody tab={settingsTab} />
        </div>
      </div>
    </div>
  );
}

function PackSettingsBody({ tab }: { tab: string }) {
  const p = pack.value!;
  const s = p.strings || ({} as any);

  if (tab === 'general') {
    return (
      <div>
        <div class="form-row">
          <label>Title</label>
          <EnhancedInput
            value={s.title || ''}
            onChange={(v) => updatePack(p => { p.strings.title = v; })}
            context="one-line game title for a roguelike dungeon pack"
          />
        </div>
        <div class="form-row-stacked">
          <label>Subtitle</label>
          <EnhancedTextarea
            value={s.subtitle || ''}
            onChange={(v) => updatePack(p => { p.strings.subtitle = v; })}
            context="short subtitle tagline for a roguelike game"
            rows={3}
          />
        </div>
        <div class="form-row-stacked">
          <label>Intro lines (one per line)</label>
          <EnhancedTextarea
            value={(s.intro || []).join('\n')}
            onChange={(v) => updatePack(p => { p.strings.intro = v.split('\n').filter(x => x); })}
            context="five-line intro narration for a roguelike game, one sentence per line"
            rows={5}
          />
        </div>
        <div class="form-row-stacked">
          <label>Pack theme</label>
          <EnhancedTextarea
            value={p.theme || ''}
            onChange={(v) => updatePack(p => { p.theme = v || undefined; })}
            context="three-line theme description for a roguelike game pack"
            rows={3}
          />
        </div>
      </div>
    );
  }

  if (tab === 'prompts') {
    const promptFields = [
      { key: 'campaign_cleared', label: 'Campaign cleared (big title on victory screen)', context: 'congratulations message when player clears a campaign' },
      { key: 'campaign_conquered', label: 'Campaign conquered (subtitle, {name} = campaign)', context: 'message when player conquers all campaigns, use {name} for campaign name' },
      { key: 'prompt_first', label: 'First play (no save, never played)', context: 'three-line prompt asking player to press enter to start their first game' },
      { key: 'prompt_next', label: 'Next campaign (completed one, more to go)', context: 'three-line prompt asking player to press enter for the next world' },
      { key: 'prompt_resume', label: 'Resume (has a saved game)', context: 'three-line prompt asking player to press enter to resume' },
      { key: 'prompt_restart', label: 'Restart (all campaigns completed)', context: 'three-line prompt asking player to press enter to play again' },
      { key: 'prompt_after_clear', label: 'After clear (prompt on victory screen)', context: 'three-line prompt shown after clearing all campaigns' },
    ];
    return (
      <div>
        {promptFields.map((f, i) => (
          <div key={f.key}>
            {f.key === 'prompt_first' && <hr class="section-divider" />}
            <div class="form-row-stacked">
              <label>{f.label}</label>
              <EnhancedTextarea
                value={(s as any)[f.key] || ''}
                onChange={(v) => updatePack(p => { (p.strings as any)[f.key] = v; })}
                context={f.context}
                rows={3}
              />
            </div>
          </div>
        ))}
      </div>
    );
  }

  if (tab === 'items') {
    return <ItemSpritesTab />;
  }

  return null;
}

const SPRITE_TYPES = [
  { key: 'potion', defaultName: 'Health Potion', prompt: 'a glowing healing potion bottle' },
  { key: 'speed_potion', defaultName: 'Speed Potion', prompt: 'a swirling speed potion bottle with lightning energy' },
  { key: 'bomb', defaultName: 'Bomb', prompt: 'a round bomb with a lit fuse' },
  { key: 'gold', defaultName: 'Gold', prompt: 'a pile of gold coins' },
  { key: 'key', defaultName: 'Key', prompt: 'an ornate dungeon key' },
];

function ItemSpritesTab() {
  const p = pack.value!;
  const sprites = p.item_sprites || {};
  const names = p.item_names || {};
  const descs = p.item_descriptions || {};

  async function genSprite(key: string) {
    const desc = descs[key] || SPRITE_TYPES.find(s => s.key === key)?.prompt || key;
    showToast('Generating sprite...', 'info');
    const raw = await generateImage({
      prompt: `16x16 pixel art sprite: ${desc}. Single item on solid BLACK background, centered, clean pixel art style.`,
      width: 64,
      height: 64,
    });
    if (raw) {
      const { processSprite } = await import('../canvas/sprite-processing');
      const b64 = await processSprite(raw);
      updatePack(p => {
        if (!p.item_sprites) p.item_sprites = {};
        p.item_sprites[key] = b64;
      });
      showToast('Sprite generated!', 'success');
    }
  }

  return (
    <div>
      {SPRITE_TYPES.map(st => {
        const itemName = names[st.key] || st.defaultName;
        const itemDesc = descs[st.key] || st.prompt;
        return (
          <div key={st.key} style="background:var(--input-bg);border:1px solid var(--border);border-radius:6px;padding:8px;margin-bottom:8px;">
            <div class="flex gap-4 items-center mb-8">
              <EnhancedInput
                value={itemName}
                onChange={(v) => updatePack(p => {
                  if (!p.item_names) p.item_names = {};
                  p.item_names[st.key] = v;
                })}
                context={`one-line item name for a roguelike game item`}
                style="font-size:12px;font-weight:600;"
              />
              <button
                style="font-size:9px;padding:2px 4px;"
                onClick={() => updatePack(p => {
                  if (!p.item_names) p.item_names = {};
                  p.item_names[st.key] = st.defaultName;
                })}
                title={`Reset to ${st.defaultName}`}
              >
                &#x21ba;
              </button>
            </div>
            <EnhancedTextarea
              value={itemDesc}
              onChange={(v) => updatePack(p => {
                if (!p.item_descriptions) p.item_descriptions = {};
                p.item_descriptions[st.key] = v;
              })}
              context={`short visual description of a game item called ${itemName} for pixel art sprite generation`}
              rows={3}
            />
            <div class="flex gap-8 items-center" style="margin-top:6px;">
              {sprites[st.key] ? (
                <>
                  <img
                    src={`data:image/png;base64,${sprites[st.key]}`}
                    style="width:48px;height:48px;border-radius:4px;border:1px solid var(--border);image-rendering:pixelated;"
                  />
                  <button class="ai-btn" onClick={() => genSprite(st.key)} style="font-size:11px;">
                    &#10024; Regen
                  </button>
                  <button
                    style="font-size:10px;padding:2px 6px;"
                    onClick={() => updatePack(p => { delete p.item_sprites[st.key]; })}
                  >
                    Clear
                  </button>
                </>
              ) : (
                <>
                  <div style="width:48px;height:48px;border-radius:4px;border:1px dashed var(--border);display:flex;align-items:center;justify-content:center;font-size:20px;color:#555;">
                    &#127890;
                  </div>
                  <button class="ai-btn" onClick={() => genSprite(st.key)}>
                    &#10024; Generate Sprite
                  </button>
                </>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}

async function handleCreateCampaign() {
  const c = await createCampaign();
  if (c) {
    navigate(c.id);
    showToast('Campaign created', 'success');
  }
}

async function handleGenerateAI() {
  const theme = prompt('Optional theme (leave blank for random):');
  if (theme === null) return;
  showToast('Generating campaign... this may take a moment', 'info');
  try {
    const ow = await api('/api/generate/overworld', { method: 'POST', body: { theme: theme || null } });
    if (!ow) return;
    const newCamp = await createCampaign();
    if (!newCamp) return;
    newCamp.overworld = ow;
    newCamp.designs = [];
    for (let i = 0; i < ow.levels.length; i++) {
      const lv = ow.levels[i];
      showToast(`Generating level ${i + 1}/${ow.levels.length}: ${lv.name}`, 'info');
      const design = await api('/api/generate/level-design', {
        method: 'POST',
        body: {
          campaign_name: ow.name,
          campaign_desc: ow.description,
          level_config: {
            title: lv.name,
            font: lv.font || ow.font || '',
            description: lv.description,
            theme: lv.theme,
            palette: lv.palette || [],
            budget: lv.budget,
            floor: i + 1,
            campaign_tier: 0,
          },
          theme: theme || null,
        },
      });
      if (design) newCamp.designs.push(design);
    }
    pack.value = { ...pack.value! };
    packVersion.value++;
    savePack();
    navigate(newCamp.id);
    showToast('AI campaign generated!', 'success');
  } catch (e: any) {
    showToast('Generation failed: ' + e.message, 'error');
  }
}
