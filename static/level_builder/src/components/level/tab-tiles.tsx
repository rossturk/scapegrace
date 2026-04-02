import type { Phase2Result } from '../../types/pack';
import { updateDesign, updateOverworld } from '../../store/actions';
import { selectedLevelIdx } from '../../store/state';
import { EnhancedInput } from '../enhanced-input';
import { showToast } from '../toast';
import { generateImage } from '../../api/generation';

interface Props {
  design: Phase2Result;
  palette: string[];
}

export function TabTiles({ design, palette }: Props) {
  const defs = design.tile_defs || [];
  const idx = selectedLevelIdx.value!;

  return (
    <div>
      {defs.map((td, i) => {
        const col = palette[i] || '#333';
        return (
          <div key={i} style="background:var(--input-bg);border:1px solid var(--border);border-radius:6px;padding:8px;margin-bottom:8px;">
            <div class="flex gap-4 items-center mb-8">
              <div class="color-swatch" style={`background:${col};flex-shrink:0;`} />
              <EnhancedInput
                value={td.name || ''}
                onChange={(v) => updateDesign(d => { d.tile_defs[i].name = v; })}
                context="tile type name for roguelike dungeon"
                style="flex:1;font-weight:600;"
              />
              <input
                value={td.char || ''}
                maxLength={1}
                style="width:30px;text-align:center;"
                onChange={(e) => updateDesign(d => { d.tile_defs[i].char = (e.target as HTMLInputElement).value; })}
                placeholder="Ch"
              />
              <input
                type="color"
                value={col}
                onChange={(e) => {
                  const newCol = (e.target as HTMLInputElement).value;
                  updateOverworld(ow => {
                    const pal = ow.levels[idx].palette || [];
                    while (pal.length <= i) pal.push('#555555');
                    pal[i] = newCol;
                    ow.levels[idx].palette = pal;
                  });
                }}
              />
              <button style="padding:2px 6px;font-size:10px;" onClick={() => updateDesign(d => { d.tile_defs.splice(i, 1); })}>x</button>
            </div>
            <div class="flex gap-8 items-center">
              {td.image ? (
                <>
                  <img src={`data:image/png;base64,${td.image}`} style="width:48px;height:48px;border-radius:4px;border:1px solid var(--border);image-rendering:pixelated;" />
                  <button class="ai-btn" onClick={() => genTileImage(i, td.name, col)} style="font-size:11px;">&#10024; Regen</button>
                  <button style="font-size:10px;padding:2px 6px;" onClick={() => updateDesign(d => { delete (d.tile_defs[i] as any).image; })}>Clear</button>
                </>
              ) : (
                <>
                  <div style={`width:48px;height:48px;border-radius:4px;border:1px dashed var(--border);display:flex;align-items:center;justify-content:center;font-size:20px;color:#555;background:${col}`}>
                    {td.char || ''}
                  </div>
                  <button class="ai-btn" onClick={() => genTileImage(i, td.name, col)}>&#10024; Generate Tile Image</button>
                </>
              )}
            </div>
            <p class="note" style="margin-top:4px;">{i === 0 ? 'Wall (not walkable)' : 'Walkable'}</p>
          </div>
        );
      })}
      <button style="font-size:12px;" onClick={() => updateDesign(d => {
        d.tile_defs.push({ name: 'new_tile', char: '.' });
      })}>
        + Add Tile Def
      </button>
    </div>
  );
}

async function genTileImage(tileIdx: number, name: string, color: string) {
  showToast('Generating tile image...', 'info');
  const b64 = await generateImage({
    prompt: `16x16 pixel art tilemap tile: ${name}. Color scheme: ${color}. Top-down dungeon tile, clean pixel art.`,
    width: 64,
    height: 64,
  });
  if (b64) {
    updateDesign(d => { d.tile_defs[tileIdx].image = b64; });
    showToast('Tile image generated!', 'success');
  }
}
