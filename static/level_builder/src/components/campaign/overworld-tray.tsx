import { signal } from '@preact/signals';
import type { BundledCampaign } from '../../types/pack';
import { pack } from '../../store/state';

export interface OverworldTrayItem {
  type: string;
  name: string;
  icon: string;
  image?: string | null;
  /** For signposts: index into campaign.signposts */
  signpost_idx?: number;
}

export const draggedOverworldItem = signal<OverworldTrayItem | null>(null);

const BORDER_COLORS: Record<string, string> = {
  signpost: '#88cc44',
};

export function generateOverworldTray(campaign: BundledCampaign): OverworldTrayItem[] {
  const tray: OverworldTrayItem[] = [];
  const signSprite = pack.value?.item_sprites?.['sign'] || null;
  const placed = campaign.overworld.placed_signposts || [];

  for (let i = 0; i < (campaign.signposts || []).length; i++) {
    const sp = campaign.signposts![i];
    const alreadyPlaced = placed.some(p => p.signpost_idx === i);
    if (!alreadyPlaced) {
      tray.push({
        type: 'signpost',
        name: sp.title,
        icon: '\u{1F4DC}',
        image: signSprite,
        signpost_idx: i,
      });
    }
  }

  return tray;
}

interface TrayProps {
  items: OverworldTrayItem[];
}

export function OverworldTray({ items }: TrayProps) {
  if (items.length === 0) return null;

  return (
    <div class="entity-tray">
      {items.map((t, i) => {
        const bc = BORDER_COLORS[t.type] || 'var(--border)';
        return (
          <div
            key={i}
            class="tray-item"
            draggable
            data-name={t.name}
            style={`border-color:${bc}`}
            onDragStart={(e) => {
              draggedOverworldItem.value = t;
              if (e.dataTransfer) {
                e.dataTransfer.setData('text/plain', JSON.stringify(t));
                e.dataTransfer.effectAllowed = 'copy';
              }
            }}
            title={t.name}
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
