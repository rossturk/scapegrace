import { useState } from 'preact/hooks';
import type { BundledCampaign } from '../../types/pack';
import { updateOverworld, updateCampaign, deleteCampaign, navigate } from '../../store/actions';
import { EnhancedInput, EnhancedTextarea } from '../enhanced-input';
import { FontSelect } from '../font-picker';
import { showToast } from '../toast';
import { generateImage, generateDescription } from '../../api/generation';
import { loadGoogleFont } from '../../api/fonts';

import type { OverworldNode } from '../../types/pack';

function formatNodeId(id: string, levels: OverworldNode[]): string {
  if (id === 'start') return 'Start';
  if (id === 'store') return 'Store';
  const m = id.match(/^level_(\d+)$/);
  if (m) {
    const idx = parseInt(m[1]);
    return levels[idx]?.name || `Level ${idx + 1}`;
  }
  return id;
}

interface Props {
  campaign: BundledCampaign;
}

export function LeftPanel({ campaign }: Props) {
  const [tab, setTab] = useState('overview');
  const ow = campaign.overworld;

  if (ow.font) loadGoogleFont(ow.font);
  if (ow.description_font) loadGoogleFont(ow.description_font);
  if (ow.label_font) loadGoogleFont(ow.label_font);

  const tabs = [
    { id: 'overview', label: 'Overview' },
    { id: 'monsters', label: 'Monsters' },
    { id: 'settings', label: 'Store & Rules' },
  ];

  return (
    <div class="left-panel">
      <div class="tab-bar" style="margin:-16px -16px 12px -16px;">
        {tabs.map(t => (
          <button
            key={t.id}
            class={tab === t.id ? 'active' : ''}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>
      {tab === 'overview' && <OverviewTab campaign={campaign} />}
      {tab === 'monsters' && <MonstersTab campaign={campaign} />}
      {tab === 'settings' && <SettingsTab campaign={campaign} />}
    </div>
  );
}

function OverviewTab({ campaign }: Props) {
  const ow = campaign.overworld;
  const [bgMode, setBgMode] = useState(
    ow.bg_image ? 'image' : (ow.bg_gradient ? 'gradient' : 'solid')
  );

  return (
    <div>
      <h3 style="margin-bottom:8px;color:#888;text-transform:uppercase;font-size:10px;letter-spacing:1px;">Basic</h3>
      <div class="form-row-stacked">
        <label>Campaign Name</label>
        <EnhancedInput
          value={ow.name}
          onChange={(v) => updateOverworld(o => { o.name = v; })}
          context="campaign name for a roguelike game"
        />
      </div>
      <div class="form-row-stacked">
        <label>Description</label>
        <EnhancedTextarea
          value={ow.description}
          onChange={(v) => updateOverworld(o => { o.description = v; })}
          context="five-line campaign overworld description"
          rows={5}
        />
      </div>
      <div class="form-row">
        <label>Text Color</label>
        <input
          type="color"
          value={ow.text_color || '#e8d8c8'}
          onChange={(e) => updateOverworld(o => { o.text_color = (e.target as HTMLInputElement).value; })}
        />
        <span style="font-size:11px;color:#888;">{ow.text_color || '#e8d8c8'}</span>
      </div>

      <hr class="section-divider" />
      <h3 style="margin-bottom:8px;color:#888;text-transform:uppercase;font-size:10px;letter-spacing:1px;">Typography</h3>
      <div class="form-row">
        <label>Title Font</label>
        <FontSelect current={ow.font || ''} onChange={(v) => updateOverworld(o => { o.font = v || undefined; })} />
      </div>
      <div class="form-row">
        <label>Desc Font</label>
        <FontSelect current={ow.description_font || ''} onChange={(v) => updateOverworld(o => { o.description_font = v || undefined; })} />
      </div>
      <div class="form-row">
        <label>Label Font</label>
        <FontSelect current={ow.label_font || ''} onChange={(v) => updateOverworld(o => { o.label_font = v || undefined; })} />
      </div>

      <hr class="section-divider" />
      <h3 style="margin-bottom:8px;color:#888;text-transform:uppercase;font-size:10px;letter-spacing:1px;">Background</h3>
      <div class="flex gap-4 mb-8">
        {['solid', 'gradient', 'image', 'terrain'].map(mode => (
          <button
            key={mode}
            style={`flex:1;font-size:11px;${bgMode === mode ? 'background:var(--accent);border-color:var(--accent);' : ''}`}
            onClick={() => {
              setBgMode(mode);
              updateOverworld(o => {
                if (mode === 'solid') { o.bg_gradient = undefined; o.bg_image = undefined; }
                else if (mode === 'gradient') { o.bg_image = undefined; if (!o.bg_gradient) o.bg_gradient = 'linear-gradient(135deg, #0a0a2e, #1a0a0a)'; }
                else if (mode === 'image') { o.bg_gradient = undefined; }
                else if (mode === 'terrain') { o.bg_gradient = undefined; o.bg_image = undefined; }
              });
            }}
          >
            {mode.charAt(0).toUpperCase() + mode.slice(1)}
          </button>
        ))}
      </div>
      {bgMode === 'solid' && (
        <div class="form-row">
          <label>Color</label>
          <input
            type="color"
            value={ow.bg_color || '#0a0a0a'}
            onChange={(e) => updateOverworld(o => { o.bg_color = (e.target as HTMLInputElement).value; })}
          />
        </div>
      )}
      {bgMode === 'terrain' && (
        <div>
          <div class="form-row">
            <label>Base Color</label>
            <input
              type="color"
              value={ow.bg_color || '#0a0a0a'}
              onChange={(e) => updateOverworld(o => { o.bg_color = (e.target as HTMLInputElement).value; })}
            />
          </div>
          <button onClick={() => updateOverworld(o => { o.terrain_seed = Math.floor(Math.random() * 999999); })} style="margin-top:6px;">
            Regenerate
          </button>
        </div>
      )}

      <hr class="section-divider" />
      <p class="note" style="margin-bottom:8px;">Connections: {ow.connections?.length || 0}. Right-click a hallway to remove it. Drag exit (red) to entry (cyan) handles to connect.</p>
      <hr class="section-divider" />
      <button class="danger" onClick={async () => {
        if (confirm('Delete this campaign?')) {
          await deleteCampaign(campaign.id);
          navigate(null);
        }
      }}>
        Delete Campaign
      </button>
    </div>
  );
}

function MonstersTab({ campaign }: Props) {
  const cm = campaign.monster_templates || [];

  return (
    <div>
      <p class="note mb-8">Monsters shared across all levels.</p>
      {cm.map((monster, i) => (
        <div key={i} style="background:var(--input-bg);border:1px solid var(--border);border-radius:6px;padding:8px;margin-bottom:8px;">
          <div class="flex gap-4 items-center mb-8">
            <EnhancedInput
              value={monster.name}
              onChange={(v) => updateCampaign(c => {
                if (c.monster_templates?.[i]) c.monster_templates[i].name = v;
              })}
              context="monster name for roguelike campaign"
              style="font-size:12px;font-weight:600;"
            />
            <button
              class="danger"
              onClick={() => updateCampaign(c => { c.monster_templates?.splice(i, 1); })}
              style="padding:2px 6px;font-size:10px;"
            >
              x
            </button>
          </div>
          <EnhancedTextarea
            value={monster.description || ''}
            onChange={(v) => updateCampaign(c => {
              if (c.monster_templates?.[i]) c.monster_templates[i].description = v;
            })}
            context={`short five-line monster description for a creature called ${monster.name}`}
            rows={5}
          />
          <div class="flex gap-8 items-center" style="margin-top:6px;">
            {monster.image ? (
              <>
                <img
                  src={`data:image/png;base64,${monster.image}`}
                  style="width:128px;height:128px;border-radius:4px;border:1px solid var(--border);image-rendering:pixelated;"
                />
                <button
                  class="ai-btn"
                  onClick={() => genMonsterSprite(campaign, i)}
                  style="font-size:11px;"
                >
                  &#10024; Regen
                </button>
                <button
                  style="font-size:10px;padding:2px 6px;"
                  onClick={() => updateCampaign(c => {
                    if (c.monster_templates?.[i]) c.monster_templates[i].image = undefined;
                  })}
                >
                  Clear
                </button>
              </>
            ) : (
              <>
                <div style="width:48px;height:48px;border-radius:4px;border:1px dashed var(--border);display:flex;align-items:center;justify-content:center;font-size:20px;color:#555;">
                  &#128126;
                </div>
                <button class="ai-btn" onClick={() => genMonsterSprite(campaign, i)}>
                  &#10024; Generate Sprite
                </button>
              </>
            )}
          </div>
        </div>
      ))}
      <button
        onClick={() => updateCampaign(c => {
          if (!c.monster_templates) c.monster_templates = [];
          c.monster_templates.push({ name: 'New Monster', hp: 10, attack: 3 });
        })}
        style="font-size:12px;"
      >
        + Add Monster
      </button>
    </div>
  );
}

async function genMonsterSprite(campaign: BundledCampaign, idx: number) {
  const monster = campaign.monster_templates?.[idx];
  if (!monster) return;
  showToast('Generating sprite...', 'info');
  const raw = await generateImage({
    prompt: `16x16 pixel art sprite: ${monster.description || monster.name}. Single creature on solid BLACK background, centered, clean pixel art style.`,
    width: 256,
    height: 256,
  });
  if (raw) {
    const { processSprite } = await import('../../canvas/sprite-processing');
    const b64 = await processSprite(raw);
    updateCampaign(c => {
      if (c.monster_templates?.[idx]) c.monster_templates[idx].image = b64;
    });
    showToast('Sprite generated!', 'success');
  }
}

function SettingsTab({ campaign }: Props) {
  const store = campaign.overworld.store || {};
  const st = campaign.settings || {} as any;

  return (
    <div>
      <h3 style="margin-bottom:8px;">Store Inventory</h3>
      <div class="form-row">
        <label>Healing Potions</label>
        <input
          type="number"
          value={store.healing_potions || 0}
          min={0}
          onChange={(e) => updateOverworld(o => {
            if (!o.store) o.store = {};
            o.store.healing_potions = +(e.target as HTMLInputElement).value;
          })}
        />
      </div>
      <div class="form-row">
        <label>Speed Potions</label>
        <input
          type="number"
          value={store.speed_potions || 0}
          min={0}
          onChange={(e) => updateOverworld(o => {
            if (!o.store) o.store = {};
            o.store.speed_potions = +(e.target as HTMLInputElement).value;
          })}
        />
      </div>
      <div class="form-row">
        <label>Bombs</label>
        <input
          type="number"
          value={store.bombs || 0}
          min={0}
          onChange={(e) => updateOverworld(o => {
            if (!o.store) o.store = {};
            o.store.bombs = +(e.target as HTMLInputElement).value;
          })}
        />
      </div>
      <hr class="section-divider" />
      <h3 style="margin-bottom:8px;">Difficulty Rules</h3>
      <div class="form-row">
        <label>Locked doors from level</label>
        <input
          type="number"
          value={st.locked_doors_from_level || 3}
          min={1}
          max={99}
          onChange={(e) => updateCampaign(c => {
            if (!c.settings) c.settings = {} as any;
            c.settings.locked_doors_from_level = +(e.target as HTMLInputElement).value;
          })}
        />
      </div>
      <div class="form-row">
        <label>Traps from level</label>
        <input
          type="number"
          value={st.traps_from_level || 2}
          min={1}
          max={99}
          onChange={(e) => updateCampaign(c => {
            if (!c.settings) c.settings = {} as any;
            c.settings.traps_from_level = +(e.target as HTMLInputElement).value;
          })}
        />
      </div>
      <div class="form-row">
        <label>Damage tiles from level</label>
        <input
          type="number"
          value={st.damage_tiles_from_level || 4}
          min={1}
          max={99}
          onChange={(e) => updateCampaign(c => {
            if (!c.settings) c.settings = {} as any;
            c.settings.damage_tiles_from_level = +(e.target as HTMLInputElement).value;
          })}
        />
      </div>
      <div class="form-row">
        <label>Damage per tile</label>
        <input
          type="number"
          value={st.damage_tile_damage || 3}
          min={1}
          max={99}
          onChange={(e) => updateCampaign(c => {
            if (!c.settings) c.settings = {} as any;
            c.settings.damage_tile_damage = +(e.target as HTMLInputElement).value;
          })}
        />
      </div>
    </div>
  );
}
