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

#[derive(Deserialize, Clone)]
pub struct TileDefSlim {
    pub name: String,
    #[serde(default)]
    pub char: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct Phase2Result {
    pub tile_defs: Vec<TileDefSlim>,
    pub boss: MonsterRaw,
    pub monster_types: Vec<MonsterTemplateRaw>,
    pub weapon: ItemTemplateRaw,
    pub armor: ItemTemplateRaw,
    pub traps: Option<Vec<TrapRaw>>,
    #[allow(dead_code)]
    pub budget_spent: Option<serde_json::Value>,
    pub mode: Option<ModeRaw>,
    pub victory_message: Option<String>,
    pub defeat_message: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct ModeRaw {
    pub root: String,
    pub scale: String,
}

#[derive(Deserialize, Clone)]
#[allow(dead_code)]
pub struct MonsterRaw {
    pub name: String,
    #[serde(default)]
    pub hp: i32,
    #[serde(default)]
    pub attack: i32,
    pub defense: Option<i32>,
    pub xp_value: Option<i32>,
    pub description: Option<String>,
}

#[derive(Deserialize, Clone)]
#[allow(dead_code)]
pub struct MonsterTemplateRaw {
    pub name: String,
    #[serde(default)]
    pub hp: i32,
    #[serde(default)]
    pub attack: i32,
    pub defense: Option<i32>,
    pub xp_value: Option<i32>,
    pub description: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct ItemTemplateRaw {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Deserialize, Clone)]
#[allow(dead_code)]
pub struct TrapRaw {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub damage: Option<i32>,
    pub name: Option<String>,
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
}

#[derive(Deserialize)]
pub struct OverworldResult {
    pub name: String,
    pub font: Option<String>,
    pub description_font: Option<String>,
    pub label_font: Option<String>,
    pub description: String,
    pub bg_color: Option<String>,
    pub text_color: Option<String>,
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

// ── Tile def expansion ──

/// Build full tile_defs from slim LLM output + palette colors.
/// First entry = wall (not walkable), rest = walkable. Colors assigned from palette.
fn expand_tile_defs(slim: &[TileDefSlim], palette: &[String]) -> HashMap<String, TileDefRaw> {
    let chars = ["#", ".", "~", "*", "+", "^"];
    let mut defs = HashMap::new();
    for (i, td) in slim.iter().enumerate() {
        let ch = chars.get(i).unwrap_or(&"?");
        let color = palette.get(i).cloned().unwrap_or_else(|| "#888888".into());
        defs.insert(ch.to_string(), TileDefRaw {
            name: td.name.clone(),
            color,
            walkable: i > 0, // first entry is wall
            char: td.char.clone(),
        });
    }
    // Ensure at least a wall and floor exist
    if defs.is_empty() {
        defs.insert("#".into(), TileDefRaw { name: "wall".into(), color: palette.first().cloned().unwrap_or("#444".into()), walkable: false, char: None });
        defs.insert(".".into(), TileDefRaw { name: "floor".into(), color: palette.get(1).cloned().unwrap_or("#888".into()), walkable: true, char: None });
    }
    defs
}

// ── Overworld layout ──

/// Compute node positions from the DAG structure using layered layout.
/// BFS from node 0 assigns depth (x-layer), nodes at same depth spread vertically.
fn layout_overworld(ow: &mut crate::game::Overworld) {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Prepend a "Start" node connected to node 0 (which becomes node 1)
    let start_node = crate::game::OverworldNode {
        name: "Start".into(),
        font: ow.font.clone(),
        description: String::new(),
        theme: String::new(),
        palette: vec!["#888888".into()],
        budget: 0,
        x: 0.0, y: 0.5,
        completed: true,
        unlocked: true,
        is_final: false,
    };
    ow.nodes.insert(0, start_node);
    // Shift all existing connection indices by 1 and add start→old node 0
    ow.connections = ow.connections.iter().map(|&(a, b)| (a + 1, b + 1)).collect();
    ow.connections.push((0, 1));
    // Unlock old node 0 (now index 1)
    ow.nodes[1].unlocked = true;

    let n = ow.nodes.len();

    // BFS from node 0 to assign depth
    let mut depth = vec![-1i32; n];
    depth[0] = 0;
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(0);
    while let Some(node) = queue.pop_front() {
        for &(a, b) in &ow.connections {
            let neighbor = if a == node { b } else if b == node { a } else { continue };
            if neighbor < n && depth[neighbor] == -1 {
                depth[neighbor] = depth[node] + 1;
                queue.push_back(neighbor);
            }
        }
    }

    for d in &mut depth {
        if *d == -1 { *d = 0; }
    }

    let max_depth = *depth.iter().max().unwrap_or(&0);
    if max_depth == 0 {
        for (i, node) in ow.nodes.iter_mut().enumerate() {
            node.x = i as f32 / (n.max(2) - 1) as f32;
            node.y = 0.5;
        }
        return;
    }

    // Group nodes by depth layer
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); (max_depth + 1) as usize];
    for (i, &d) in depth.iter().enumerate() {
        layers[d as usize].push(i);
    }

    // Assign positions: x from depth with jitter, y spread within layer with jitter
    for (layer_idx, layer) in layers.iter().enumerate() {
        let base_x = layer_idx as f32 / max_depth as f32;
        let count = layer.len();
        for (slot, &node_idx) in layer.iter().enumerate() {
            let base_y = if count == 1 {
                0.5
            } else {
                0.15 + (slot as f32 / (count - 1) as f32) * 0.7
            };
            // Add organic jitter (but not to start node)
            let jitter_x = if node_idx == 0 { 0.0 } else { rng.gen_range(-0.04..=0.04) };
            let jitter_y = if node_idx == 0 { 0.0 } else { rng.gen_range(-0.06..=0.06) };
            ow.nodes[node_idx].x = (base_x + jitter_x).clamp(0.0, 1.0);
            ow.nodes[node_idx].y = (base_y + jitter_y).clamp(0.05, 0.95);
        }
    }
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
    // Build nodes (positions computed below by layout algorithm)
    let nodes: Vec<crate::game::OverworldNode> = result.levels.into_iter().enumerate().map(|(i, n)| {
        crate::game::OverworldNode {
            name: n.name,
            font: n.font.unwrap_or_else(|| ow_font.clone()),
            description: n.description,
            theme: n.theme,
            palette: n.palette.or_else(|| n.color.map(|c| vec![c])).unwrap_or_else(|| vec!["#888888".into()]),
            budget: n.budget,
            x: 0.0,
            y: 0.0,
            completed: false,
            unlocked: i == 0,
            is_final: i == final_level,
        }
    }).collect();

    let desc_font = result.description_font.unwrap_or_else(|| ow_font.clone());
    let label_font = result.label_font.unwrap_or_else(|| ow_font.clone());
    let bg_color = result.bg_color.unwrap_or_else(|| "#0a0a0a".into());
    let text_color = result.text_color.unwrap_or_else(|| "#e0d5c0".into());

    let mut overworld = crate::game::Overworld {
        name: result.name,
        font: ow_font,
        description_font: desc_font,
        label_font,
        description: result.description,
        bg_color,
        text_color,
        connections: result.connections,
        current_node: 0,
        nodes,
    };

    layout_overworld(&mut overworld);

    on_phase(PhaseUpdate {
        phase: "overworld designed".into(),
        detail: format!("{} — {} levels", overworld.name, overworld.nodes.len()),
    });

    Ok(overworld)
}

/// Build a level from a pre-generated design (no LLM call — just mapgen + assemble)
pub fn build_level_from_design(
    config: &LevelConfig,
    design: &Phase2Result,
) -> Result<(Level, [i32; 2], i32), String> {
    let floor = config.floor;
    let budget = config.budget;

    let full_defs = expand_tile_defs(&design.tile_defs, &config.palette);
    let p1 = Phase1Result {
        title: config.title.clone(),
        description: config.description.clone(),
        font: Some(config.font.clone()),
        tile_defs: full_defs.clone(),
    };

    let map = crate::mapgen::generate_map(&full_defs);
    assemble_level(floor, budget, &p1, design, &map)
}

// ── Three-phase generation (legacy, still used if no pre-generated design) ──

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

    // Build full tile_defs from slim LLM output + palette
    let full_defs = expand_tile_defs(&p2.tile_defs, &config.palette);
    let p1 = Phase1Result {
        title: config.title.clone(),
        description: config.description.clone(),
        font: Some(config.font.clone()),
        tile_defs: full_defs.clone(),
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

    // ── Build world (procedural generation) ──
    on_phase(PhaseUpdate { phase: "building world".into(), detail: String::new() });

    let map = crate::mapgen::generate_map(&full_defs);
    let (level, start, remaining) = assemble_level(floor, budget, &p1, &p2, &map)?;

    on_phase(PhaseUpdate { phase: "world built".into(), detail: String::new() });
    Ok((level, start, remaining))
}

// ── Prompt builders ──

fn build_phase2_prompt(floor: i32, player: &Player, budget: i32, theme: &str, title: &str, description: &str, _palette: &[String]) -> String {
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
    p.push_str("- tile_defs: array of {name, char (display char or empty)}. First entry is the wall tile, rest are walkable. Include 3-5 tiles total (wall, floor, and 1-3 thematic). Colors are assigned by the engine from the palette.\n");
    p.push_str("- boss: {name, description}. Stats are computed by the engine from budget.\n");
    p.push_str("- monster_types: array of 2-3 templates {name, description}. Stats are computed by the engine.\n");
    p.push_str("- weapon: {name, description}\n");
    p.push_str("- armor: {name, description}\n");
    p.push_str("- traps: array of {name}. The engine decides count and damage from budget.\n");
    p.push_str("- mode: {root, scale} — a musical mode for the level's ambient sound. root is a note name (e.g. \"C\", \"F#\", \"Bb\"), scale is one of: \"ionian\", \"dorian\", \"phrygian\", \"lydian\", \"mixolydian\", \"aeolian\", \"locrian\", \"pentatonic_major\", \"pentatonic_minor\", \"blues\", \"whole_tone\", \"chromatic\". Choose a mode that fits the level's mood.\n");
    p.push_str("- victory_message: one short atmospheric sentence shown when the player beats this level\n- defeat_message: one short atmospheric sentence shown when the player dies in this level\n\n");
    p.push_str("Return ONLY valid JSON.");
    p
}

pub fn build_single_level_design_prompt(
    campaign_name: &str, campaign_desc: &str, config: &LevelConfig,
) -> String {
    let mut p = String::new();
    p.push_str(&format!("Generate TILE DEFINITIONS and OBJECTS for one level of a roguelike campaign.\n\n"));
    p.push_str(&format!("Campaign: \"{}\"\n{}\n\n", campaign_name, campaign_desc));
    p.push_str(&format!("Level: \"{}\" (theme: {}, budget: {})\n", config.title, config.theme, config.budget));
    p.push_str(&format!("{}\nPalette: {}\n\n", config.description, config.palette.join(", ")));
    p.push_str("You are ADVERSARIAL — your goal is to kill the player.\n\n");
    p.push_str("Return a JSON object with:\n");
    p.push_str("- tile_defs: array of {name, char (display char or empty)}. First entry is the wall tile, rest are walkable. Include 3-5 tiles total (wall, floor, and 1-3 thematic). Colors are assigned by the engine from the palette.\n");
    p.push_str("- boss: {name, description}. Stats are computed by the engine from budget.\n");
    p.push_str("- monster_types: array of 2-3 templates {name, description}. Stats are computed by the engine.\n");
    p.push_str("- weapon: {name, description}\n");
    p.push_str("- armor: {name, description}\n");
    p.push_str("- traps: array of {name}. The engine decides count and damage from budget.\n");
    p.push_str("- mode: {root, scale} — musical mode. root = note name, scale = one of: ionian, dorian, phrygian, lydian, mixolydian, aeolian, locrian\n");
    p.push_str("- victory_message: one short atmospheric sentence shown when the player beats this level\n- defeat_message: one short atmospheric sentence shown when the player dies in this level\n\n");
    p.push_str("Return ONLY valid JSON.");
    p
}

pub fn call_llm_for_design<F: Fn()>(
    client: &reqwest::blocking::Client, api_key: &str, model: &str, prompt: &str,
    on_token: Option<F>,
) -> Result<Phase2Result, String> {
    let content = call_llm_streaming(client, api_key, model, prompt, on_token)?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Design parse error: {}\n\nRaw: {}", e, &content[..content.len().min(500)]))
}

fn build_overworld_prompt() -> String {
    let mut p = String::new();
    p.push_str("Design a CAMPAIGN OVERWORLD for a roguelike game (like a Super Mario World map).\n\n");
    p.push_str("Be wildly creative with the setting. Invent something original and unexpected — the weirder the better.\n");
    p.push_str("Think more like: a sentient library that reshelves itself, a civilization built inside frozen music, a war between rival paint colors, a detective agency run by ghosts, an opera house where the architecture argues with the performers, a postal service that delivers to parallel dimensions, a courtroom where gravity is on trial.\n\n");
    p.push_str("Return a JSON object with:\n");
    p.push_str("- name: campaign name (2-4 words, evocative)\n");
    p.push_str("- font: a Google Fonts font family for the overworld title\n");
    p.push_str("- description_font: a Google Fonts font family for the description text (readable, elegant)\n");
    p.push_str("- label_font: a Google Fonts font family for level name labels on the map\n");
    p.push_str("- description: one atmospheric sentence about the campaign\n");
    p.push_str("- bg_color: hex background color for the overworld screen (e.g. '#0a0a0a', '#1a0a2e')\n");
    p.push_str("- text_color: hex color for title and label text — must have strong contrast against bg_color\n");
    p.push_str("- levels: array of 5-8 level nodes, each with:\n");
    p.push_str("  - name: level title (2-4 words)\n");
    p.push_str("  - font: a Google Fonts font family for the level\n");
    p.push_str("  - description: one atmospheric sentence\n");
    p.push_str("  - theme: detailed theme string for the level (e.g. 'collapsing origami palace', 'library where books rewrite themselves', 'volcanic glassblowing workshop')\n");
    p.push_str("  - color: a hex color (e.g. '#e94560') representing the level's primary color/mood. Each level should have a distinct color.\n");
    p.push_str("  - palette: array of 4-6 hex colors for the level's tile types (wall, floor, and 1-3 thematic tiles like lava, water, grass, etc). Be creative and bold with colors. These must be thematically cohesive and visually distinct per level. IMPORTANT: Do NOT use these colors, they are reserved for game entities: green (#66bb6a), red/crimson (#e64545), gold/yellow (#ffd700), cyan/teal (#4dd0e1), orange (#ffa726). Avoid any color close to these.\n");
    p.push_str("  - budget: scapebux budget for the level (integer)\n");
    p.push_str("- connections: array of [i, j] pairs (0-indexed) defining paths between levels. This is a DAG — create BRANCHING paths, not a linear chain.\n");
    p.push_str("- final_level: index (0-based) of the FINAL BOSS level. Beating this level wins the game.\n\n");
    p.push_str("RULES:\n");
    p.push_str("- Total budget across ALL levels must be approximately 1200 scapebux\n");
    p.push_str("- Early levels should have lower budgets (~120-160), the final level should be the hardest (~250-300)\n");
    p.push_str("- Level 0 is the starting level. The final level should be at the END of the path, requiring multiple levels to reach.\n");
    p.push_str("- Make sure all levels are reachable from level 0 via connections.\n");
    p.push_str("- Each level theme should be distinct but all should feel part of the same campaign\n\n");
    p.push_str("Return ONLY valid JSON.");
    p
}

// ── Assembly ──

fn assemble_level(
    floor: i32, budget: i32,
    p1: &Phase1Result, p2: &Phase2Result, map: &crate::mapgen::MapGenResult,
) -> Result<(Level, [i32; 2], i32), String> {
    let width = 60_i32;
    let height = 36_i32;

    // Build tile defs lookup
    let mut tile_defs: HashMap<String, TileDef> = HashMap::new();
    for (_ch, raw) in &p1.tile_defs {
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

    // Add locked door tile def if the map has one
    if map.key_position.is_some() {
        tile_defs.insert("locked_door".into(), TileDef {
            name: "locked_door".into(), color: "#aa6622".into(), walkable: false,
            char_display: "🔒".into(),
        });
    }

    // Use the procedurally generated grid directly
    let tiles = map.tiles.clone();
    let player_start = map.player_start;

    // Two flood fills: player side (blocked by locked door) and full map (ignoring lock)
    let player_side = flood_fill(&tiles, &tile_defs, player_start[0], player_start[1], width, height);
    let player_side_vec: Vec<(i32, i32)> = player_side.iter().copied().collect();

    // Full reachable: temporarily treat locked_door as walkable
    let mut full_defs = tile_defs.clone();
    if map.key_position.is_some() {
        full_defs.insert("locked_door".into(), TileDef {
            name: "locked_door".into(), color: "#aa6622".into(), walkable: true,
            char_display: "🔒".into(),
        });
    }
    let full_reachable = flood_fill(&tiles, &full_defs, player_start[0], player_start[1], width, height);

    // Locked side = full reachable minus player side
    let locked_side_vec: Vec<(i32, i32)> = full_reachable.iter()
        .filter(|t| !player_side.contains(t))
        .copied()
        .collect();
    let has_locked_side = !locked_side_vec.is_empty();

    // Combined reachable for general use
    let reachable_vec: Vec<(i32, i32)> = full_reachable.iter().copied().collect();

    // Boss position
    let bx = map.boss_position[0];
    let by = map.boss_position[1];

    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut remaining_budget = budget;

    // Scaling factors
    let hp_per_point = 1.5 + floor as f32 * 0.5;
    let atk_scale = 2 + floor;
    let def_scale = floor;

    // ── Boss: 15-25% of budget ──
    let boss_cost = rng.gen_range(budget * 15 / 100..=budget * 25 / 100).max(10);
    remaining_budget -= boss_cost;
    let boss_hp = (boss_cost as f32 * hp_per_point).round() as i32;
    let boss_atk = atk_scale + boss_cost / 12;
    let boss_def = (def_scale / 2).max(0);
    let boss_xp = boss_cost + floor * 5;

    eprintln!("Boss '{}' — cost {} → {} HP, {} ATK, {} DEF",
        p2.boss.name, boss_cost, boss_hp, boss_atk, boss_def);

    let mut monsters = vec![Monster {
        id: format!("boss_{}", floor),
        name: p2.boss.name.clone(),

        x: bx, y: by,
        hp: boss_hp, max_hp: boss_hp,
        attack: boss_atk, defense: boss_def,
        xp_value: boss_xp,
        description: p2.boss.description.clone().unwrap_or_default(),
        is_boss: true,
    }];

    // Helper: pick a random side for spawning (40% chance locked side if it exists)
    let pick_side = |rng: &mut rand::rngs::ThreadRng| -> &Vec<(i32, i32)> {
        if has_locked_side && rng.gen::<f32>() < 0.4 {
            &locked_side_vec
        } else {
            &player_side_vec
        }
    };

    // ── Monsters: spend until budget runs low ──
    if !p2.monster_types.is_empty() {
        let mut i = 0;
        while remaining_budget >= 5 {
            let mon_cost = rng.gen_range(5..=8.min(remaining_budget));
            remaining_budget -= mon_cost;
            let mon_hp = (mon_cost as f32 * (1.0 + floor as f32 * 0.3)).round() as i32;
            let mon_atk = (atk_scale - 1).max(1) + mon_cost / 10;
            let mon_def = (def_scale / 2).max(0);
            let mon_xp = mon_cost + floor * 2;

            let tmpl = &p2.monster_types[i % p2.monster_types.len()];
            let tiles = pick_side(&mut rng);
            if let Some(&(mx, my)) = pick_random_reachable(tiles, player_start, 3, &monsters, &mut rng) {
                monsters.push(Monster {
                    id: format!("m_{}_{}", floor, i),
                    name: tmpl.name.clone(),
                    x: mx, y: my,
                    hp: mon_hp, max_hp: mon_hp,
                    attack: mon_atk, defense: mon_def,
                    xp_value: mon_xp,
                    description: tmpl.description.clone().unwrap_or_default(),
                    is_boss: false,
                });
            }
            i += 1;
        }
    }

    // ── Traps: 3-8 cost each from remaining budget ──
    let mut traps: Vec<Trap> = Vec::new();
    if let Some(trap_defs) = &p2.traps {
        for td in trap_defs.iter() {
            let trap_cost = rng.gen_range(3..=8.min(remaining_budget.max(3)));
            if remaining_budget < 3 { break; }
            remaining_budget -= trap_cost;
            let tiles = pick_side(&mut rng);
            if let Some(&(tx, ty)) = pick_random_reachable(tiles, player_start, 4, &monsters, &mut rng) {
                traps.push(Trap {
                    x: tx, y: ty,
                    damage: trap_cost + floor,
                    name: td.name.clone().unwrap_or_else(|| "Trap".into()),
                    triggered: false,
                });
            }
        }
    }

    // ── Gold: 2-5 cost each from remaining budget ──
    let mut items: Vec<Item> = Vec::new();
    {
        let mut gold_i = 0;
        while remaining_budget >= 2 {
            let gold_cost = rng.gen_range(2..=5.min(remaining_budget));
            remaining_budget -= gold_cost;
            let amount = gold_cost * 2 + rng.gen_range(0..=floor);
            let tiles = pick_side(&mut rng);
            if let Some(&(gx, gy)) = pick_random_reachable(tiles, player_start, 2, &monsters, &mut rng) {
                items.push(Item {
                    id: format!("gold_{}_{}", floor, gold_i), name: format!("{} Gold", amount),
                    x: gx, y: gy,
                    item_type: "gold".into(), value: amount, description: String::new(),
                });
            }
            gold_i += 1;
            if gold_i >= 6 { break; }
        }

        // ── Free items: weapon, armor, potions, key — randomly distributed ──
        let w_tiles = pick_side(&mut rng);
        if let Some(&(wx, wy)) = pick_random_reachable(w_tiles, player_start, 1, &monsters, &mut rng) {
            items.push(Item {
                id: format!("w_{}", floor), name: p2.weapon.name.clone(),
                x: wx, y: wy, item_type: "weapon".into(), value: floor + 1,
                description: p2.weapon.description.clone().unwrap_or_default(),
            });
        }
        let a_tiles = pick_side(&mut rng);
        if let Some(&(ax, ay)) = pick_random_reachable(a_tiles, player_start, 1, &monsters, &mut rng) {
            items.push(Item {
                id: format!("a_{}", floor), name: p2.armor.name.clone(),
                x: ax, y: ay, item_type: "armor".into(), value: floor,
                description: p2.armor.description.clone().unwrap_or_default(),
            });
        }
        let mon_count = monsters.iter().filter(|m| !m.is_boss).count();
        let potion_count = (mon_count / 4).max(2);
        for i in 0..potion_count {
            if let Some(&(px, py)) = pick_random_reachable(&reachable_vec, player_start, 2, &monsters, &mut rng) {
                items.push(Item {
                    id: format!("pot_{}_{}", floor, i), name: "Health Potion".into(),
                    x: px, y: py,
                    item_type: "potion".into(), value: 0, description: String::new(),
                });
            }
        }
        if let Some(key_pos) = map.key_position {
            items.push(Item {
                id: format!("key_{}", floor), name: "Key".into(),
                x: key_pos[0], y: key_pos[1],
                item_type: "key".into(), value: 0,
                description: "Unlocks a locked door.".into(),
            });
        }
    }

    let monster_count = monsters.iter().filter(|m| !m.is_boss).count();
    let trap_count = traps.len();
    let gold_count = items.iter().filter(|i| i.item_type == "gold").count();
    eprintln!("Budget: {} total, {} remaining — boss + {} monsters + {} traps + {} gold",
        budget, remaining_budget, monster_count, trap_count, gold_count);

    let scale = p2.mode.as_ref()
        .map(|m| build_scale(&m.root, &m.scale))
        .unwrap_or_else(|| build_scale("C", "aeolian"));

    if let Some(m) = &p2.mode {
        eprintln!("Mode: {} {}", m.root, m.scale);
    }

    let level = Level {
        width, height, tiles, tile_defs, monsters, items, traps,
        title: p1.title.clone(),
        description: p1.description.clone(),
        font: p1.font.clone().expect("font was set from overworld config"),
        scale,
        victory_message: p2.victory_message.clone().unwrap_or_default(),
        defeat_message: p2.defeat_message.clone().unwrap_or_default(),
        revealed: HashSet::new(),
        visible: HashSet::new(),
    };

    Ok((level, player_start, remaining_budget))
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
pub fn build_scale(root: &str, scale_name: &str) -> Vec<f32> {
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
        _ => vec![0, 2, 3, 5, 7, 8, 10], // default aeolian (natural minor)
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
