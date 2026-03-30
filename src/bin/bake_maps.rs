/// One-shot tool: generate and bake prebuilt maps into every level of campaigns.json.
/// After this, the game uses the saved maps directly — no runtime map generation.

use scapegrace::gen::{BundledPack, CampaignSettings, expand_tile_defs};
use scapegrace::mapgen::generate_map_with_options;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "campaigns.json".into());
    eprintln!("Loading {}...", path);

    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path, e));
    let mut pack: BundledPack = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", path, e));

    let total_campaigns = pack.campaigns.len();
    let mut total_levels = 0usize;
    let mut generated = 0usize;
    let mut skipped = 0usize;

    for (ci, campaign) in pack.campaigns.iter_mut().enumerate() {
        let settings = &campaign.settings;
        let levels = &campaign.overworld.levels;
        let num_levels = levels.len();
        let num_designs = campaign.designs.len();

        for li in 0..num_levels.min(num_designs) {
            total_levels += 1;
            let design = &mut campaign.designs[li];

            if design.prebuilt_map.is_some() {
                skipped += 1;
                continue;
            }

            let level = &levels[li];
            let palette = level.palette.as_deref().unwrap_or(&[]);
            let full_defs = expand_tile_defs(&design.tile_defs, palette);
            let level_num = (li + 1) as u8;
            let skip_locked_door = level_num < settings.locked_doors_from_level;
            let map = generate_map_with_options(&full_defs, skip_locked_door);

            design.prebuilt_map = Some(map);
            generated += 1;

            eprint!("\r[{}/{}] Campaign {}: generated level {}/{}    ",
                ci + 1, total_campaigns, &campaign.overworld.name[..campaign.overworld.name.len().min(30)],
                li + 1, num_levels);
        }
    }

    eprintln!("\n\nDone! {} levels generated, {} already had maps, {} total.",
        generated, skipped, total_levels);

    eprintln!("Writing {}...", path);
    let json = serde_json::to_string(&pack).unwrap();
    std::fs::write(&path, &json).unwrap();
    eprintln!("Saved. ({:.1} MB)", json.len() as f64 / 1_000_000.0);
}
