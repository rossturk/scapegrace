import type { BundledCampaign, Phase2Result, PlacedEntities } from '../../types/pack';
import { pack } from '../../store/state';

export interface TrayItem {
  type: string;
  name: string;
  icon: string;
  image?: string | null;
  item_type: string;
}

const BORDER_COLORS: Record<string, string> = {
  boss: '#e94560', monster: '#ff69b4', weapon: '#ffa726', armor: '#ffa726',
  gold: '#ffa726', potion: '#ffa726', key: '#4dd0e1', bomb: '#ffa726',
  speed_potion: '#ffa726', trap: '#ab47bc', exit_door: '#ffcc00', entry_door: '#44ccff',
};

function packItemName(key: string): string {
  const p = pack.value;
  const defaults: Record<string, string> = { gold: 'Gold', potion: 'Health Potion', key: 'Key', bomb: 'Bomb', speed_potion: 'Speed Potion' };
  return p?.item_names?.[key] || defaults[key] || key;
}

function packItemSprite(key: string): string | null {
  return pack.value?.item_sprites?.[key] || null;
}

export function generateTray(campaign: BundledCampaign, levelIdx: number, placed: PlacedEntities): TrayItem[] {
  const lv = campaign.overworld.levels[levelIdx];
  const design = campaign.designs[levelIdx] || {} as any;
  const budget = lv.budget || 150;
  const monsters = campaign.monster_templates || design.monster_types || [];
  const trapDefs = design.traps || [];

  let rem = budget;
  const tray: TrayItem[] = [];

  // Boss: ~20%
  const bossCost = Math.round(budget * 0.20);
  rem -= bossCost;
  tray.push({ type: 'boss', name: design.boss?.name || 'Boss', icon: '\u{1F480}', image: design.boss?.image, item_type: 'boss' });

  // Monsters: ~60%
  const monBudget = Math.round(rem * 0.6);
  const goldBudget = Math.round(rem * 0.25);
  const trapBudget = rem - monBudget - goldBudget;

  if (monsters.length > 0) {
    let mi = 0, mrem = monBudget;
    while (mrem >= 5) {
      mrem -= 6;
      const tmpl = monsters[mi % monsters.length];
      tray.push({ type: 'monster', name: tmpl.name, icon: '\u{1F47E}', image: tmpl.image, item_type: 'monster' });
      mi++;
    }
  }

  // Gold
  let grem = goldBudget, gi = 0;
  while (grem >= 3 && gi < 6) {
    grem -= 3;
    tray.push({ type: 'item', name: packItemName('gold'), icon: '\u{1F4B0}', item_type: 'gold', image: packItemSprite('gold') });
    gi++;
  }

  // Traps
  if (trapDefs.length > 0) {
    let trem = trapBudget, ti = 0;
    while (trem >= 5) {
      trem -= 5;
      const td = trapDefs[ti % trapDefs.length];
      tray.push({ type: 'trap', name: td.name || 'Trap', icon: '\u26A0', item_type: 'trap', image: td.image });
      ti++;
    }
  }

  // Free items
  tray.push({ type: 'item', name: design.weapon?.name || 'Weapon', icon: '\u2694', item_type: 'weapon', image: design.weapon?.image });
  tray.push({ type: 'item', name: design.armor?.name || 'Armor', icon: '\u{1F6E1}', item_type: 'armor', image: design.armor?.image });
  const monCount = tray.filter(t => t.type === 'monster').length;
  const potionCount = Math.max(1, Math.floor(monCount / 6));
  for (let i = 0; i < potionCount; i++) {
    tray.push({ type: 'item', name: packItemName('potion'), icon: '\u{1F9EA}', item_type: 'potion', image: packItemSprite('potion') });
  }
  if (design.prebuilt_map?.key_position) {
    tray.push({ type: 'item', name: packItemName('key'), icon: '\u{1F511}', item_type: 'key', image: packItemSprite('key') });
  }

  tray.push({ type: 'exit_door', name: 'Exit Door', icon: '\u{1F6AA}', item_type: 'exit_door' });
  tray.push({ type: 'entry_door', name: 'Entry Door', icon: '\u{1F535}', item_type: 'entry_door' });

  // Remove already-placed
  const placedMonsters = [...(placed.monsters || [])];
  const placedItems = [...(placed.items || [])];
  const placedTraps = [...(placed.traps || [])];

  return tray.filter(t => {
    if (t.type === 'exit_door') return !placed.exit_door;
    if (t.type === 'entry_door') return !placed.entry_door;
    if (t.type === 'boss') return !placed.boss;
    if (t.type === 'monster') {
      const idx = placedMonsters.findIndex(pm => pm.name === t.name);
      if (idx >= 0) { placedMonsters.splice(idx, 1); return false; }
    }
    if (t.type === 'item') {
      const idx = placedItems.findIndex(pi => pi.item_type === t.item_type);
      if (idx >= 0) { placedItems.splice(idx, 1); return false; }
    }
    if (t.type === 'trap') {
      const idx = placedTraps.findIndex(pt => pt.name === t.name);
      if (idx >= 0) { placedTraps.splice(idx, 1); return false; }
    }
    return true;
  });
}

interface TrayProps {
  items: TrayItem[];
  onDragStart: (idx: number, e: DragEvent) => void;
  onAutoPlace: (idx: number) => void;
}

export function EntityTray({ items, onDragStart, onAutoPlace }: TrayProps) {
  if (items.length === 0) return null;

  return (
    <div class="entity-tray">
      {items.map((t, i) => {
        const bc = BORDER_COLORS[t.item_type] || BORDER_COLORS[t.type] || 'var(--border)';
        return (
          <div
            key={i}
            class="tray-item"
            draggable
            data-name={t.name}
            style={`border-color:${bc}`}
            onDragStart={(e) => onDragStart(i, e as any)}
            onDblClick={() => onAutoPlace(i)}
          >
            <span class="tray-icon">
              {t.image ? (
                <img src={`data:image/png;base64,${t.image}`} style="width:28px;height:28px;image-rendering:pixelated;" />
              ) : t.icon}
            </span>
          </div>
        );
      })}
    </div>
  );
}
