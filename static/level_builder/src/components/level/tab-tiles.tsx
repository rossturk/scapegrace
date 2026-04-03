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
                onChange={(v) => updateDesign(d => {
                  const oldName = d.tile_defs[i].name;
                  d.tile_defs[i].name = v;
                  // Rename all references in prebuilt_map tiles
                  if (d.prebuilt_map?.tiles && oldName !== v) {
                    for (const row of d.prebuilt_map.tiles) {
                      for (let x = 0; x < row.length; x++) {
                        if (row[x] === oldName) row[x] = v;
                      }
                    }
                  }
                })}
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
              <button style="padding:2px 6px;font-size:10px;" onClick={() => updateDesign(d => {
                const removedName = d.tile_defs[i].name;
                d.tile_defs.splice(i, 1);
                // Replace removed tile's references in prebuilt_map with first remaining floor tile
                if (d.prebuilt_map?.tiles) {
                  const fallback = d.tile_defs.length > 1 ? d.tile_defs[1].name : (d.tile_defs[0]?.name || 'wall');
                  for (const row of d.prebuilt_map.tiles) {
                    for (let x = 0; x < row.length; x++) {
                      if (row[x] === removedName) row[x] = fallback;
                    }
                  }
                }
              })}>x</button>
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
  const isWall = tileIdx === 0;
  showToast('Generating tile image...', 'info');
  const raw = await generateImage({
    prompt: `Generate a single square tile texture for a top-down 2D roguelike game. ` +
      `The tile is: "${name}". ${isWall ? 'It is a solid wall or obstacle — should look dense and impassable.' : 'It is a walkable floor tile — should look open and traversable.'} ` +
      `STYLE REQUIREMENTS: Simple flat pixel art. Use color ${color} as the dominant color. ` +
      `Bright and saturated, NOT dark or muddy. Minimal detail — just enough to suggest the material. ` +
      `Perfectly seamless and tileable in all directions. Uniform texture with NO focal point, NO objects, NO borders, NO text. ` +
      `Think classic SNES/GBA RPG tile. Fill the ENTIRE image with the texture edge to edge.`,
    width: 64,
    height: 64,
  });
  if (raw) {
    const { blendWithColor } = await import('../../canvas/sprite-processing');
    const b64 = await blendWithColor(raw, color, 0.3);
    updateDesign(d => { d.tile_defs[tileIdx].image = b64; });
    showToast('Tile image generated!', 'success');
  }
}
