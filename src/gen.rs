use crate::game::*;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

// ── Phase 1: Universe (title, description, font, colors, tile_defs) ──

#[derive(Deserialize, Clone)]
pub struct Phase1Result {
    pub title: String,
    pub description: String,
    pub font: Option<String>,
    pub tile_defs: HashMap<String, TileDefRaw>,
}

#[derive(Deserialize, Clone)]
pub struct TileDefRaw {
    pub name: String,
    pub color: String,
    pub walkable: bool,
    #[serde(default)]
    pub char: Option<String>,
}

// ── Phase 2: Objects (boss, monsters, weapon, armor, traps, budget) ──

#[derive(Deserialize)]
pub struct Phase2Result {
    pub tile_defs: HashMap<String, TileDefRaw>,
    pub boss: MonsterRaw,
    pub monster_types: Vec<MonsterTemplateRaw>,
    pub weapon: ItemTemplateRaw,
    pub armor: ItemTemplateRaw,
    pub traps: Option<Vec<TrapRaw>>,
    pub budget_spent: Option<serde_json::Value>,
    pub mode: Option<ModeRaw>,
}

#[derive(Deserialize)]
pub struct ModeRaw {
    pub root: String,
    pub scale: String,
}

#[derive(Deserialize)]
pub struct MonsterRaw {
    pub name: String,
    pub sprite: Option<String>,
    pub hp: i32,
    pub attack: i32,
    pub defense: Option<i32>,
    pub xp_value: Option<i32>,
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct MonsterTemplateRaw {
    pub name: String,
    pub sprite: Option<String>,
    pub hp: i32,
    pub attack: i32,
    pub defense: Option<i32>,
    pub xp_value: Option<i32>,
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct ItemTemplateRaw {
    pub name: String,
    pub sprite: Option<String>,
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct TrapRaw {
    #[allow(dead_code)]
    pub x: Option<i32>,
    #[allow(dead_code)]
    pub y: Option<i32>,
    pub damage: Option<i32>,
    pub name: Option<String>,
}

// ── Phase 3: World (grid, player_start, boss position) ──

#[derive(Deserialize)]
pub struct Phase3Result {
    pub grid: Vec<String>,
    pub player_start: [i32; 2],
    pub boss_position: [i32; 2],
}

// ── Overworld result ──

#[derive(Deserialize)]
pub struct OverworldNodeRaw {
    pub name: String,
    pub font: Option<String>,
    pub description: String,
    pub theme: String,
    pub color: Option<String>,
    pub palette: Option<Vec<String>>,
    pub budget: i32,
    pub x: f32,
    pub y: f32,
}

#[derive(Deserialize)]
pub struct OverworldResult {
    pub name: String,
    pub font: Option<String>,
    pub description: String,
    pub levels: Vec<OverworldNodeRaw>,
    pub connections: Vec<(usize, usize)>,
    pub final_level: Option<usize>,
}

/// Config passed from overworld node to level generation
pub struct LevelConfig {
    pub title: String,
    pub font: String,
    pub description: String,
    pub theme: String,
    pub palette: Vec<String>,
    pub budget: i32,
    pub floor: i32,
}

// ── Phase status (sent to client) ──

#[derive(Clone, serde::Serialize)]
pub struct PhaseUpdate {
    pub phase: String,
    pub detail: String,
}

// ── LLM caller ──

fn call_llm_streaming<F>(
    client: &reqwest::blocking::Client, api_key: &str, model: &str, prompt: &str,
    on_token: Option<F>,
) -> Result<String, String>
where F: Fn()
{
    use std::io::BufRead;

    let resp = client.post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 1.0,
            "stream": on_token.is_some(),
        }))
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .map_err(|e| format!("HTTP error: {}", e))?;

    if on_token.is_none() {
        // Non-streaming path
        let body: serde_json::Value = resp.json()
            .map_err(|e| format!("JSON parse error: {}", e))?;
        let content = body["choices"][0]["message"]["content"]
            .as_str()
            .ok_or("No content in response")?
            .trim()
            .to_string();
        return Ok(clean_llm_content(content));
    }

    // Streaming SSE path
    let on_token = on_token.unwrap();
    let reader = std::io::BufReader::new(resp);
    let mut content = String::new();

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Stream read error: {}", e))?;
        if !line.starts_with("data: ") { continue; }
        let data = &line[6..];
        if data == "[DONE]" { break; }
        if let Ok(chunk) = serde_json::from_str::<serde_json::Value>(data) {
            if let Some(delta) = chunk["choices"][0]["delta"]["content"].as_str() {
                content.push_str(delta);
                on_token();
            }
        }
    }

    Ok(clean_llm_content(content))
}

fn clean_llm_content(mut content: String) -> String {
    if content.starts_with("```") {
        if let Some(rest) = content.split_once('\n') {
            content = rest.1.to_string();
        }
    }
    if content.ends_with("```") {
        content = content.rsplit_once("```")
            .map_or(content.clone(), |(before, _)| before.to_string());
    }
    content = content.trim().to_string();
    if !content.starts_with('{') {
        if let Some(start) = content.find('{') {
            if let Some(end) = content.rfind('}') {
                content = content[start..=end].to_string();
            }
        }
    }
    content
}

// ── Overworld generation ──

pub fn generate_overworld<F, T>(
    mut on_phase: F,
    on_token: T,
) -> Result<crate::game::Overworld, String>
where F: FnMut(PhaseUpdate) + Send, T: Fn() + Send
{
    let api_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
    let model = std::env::var("ALLMUDDY_MODEL").unwrap_or_else(|_| "anthropic/claude-sonnet-4".into());
    let client = reqwest::blocking::Client::new();

    let prompt = build_overworld_prompt();
    let content = call_llm_streaming(&client, &api_key, &model, &prompt, Some(on_token))?;
    let result: OverworldResult = serde_json::from_str(&content)
        .map_err(|e| format!("Overworld parse error: {}\n\nRaw: {}", e, &content[..content.len().min(500)]))?;

    if result.levels.len() < 5 || result.levels.len() > 8 {
        return Err(format!("Expected 5-8 levels, got {}", result.levels.len()));
    }

    let ow_font = result.font.ok_or("LLM did not provide an overworld font")?;

    let final_level = result.final_level.unwrap_or(result.levels.len() - 1);
    let nodes: Vec<crate::game::OverworldNode> = result.levels.into_iter().enumerate().map(|(i, n)| {
        crate::game::OverworldNode {
            name: n.name,
            font: n.font.unwrap_or_else(|| ow_font.clone()),
            description: n.description,
            theme: n.theme,
            palette: n.palette.or_else(|| n.color.map(|c| vec![c])).unwrap_or_else(|| vec!["#888888".into()]),
            budget: n.budget,
            x: n.x.clamp(0.0, 1.0),
            y: n.y.clamp(0.0, 1.0),
            completed: false,
            unlocked: i == 0,
            is_final: i == final_level,
        }
    }).collect();

    let overworld = crate::game::Overworld {
        name: result.name,
        font: ow_font,
        description: result.description,
        connections: result.connections,
        current_node: 0,
        nodes,
    };

    on_phase(PhaseUpdate {
        phase: "overworld designed".into(),
        detail: format!("{} — {} levels", overworld.name, overworld.nodes.len()),
    });

    Ok(overworld)
}

// ── Three-phase generation ──

pub fn generate_level<F, T>(
    config: &LevelConfig, player: &Player,
    mut on_phase: F,
    on_token: T,
) -> Result<(Level, [i32; 2], i32), String>
where F: FnMut(PhaseUpdate) + Send, T: Fn() + Send
{
    let api_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
    let model = std::env::var("ALLMUDDY_MODEL").unwrap_or_else(|_| "anthropic/claude-sonnet-4".into());
    let client = reqwest::blocking::Client::new();
    let theme = &config.theme;
    let floor = config.floor;
    let budget = config.budget;

    // ── Phase 1: Objects + tile_defs (single LLM call) ──
    on_phase(PhaseUpdate { phase: "designing level".into(), detail: String::new() });

    let p2_prompt = build_phase2_prompt(floor, player, budget, theme, &config.title, &config.description, &config.palette);
    let p2_content = call_llm_streaming(&client, &api_key, &model, &p2_prompt, Some(&on_token))?;
    let p2: Phase2Result = serde_json::from_str(&p2_content)
        .map_err(|e| format!("Phase 1 parse error: {}", e))?;

    // Build Phase1Result from config + Phase 2 tile_defs
    let p1 = Phase1Result {
        title: config.title.clone(),
        description: config.description.clone(),
        font: Some(config.font.clone()),
        tile_defs: p2.tile_defs.clone(),
    };

    let trap_count = p2.traps.as_ref().map_or(0, |t| t.len());
    let mon_count = p2.monster_types.len();
    eprintln!("Phase 1: '{}' — boss '{}', {} monster types, {} traps, weapon '{}', armor '{}'",
        p1.title, p2.boss.name, mon_count, trap_count, p2.weapon.name, p2.armor.name);
    on_phase(PhaseUpdate {
        phase: "level designed".into(),
        detail: format!("boss: {} · {} monster types · {} traps · {} · {}",
            p2.boss.name, mon_count, trap_count, p2.weapon.name, p2.armor.name),
    });

    // ── Phase 3: Build world (full context one-shot, with retries) ──
    let tile_chars: Vec<String> = p1.tile_defs.keys().cloned().collect();
    let max_map_attempts = 5;

    for attempt in 1..=max_map_attempts {
        on_phase(PhaseUpdate {
            phase: "building world".into(),
            detail: if attempt > 1 { format!("attempt {}", attempt) } else { String::new() },
        });

        let p3_prompt = build_phase3_prompt(floor, theme, &p1, &p2, &tile_chars);
        let p3_content = match call_llm_streaming(&client, &api_key, &model, &p3_prompt, Some(&on_token)) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Phase 3 LLM error (attempt {}): {}", attempt, e);
                continue;
            }
        };
        let p3: Phase3Result = match serde_json::from_str(&p3_content) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Phase 3 parse error (attempt {}): {}", attempt, e);
                continue;
            }
        };

        match assemble_level(floor, budget, &p1, &p2, &p3) {
            Ok((level, start, remaining)) => {
                on_phase(PhaseUpdate { phase: "world built".into(), detail: String::new() });
                return Ok((level, start, remaining));
            }
            Err(e) => {
                eprintln!("Phase 3 validation failed (attempt {}): {}", attempt, e);
                continue;
            }
        }
    }

    Err(format!("Failed to build valid map after {} attempts", max_map_attempts))
}

// ── Prompt builders ──

fn build_phase2_prompt(floor: i32, player: &Player, budget: i32, theme: &str, title: &str, description: &str, palette: &[String]) -> String {
    let mut p = String::new();
    p.push_str(&format!("Generate the TILE DEFINITIONS and OBJECTS for level {} of a roguelike game.\n\n", floor));
    p.push_str(&format!("Theme: {} — \"{}\"\n", theme, title));
    p.push_str(&format!("{}\n\n", description));

    p.push_str("You are ADVERSARIAL — your goal is to kill the player.\n");
    p.push_str(&format!("Player: level {}, {}/{} HP, ATK {}, DEF {}, weapon '{}' (+{}), armor '{}' (+{}), {} potions.\n\n",
        player.level, player.hp, player.max_hp,
        player.attack + player.weapon_damage, player.defense + player.armor_defense,
        player.weapon, player.weapon_damage, player.armor, player.armor_defense, player.potions));

    p.push_str(&format!("BUDGET: {} scapebux.\n", budget));
    p.push_str("  Spend: Boss 25, Monster 10 each, Trap 6 each.\n");
    p.push_str("  Earn back: Weapon +15, Armor +10, Potion +5, Gold +3.\n");
    p.push_str("  Unspent carries over to the next level.\n\n");

    p.push_str("Return a JSON object with:\n");
    p.push_str(&format!("- tile_defs: object mapping single chars to {{name, color (hex), walkable (bool), char (display char or empty)}}. Must include a wall char (not walkable) and a floor char (walkable). Add 1-3 thematic tiles. USE THESE EXACT COLORS for your tiles (assign them to whichever tile types make sense): {}. You may darken or lighten them slightly for variety but stay in the same hues.\n",
        palette.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", ")));
    p.push_str(&format!("- boss: {{name, sprite (emoji), hp (~{}), attack (~{}), defense (~{}), xp_value (~{}), description}}\n",
        15 + floor * 8, 3 + floor * 2, floor * 2, 20 + floor * 5));
    p.push_str(&format!("- monster_types: array of 2-3 templates {{name, sprite, hp (~{}), attack (~{}), defense (~{}), xp_value (~{}), description}}\n",
        5 + floor * 3, 2 + floor, floor, 5 + floor * 3));
    p.push_str("- weapon: {name, sprite (emoji), description}\n");
    p.push_str("- armor: {name, sprite (emoji), description}\n");
    p.push_str("- traps: array of {damage (5-12), name}. Number based on your budget. These are hidden floor tiles that hurt the player.\n");
    p.push_str("- budget_spent: {boss, monsters, traps, weapon, armor, potions, gold, total, remaining}\n");
    p.push_str("- mode: {root, scale} — a musical mode for the level's ambient sound. root is a note name (e.g. \"C\", \"F#\", \"Bb\"), scale is one of: \"ionian\", \"dorian\", \"phrygian\", \"lydian\", \"mixolydian\", \"aeolian\", \"locrian\", \"pentatonic_major\", \"pentatonic_minor\", \"blues\", \"whole_tone\", \"chromatic\". Choose a mode that fits the level's mood.\n\n");
    p.push_str("Return ONLY valid JSON.");
    p
}

fn build_phase3_prompt(floor: i32, theme: &str, p1: &Phase1Result, p2: &Phase2Result, tile_chars: &[String]) -> String {
    let mut p = String::new();
    p.push_str(&format!("Generate the MAP for level {} of a roguelike game.\n\n", floor));
    p.push_str(&format!("Theme: {} — \"{}\"\n", theme, p1.title));
    p.push_str(&format!("{}\n", p1.description));
    p.push_str(&format!("Boss: '{}' — {}\n", p2.boss.name, p2.boss.description.as_deref().unwrap_or("")));
    p.push_str(&format!("Monsters: {}\n", p2.monster_types.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ")));
    p.push_str(&format!("Weapon: {} · Armor: {}\n\n", p2.weapon.name, p2.armor.name));

    p.push_str(&format!("Available tile chars: {}\n", tile_chars.join(", ")));
    p.push_str("Tile info:\n");
    for (ch, td) in &p1.tile_defs {
        p.push_str(&format!("  {} = {} ({})\n", ch, td.name, if td.walkable { "walkable" } else { "wall" }));
    }

    // Include 3 random example templates
    let examples = crate::maps::pick_three();
    p.push_str("\nHere are 3 examples of CONNECTED maps (# = wall, . = floor). Study the patterns — rooms connected by corridors. Your map should use YOUR tile chars and have its OWN layout inspired by these:\n\n");
    for (i, &idx) in examples.iter().enumerate() {
        p.push_str(&format!("Example {}:\n{}\n\n", i + 1, crate::maps::TEMPLATES[idx]));
    }
    p.push_str("Key pattern: rooms are open floor areas, walls form boundaries, narrow corridors (1-2 tiles) connect EVERY room. No isolated rooms.\n\n");

    p.push_str("MAP RULES:\n");
    p.push_str("- 40 columns x 24 rows. Each row EXACTLY 40 chars.\n");
    p.push_str("- Create your OWN layout using YOUR tile chars. Don't copy the examples exactly.\n");
    p.push_str("- CRITICAL: fully connected. Player MUST reach boss via walkable tiles. REJECTED if not.\n");
    p.push_str("- player_start and boss_position must be in DIFFERENT rooms, both on walkable tiles.\n");
    p.push_str("- Do NOT put the boss in a corner of the map. Place it in an interior room that has a corridor connecting to the rest of the map.\n");
    p.push_str("- VERIFY: before finalizing, mentally trace a path of walkable tiles from player_start to boss_position. If you can't, the map is INVALID.\n\n");

    p.push_str("Return a JSON object with:\n");
    p.push_str("- grid: array of 24 strings, each exactly 40 chars\n");
    p.push_str("- player_start: [x, y]\n");
    p.push_str("- boss_position: [x, y]\n\n");
    p.push_str("Return ONLY valid JSON.");
    p
}

fn build_overworld_prompt() -> String {
    let mut p = String::new();
    p.push_str("Design a CAMPAIGN OVERWORLD for a roguelike game (like a Super Mario World map).\n\n");
    p.push_str("Be wildly creative with the setting. Invent something original and unexpected — the weirder the better.\n");
    p.push_str("Think more like: a sentient library that reshelves itself, a civilization built inside frozen music, a war between rival paint colors, a detective agency run by ghosts, an opera house where the architecture argues with the performers, a postal service that delivers to parallel dimensions, a courtroom where gravity is on trial.\n\n");
    p.push_str("Return a JSON object with:\n");
    p.push_str("- name: campaign name (2-4 words, evocative)\n");
    p.push_str("- font: a Google Fonts font family for the overworld title\n");
    p.push_str("- description: one atmospheric sentence about the campaign\n");
    p.push_str("- levels: array of 5-8 level nodes, each with:\n");
    p.push_str("  - name: level title (2-4 words)\n");
    p.push_str("  - font: a Google Fonts font family for the level\n");
    p.push_str("  - description: one atmospheric sentence\n");
    p.push_str("  - theme: detailed theme string for the level (e.g. 'collapsing origami palace', 'library where books rewrite themselves', 'volcanic glassblowing workshop')\n");
    p.push_str("  - color: a hex color (e.g. '#e94560') representing the level's primary color/mood. Each level should have a distinct color.\n");
    p.push_str("  - palette: array of 4-6 hex colors for the level's tile types (wall, floor, and 1-3 thematic tiles like lava, water, grass, etc). Be creative and bold with colors. These must be thematically cohesive and visually distinct per level. IMPORTANT: Do NOT use these colors, they are reserved for game entities: green (#66bb6a), red/crimson (#e64545), gold/yellow (#ffd700), cyan/teal (#4dd0e1), orange (#ffa726). Avoid any color close to these.\n");
    p.push_str("  - budget: scapebux budget for the level (integer)\n");
    p.push_str("  - x: horizontal position 0.0-1.0 (left to right)\n");
    p.push_str("  - y: vertical position 0.0-1.0 (top to bottom)\n");
    p.push_str("- connections: array of [i, j] pairs (0-indexed) defining paths between levels\n");
    p.push_str("- final_level: index (0-based) of the FINAL BOSS level. Beating this level wins the game.\n\n");
    p.push_str("RULES:\n");
    p.push_str("- Total budget across ALL levels must be approximately 600 scapebux\n");
    p.push_str("- Early levels should have lower budgets (~60-80), the final level should be the hardest (~120-150)\n");
    p.push_str("- Create BRANCHING paths — not just a linear chain. The player should have choices.\n");
    p.push_str("- Level 0 is the starting level. The final level should be at the END of the path, requiring multiple levels to reach.\n");
    p.push_str("- Make sure all levels are reachable from level 0 via connections.\n");
    p.push_str("- Positions should create a visually interesting map layout (spread them out, use branching paths)\n");
    p.push_str("- Each level theme should be distinct but all should feel part of the same campaign\n\n");
    p.push_str("Return ONLY valid JSON.");
    p
}

// ── Assembly ──

fn assemble_level(
    floor: i32, budget: i32,
    p1: &Phase1Result, p2: &Phase2Result, p3: &Phase3Result,
) -> Result<(Level, [i32; 2], i32), String> {
    let width = 40_i32;
    let height = 24_i32;

    // Build tile lookups
    let mut char_to_name: HashMap<char, String> = HashMap::new();
    let mut tile_defs: HashMap<String, TileDef> = HashMap::new();

    for (ch, raw) in &p1.tile_defs {
        let c = ch.chars().next().unwrap_or('#');
        char_to_name.insert(c, raw.name.clone());
        tile_defs.insert(raw.name.clone(), TileDef {
            name: raw.name.clone(),
            color: raw.color.clone(),
            walkable: raw.walkable,
            char_display: raw.char.clone().unwrap_or_default(),
        });
    }
    if !tile_defs.contains_key("wall") {
        tile_defs.insert("wall".into(), TileDef {
            name: "wall".into(), color: "#444".into(), walkable: false, char_display: String::new(),
        });
    }

    // Parse grid
    let mut tiles: Vec<Vec<String>> = Vec::new();
    for (y, row_str) in p3.grid.iter().enumerate() {
        if y >= height as usize { break; }
        let mut row = Vec::new();
        for (x, ch) in row_str.chars().enumerate() {
            if x >= width as usize { break; }
            row.push(char_to_name.get(&ch).cloned().unwrap_or_else(|| "wall".into()));
        }
        while row.len() < width as usize { row.push("wall".into()); }
        tiles.push(row);
    }
    while tiles.len() < height as usize {
        tiles.push(vec!["wall".into(); width as usize]);
    }

    // Validate player start
    let ps = p3.player_start;
    let ps_x = ps[0].clamp(0, width - 1) as usize;
    let ps_y = ps[1].clamp(0, height - 1) as usize;
    if !tile_defs.get(&tiles[ps_y][ps_x]).map_or(false, |t| t.walkable) {
        return Err(format!("Player start ({},{}) is on a non-walkable tile", ps[0], ps[1]));
    }
    let player_start = ps;

    // Flood fill
    let reachable = flood_fill(&tiles, &tile_defs, player_start[0], player_start[1], width, height);
    let reachable_vec: Vec<(i32, i32)> = reachable.iter().copied().collect();

    // Validate boss position
    let bp = p3.boss_position;
    let bx = bp[0].clamp(0, width - 1);
    let by = bp[1].clamp(0, height - 1);
    if !reachable.contains(&(bx, by)) {
        return Err(format!(
            "Boss at ({},{}) unreachable from player at ({},{}). Reachable: {} tiles.",
            bx, by, player_start[0], player_start[1], reachable.len()
        ));
    }

    eprintln!("Boss '{}' at ({},{}) — reachable! {} tiles from player",
        p2.boss.name, bx, by, (bx - player_start[0]).abs() + (by - player_start[1]).abs());

    // Build boss monster
    let mut monsters = vec![Monster {
        id: format!("boss_{}", floor),
        name: p2.boss.name.clone(),
        sprite: p2.boss.sprite.clone().unwrap_or_else(|| "👹".into()),
        x: bx, y: by,
        hp: p2.boss.hp, max_hp: p2.boss.hp,
        attack: p2.boss.attack, defense: p2.boss.defense.unwrap_or(0),
        xp_value: p2.boss.xp_value.unwrap_or(20),
        description: p2.boss.description.clone().unwrap_or_default(),
        is_boss: true,
    }];

    // Spawn regular monsters from templates
    if !p2.monster_types.is_empty() {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let count = rng.gen_range(3..=7_usize);
        for i in 0..count {
            let tmpl = &p2.monster_types[i % p2.monster_types.len()];
            if let Some(&(mx, my)) = pick_random_reachable(&reachable_vec, player_start, 5, &monsters, &mut rng) {
                monsters.push(Monster {
                    id: format!("m_{}_{}", floor, i),
                    name: tmpl.name.clone(),
                    sprite: tmpl.sprite.clone().unwrap_or_else(|| "👾".into()),
                    x: mx, y: my,
                    hp: tmpl.hp, max_hp: tmpl.hp,
                    attack: tmpl.attack, defense: tmpl.defense.unwrap_or(0),
                    xp_value: tmpl.xp_value.unwrap_or(5),
                    description: tmpl.description.clone().unwrap_or_default(),
                    is_boss: false,
                });
            }
        }
    }

    // Spawn items
    let mut items: Vec<Item> = Vec::new();
    {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        if let Some(&(wx, wy)) = pick_random_reachable(&reachable_vec, player_start, 3, &monsters, &mut rng) {
            items.push(Item {
                id: format!("w_{}", floor), name: p2.weapon.name.clone(),
                sprite: p2.weapon.sprite.clone().unwrap_or_else(|| "⚔️".into()),
                x: wx, y: wy, item_type: "weapon".into(), value: floor + 1,
                description: p2.weapon.description.clone().unwrap_or_default(),
            });
        }
        if let Some(&(ax, ay)) = pick_random_reachable(&reachable_vec, player_start, 3, &monsters, &mut rng) {
            items.push(Item {
                id: format!("a_{}", floor), name: p2.armor.name.clone(),
                sprite: p2.armor.sprite.clone().unwrap_or_else(|| "🛡️".into()),
                x: ax, y: ay, item_type: "armor".into(), value: floor,
                description: p2.armor.description.clone().unwrap_or_default(),
            });
        }
        let potion_count = rng.gen_range(1..=3_usize);
        for i in 0..potion_count {
            if let Some(&(px, py)) = pick_random_reachable(&reachable_vec, player_start, 2, &monsters, &mut rng) {
                items.push(Item {
                    id: format!("pot_{}_{}", floor, i), name: "Health Potion".into(),
                    sprite: "🧪".into(), x: px, y: py,
                    item_type: "potion".into(), value: 0, description: String::new(),
                });
            }
        }
        let gold_count = rng.gen_range(1..=4_usize);
        for i in 0..gold_count {
            let amount = rng.gen_range(1..=(5 + floor * 3));
            if let Some(&(gx, gy)) = pick_random_reachable(&reachable_vec, player_start, 2, &monsters, &mut rng) {
                items.push(Item {
                    id: format!("gold_{}_{}", floor, i), name: format!("{} Gold", amount),
                    sprite: "💰".into(), x: gx, y: gy,
                    item_type: "gold".into(), value: amount, description: String::new(),
                });
            }
        }
    }

    // Parse traps (place on random reachable tiles since phase 2 doesn't know the map)
    let mut traps: Vec<Trap> = Vec::new();
    if let Some(trap_defs) = &p2.traps {
        let mut rng = rand::thread_rng();
        for td in trap_defs.iter() {
            if let Some(&(tx, ty)) = pick_random_reachable(&reachable_vec, player_start, 4, &monsters, &mut rng) {
                traps.push(Trap {
                    x: tx, y: ty,
                    damage: td.damage.unwrap_or(8),
                    name: td.name.clone().unwrap_or_else(|| "Trap".into()),
                    triggered: false,
                });
            }
        }
    }

    // Budget accounting
    let monster_count = monsters.iter().filter(|m| !m.is_boss).count() as i32;
    let trap_count = traps.len() as i32;
    let potion_count = items.iter().filter(|i| i.item_type == "potion").count() as i32;
    let gold_count = items.iter().filter(|i| i.item_type == "gold").count() as i32;

    let spent = 25 + monster_count * 10 + trap_count * 6;
    let earned = 15 + 10 + potion_count * 5 + gold_count * 3;
    let remaining = budget - spent + earned;

    eprintln!("Budget: {} available, spent {} (boss, {}mon, {}traps), earned {} (w,a,{}pot,{}gold) = {} remaining",
        budget, spent, monster_count, trap_count, earned, potion_count, gold_count, remaining);

    if let Some(bs) = &p2.budget_spent {
        eprintln!("LLM's accounting: {}", bs);
    }

    let scale = p2.mode.as_ref()
        .map(|m| build_scale(&m.root, &m.scale))
        .unwrap_or_else(|| build_scale("C", "pentatonic_minor"));

    if let Some(m) = &p2.mode {
        eprintln!("Mode: {} {}", m.root, m.scale);
    }

    let level = Level {
        width, height, tiles, tile_defs, monsters, items, traps,
        title: p1.title.clone(),
        description: p1.description.clone(),
        font: p1.font.clone().expect("font was set from overworld config"),
        scale,
        revealed: HashSet::new(),
        visible: HashSet::new(),
    };

    Ok((level, player_start, remaining))
}

// ── Helpers ──

fn flood_fill(
    tiles: &[Vec<String>], tile_defs: &HashMap<String, TileDef>,
    start_x: i32, start_y: i32, width: i32, height: i32,
) -> HashSet<(i32, i32)> {
    let mut visited = HashSet::new();
    let mut stack = vec![(start_x, start_y)];
    while let Some((x, y)) = stack.pop() {
        if x < 0 || y < 0 || x >= width || y >= height { continue; }
        if !visited.insert((x, y)) { continue; }
        let tile = &tiles[y as usize][x as usize];
        if !tile_defs.get(tile).map_or(false, |t| t.walkable) {
            visited.remove(&(x, y));
            continue;
        }
        stack.push((x + 1, y));
        stack.push((x - 1, y));
        stack.push((x, y + 1));
        stack.push((x, y - 1));
    }
    visited
}

fn pick_random_reachable<'a>(
    reachable: &'a [(i32, i32)], player_start: [i32; 2], min_dist: i32,
    monsters: &[Monster], rng: &mut impl rand::Rng,
) -> Option<&'a (i32, i32)> {
    let candidates: Vec<&(i32, i32)> = reachable.iter()
        .filter(|(x, y)| {
            let dist = (*x - player_start[0]).abs() + (*y - player_start[1]).abs();
            dist >= min_dist && !monsters.iter().any(|m| m.x == *x && m.y == *y)
        })
        .collect();
    if candidates.is_empty() {
        reachable.iter()
            .filter(|(x, y)| !monsters.iter().any(|m| m.x == *x && m.y == *y))
            .nth(rng.gen_range(0..reachable.len().max(1)))
    } else {
        Some(candidates[rng.gen_range(0..candidates.len())])
    }
}

/// Build a scale of frequencies from a root note name and scale type.
/// Returns frequencies spanning 2 octaves in a comfortable range (C4-C6 area).
fn build_scale(root: &str, scale_name: &str) -> Vec<f32> {
    // Parse root note to semitone offset from C
    let root_semitone = match root.to_uppercase().trim_end_matches(|c: char| c.is_ascii_digit()).to_string().as_str() {
        "C" => 0, "C#" | "DB" => 1, "D" => 2, "D#" | "EB" => 3,
        "E" => 4, "F" => 5, "F#" | "GB" => 6, "G" => 7,
        "G#" | "AB" => 8, "A" => 9, "A#" | "BB" => 10, "B" => 11,
        _ => 0, // default to C
    };

    // Scale intervals (semitones from root)
    let intervals: Vec<i32> = match scale_name.to_lowercase().as_str() {
        "ionian" | "major" => vec![0, 2, 4, 5, 7, 9, 11],
        "dorian" => vec![0, 2, 3, 5, 7, 9, 10],
        "phrygian" => vec![0, 1, 3, 5, 7, 8, 10],
        "lydian" => vec![0, 2, 4, 6, 7, 9, 11],
        "mixolydian" => vec![0, 2, 4, 5, 7, 9, 10],
        "aeolian" | "minor" => vec![0, 2, 3, 5, 7, 8, 10],
        "locrian" => vec![0, 1, 3, 5, 6, 8, 10],
        "pentatonic_major" => vec![0, 2, 4, 7, 9],
        "pentatonic_minor" => vec![0, 3, 5, 7, 10],
        "blues" => vec![0, 3, 5, 6, 7, 10],
        "whole_tone" => vec![0, 2, 4, 6, 8, 10],
        "chromatic" => vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        _ => vec![0, 3, 5, 7, 10], // default pentatonic minor
    };

    // Generate frequencies across 2 octaves starting from octave 4
    // C4 = MIDI 60 = 261.63 Hz, A4 = MIDI 69 = 440 Hz
    let base_midi = 60 + root_semitone; // root in octave 4
    let mut freqs = Vec::new();
    for octave_offset in 0..2 {
        for &interval in &intervals {
            let midi = base_midi + octave_offset * 12 + interval;
            let freq = 440.0 * 2.0_f32.powf((midi as f32 - 69.0) / 12.0);
            freqs.push(freq);
        }
    }
    freqs
}
