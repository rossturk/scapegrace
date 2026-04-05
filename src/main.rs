mod fonts;
mod game;
mod gen;
mod mapgen;
mod maps;
mod sfx;

use game::*;
use macroquad::prelude::*;
use ::rand::Rng;
use std::sync::mpsc;

const TILE: f32 = 16.0;

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut buf = Vec::with_capacity(input.len() * 3 / 4);
    let mut accum: u32 = 0;
    let mut bits: u32 = 0;
    for &b in input.as_bytes() {
        let val = if b == b'=' { break; }
            else if let Some(pos) = TABLE.iter().position(|&c| c == b) { pos as u32 }
            else { continue };
        accum = (accum << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            buf.push((accum >> bits) as u8);
            accum &= (1 << bits) - 1;
        }
    }
    Some(buf)
}

// Entity colors — used in game rendering AND overworld particles
const COLOR_BOSS: &str = "#e64545";
const COLOR_MONSTER: &str = "#cc5500";
const COLOR_SHIELD: &str = "#4488ff";
const COLOR_WEAPON: &str = "#ff8844";
const COLOR_POTION: &str = "#44ff44";
const COLOR_GOLD: &str = "#ffd700";
const COLOR_ARMOR: &str = "#4488ff";
const NAV_INITIAL_DELAY: f64 = 0.3;
const NAV_REPEAT_RATE: f64 = 0.15;
const GAME_INITIAL_DELAY: f64 = 0.18;
const GAME_REPEAT_RATE: f64 = 0.10;

enum Screen {
    KeyEntry,
    CampaignSelect,
    Start,
    SoundTest,
    GenOverworld,
    GenLevel,
    Playing,
    Dead,
    Victory,
    GameWon,
    Store,
    Teleport,
}

enum GenMsg {
    Phase(String, String),
    Token,
    DesignToken(usize), // design index — flash on the corresponding node
    OverworldReady(game::Overworld, Option<Vec<u8>>),
    LevelDesignReady(usize, gen::Phase2Result),
    LevelDone(Level, [i32; 2], Option<Vec<u8>>),
    Error(String),
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Scapegrace".to_owned(),
        window_width: 1600,
        window_height: 900,
        window_resizable: true,
        high_dpi: true,
        icon: Some(miniquad::conf::Icon {
            small: *include_bytes!("../assets/icon_16.rgba"),
            medium: *include_bytes!("../assets/icon_32.rgba"),
            big: *include_bytes!("../assets/icon_64.rgba"),
        }),
        ..Default::default()
    }
}

fn overworld_loading_phrase() -> String {
    let phrases = [
        "dreaming up a world",
        "conjuring the unknown",
        "charting strange lands",
        "summoning the cartographer",
        "unfolding the map",
        "sketching impossible geography",
        "weaving a new reality",
        "opening forbidden atlases",
        "stitching dimensions together",
        "brewing a fresh cosmos",
        "waking the sleeping world",
        "rearranging the constellations",
        "inventing new horizons",
        "invoking the mapmaker",
        "painting the void",
        "sculpting the firmament",
        "raising continents",
        "naming forgotten places",
        "filling in the blank spaces",
        "drawing borders in the dust",
        "imagining what lies beyond",
        "populating the emptiness",
        "laying the foundations",
        "choosing which stars to keep",
        "assembling the geography",
    ];
    let idx = ::rand::random::<usize>() % phrases.len();
    phrases[idx].into()
}

fn desaturate(c: Color, amount: f32) -> Color {
    let lum = c.r * 0.299 + c.g * 0.587 + c.b * 0.114;
    Color::new(
        c.r + (lum - c.r) * amount,
        c.g + (lum - c.g) * amount,
        c.b + (lum - c.b) * amount,
        c.a,
    )
}

fn hex_to_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    let (r, g, b) = match hex.len() {
        3 => (
            u8::from_str_radix(&hex[0..1].repeat(2), 16).unwrap_or(0),
            u8::from_str_radix(&hex[1..2].repeat(2), 16).unwrap_or(0),
            u8::from_str_radix(&hex[2..3].repeat(2), 16).unwrap_or(0),
        ),
        6 => (
            u8::from_str_radix(&hex[0..2], 16).unwrap_or(0),
            u8::from_str_radix(&hex[2..4], 16).unwrap_or(0),
            u8::from_str_radix(&hex[4..6], 16).unwrap_or(0),
        ),
        _ => (0, 0, 0),
    };
    Color::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
}

// ── Multi-layer soft shadow helper ──

fn draw_soft_circle_shadow(cx: f32, cy: f32, r: f32) {
    let layers: [(f32, f32); 4] = [(1.0, 0.20), (2.0, 0.14), (3.5, 0.08), (5.0, 0.04)];
    for &(off, alpha) in &layers {
        draw_circle(cx + off, cy + off, r + off * 0.5, Color::new(0.0, 0.0, 0.0, alpha));
    }
}

fn draw_soft_rect_shadow(x: f32, y: f32, w: f32, h: f32) {
    let layers: [(f32, f32); 4] = [(1.0, 0.20), (2.0, 0.14), (3.5, 0.08), (5.0, 0.04)];
    for &(off, alpha) in &layers {
        draw_rectangle(x + off, y + off, w + off, h + off, Color::new(0.0, 0.0, 0.0, alpha));
    }
}

fn draw_soft_poly_shadow(cx: f32, cy: f32, sides: u8, r: f32, rot: f32) {
    let layers: [(f32, f32); 4] = [(1.0, 0.20), (2.0, 0.14), (3.5, 0.08), (5.0, 0.04)];
    for &(off, alpha) in &layers {
        draw_poly(cx + off, cy + off, sides, r + off * 0.5, rot, Color::new(0.0, 0.0, 0.0, alpha));
    }
}

fn item_color(item_type: &str) -> Color {
    match item_type {
        "weapon" => hex_to_color(COLOR_WEAPON),
        "armor" => hex_to_color(COLOR_ARMOR),
        "potion" => hex_to_color(COLOR_POTION),
        "gold" => hex_to_color(COLOR_GOLD),
        _ => WHITE,
    }
}

/// Decode a base64 sprite string into a nearest-filtered Texture2D.
fn decode_sprite_texture(b64: &str) -> Option<Texture2D> {
    decode_base64(b64).and_then(|bytes|
        Image::from_file_with_format(&bytes, Some(ImageFormat::Png)).ok().map(|img| {
            let t = Texture2D::from_image(&img);
            t.set_filter(FilterMode::Nearest);
            t
        })
    )
}

/// Load all textures (tiles, monsters, items, traps) from a level into the provided maps.
fn load_level_textures(
    level: &game::Level,
    tile_textures: &mut std::collections::HashMap<String, Texture2D>,
    monster_textures: &mut std::collections::HashMap<String, Texture2D>,
    item_textures: &mut std::collections::HashMap<String, Texture2D>,
) {
    tile_textures.clear();
    for (name, def) in &level.tile_defs {
        if let Some(ref b64) = def.image {
            if let Some(tex) = decode_sprite_texture(b64) {
                tile_textures.insert(name.clone(), tex);
            }
        }
    }
    monster_textures.clear();
    for mon in &level.monsters {
        if !monster_textures.contains_key(&mon.name) {
            if let Some(ref b64) = mon.image {
                if let Some(tex) = decode_sprite_texture(b64) {
                    monster_textures.insert(mon.name.clone(), tex);
                }
            }
        }
    }
    item_textures.clear();
    for item in &level.items {
        if !item_textures.contains_key(&item.name) {
            if let Some(ref b64) = item.image {
                if let Some(tex) = decode_sprite_texture(b64) {
                    item_textures.insert(item.name.clone(), tex);
                }
            }
        }
    }
    for trap in &level.traps {
        if !item_textures.contains_key(&trap.name) {
            if let Some(ref b64) = &trap.image {
                if let Some(tex) = decode_sprite_texture(b64) {
                    item_textures.insert(trap.name.clone(), tex);
                }
            }
        }
    }
    // Sign sprite (shared across all signposts)
    if !item_textures.contains_key("__sign__") {
        if let Some(sign) = level.signposts.first() {
            if let Some(ref b64) = sign.image {
                if let Some(tex) = decode_sprite_texture(b64) {
                    item_textures.insert("__sign__".to_string(), tex);
                }
            }
        }
    }
}

fn config_dir() -> std::path::PathBuf {
    let base = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    base.join("com.allmuddy.scapegrace")
}

fn config_path() -> std::path::PathBuf {
    config_dir().join("config")
}

fn played_campaigns_path() -> std::path::PathBuf {
    config_dir().join("played_campaigns")
}

fn load_played_campaigns() -> std::collections::HashSet<String> {
    std::fs::read_to_string(played_campaigns_path())
        .unwrap_or_default()
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn mark_campaign_played(id: &str) {
    let _ = std::fs::create_dir_all(config_dir());
    let mut played = load_played_campaigns();
    played.insert(id.to_string());
    let content: Vec<String> = played.into_iter().collect();
    let _ = std::fs::write(played_campaigns_path(), content.join("\n"));
}

fn pick_next_campaign(campaigns: &[gen::BundledCampaign]) -> Option<usize> {
    let played = load_played_campaigns();

    // Play campaigns in order — find the first unplayed one
    for (i, c) in campaigns.iter().enumerate() {
        if !played.contains(&c.id) {
            return Some(i);
        }
    }

    // All played — reset and start from the beginning
    let _ = std::fs::remove_file(played_campaigns_path());
    eprintln!("All {} campaigns completed! Starting the journey again.", campaigns.len());
    Some(0)
}

// ── In-progress campaign save/restore ──

fn campaign_save_path() -> std::path::PathBuf {
    config_dir().join("campaign_progress.json")
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CampaignProgress {
    campaign_id: String,
    player: game::Player,
    completed_nodes: Vec<usize>,
    unlocked_nodes: Vec<usize>,
    current_node: usize,
    connections: Vec<(usize, usize)>,
}

fn save_campaign_progress(campaign_id: &str, player: &game::Player, overworld: &game::Overworld) {
    let completed: Vec<usize> = overworld.nodes.iter().enumerate()
        .filter(|(_, n)| n.completed).map(|(i, _)| i).collect();
    let unlocked: Vec<usize> = overworld.nodes.iter().enumerate()
        .filter(|(_, n)| n.unlocked).map(|(i, _)| i).collect();

    let progress = CampaignProgress {
        campaign_id: campaign_id.to_string(),
        player: player.clone(),
        completed_nodes: completed,
        unlocked_nodes: unlocked,
        current_node: overworld.current_node,
        connections: overworld.connections.clone(),
    };

    let _ = std::fs::create_dir_all(config_dir());
    if let Ok(json) = serde_json::to_string(&progress) {
        let _ = std::fs::write(campaign_save_path(), json);
        eprintln!("Campaign progress saved");
    }
}

fn load_campaign_progress() -> Option<CampaignProgress> {
    let content = std::fs::read_to_string(campaign_save_path()).ok()?;
    serde_json::from_str(&content).ok()
}

fn clear_campaign_progress() {
    let _ = std::fs::remove_file(campaign_save_path());
}

// ── Campaign completion records (loot tracking) ──

fn completions_path() -> std::path::PathBuf {
    config_dir().join("campaign_completions.json")
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct CampaignCompletion {
    campaign_id: String,
    potions: i32,
    speed_potions: i32,
    bombs: i32,
    gold: i32,
    level: i32,
}

fn load_completions() -> Vec<CampaignCompletion> {
    std::fs::read_to_string(completions_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_completion(player: &game::Player, campaign_id: &str) {
    let mut completions = load_completions();
    // Don't duplicate
    if completions.iter().any(|c| c.campaign_id == campaign_id) { return; }
    completions.push(CampaignCompletion {
        campaign_id: campaign_id.to_string(),
        potions: player.potions,
        speed_potions: player.speed_potions,
        bombs: player.bombs,
        gold: player.gold,
        level: player.level,
    });
    let _ = std::fs::create_dir_all(config_dir());
    if let Ok(json) = serde_json::to_string(&completions) {
        let _ = std::fs::write(completions_path(), json);
    }
}

// ── Persistent player save (carries across campaigns) ──

fn player_save_path() -> std::path::PathBuf {
    config_dir().join("player_save.json")
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct PlayerSave {
    level: i32,
    xp: i32,
    xp_to_next: i32,
    max_hp: i32,
    attack: i32,
    defense: i32,
    gold: i32,
    potions: i32,
    bombs: i32,
    speed_potions: i32,
}

fn save_player(player: &game::Player) {
    let save = PlayerSave {
        level: player.level,
        xp: player.xp,
        xp_to_next: player.xp_to_next,
        max_hp: player.max_hp,
        attack: player.attack,
        defense: player.defense,
        gold: player.gold,
        potions: player.potions,
        bombs: player.bombs,
        speed_potions: player.speed_potions,
    };
    let _ = std::fs::create_dir_all(config_dir());
    if let Ok(json) = serde_json::to_string(&save) {
        let _ = std::fs::write(player_save_path(), json);
        eprintln!("Player save written: level {}, {} gold, {} potions", save.level, save.gold, save.potions);
    }
}

fn load_player_save() -> Option<PlayerSave> {
    let content = std::fs::read_to_string(player_save_path()).ok()?;
    serde_json::from_str(&content).ok()
}

fn apply_player_save(player: &mut game::Player, save: &PlayerSave) {
    player.level = save.level;
    player.xp = save.xp;
    player.xp_to_next = save.xp_to_next;
    player.max_hp = save.max_hp;
    player.hp = save.max_hp;
    player.attack = save.attack;
    player.defense = save.defense;
    player.gold = save.gold;
    player.potions = save.potions;
    player.bombs = save.bombs;
    player.speed_potions = save.speed_potions;
}

fn save_config(value: &str) {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(config_path(), value);
}

fn load_config() {
    if let Ok(value) = std::fs::read_to_string(config_path()) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            if is_ollama_input(trimmed) {
                std::env::set_var("LLM_BASE_URL", normalize_ollama_url(trimmed));
                if std::env::var("LLM_MODEL").is_err() {
                    std::env::set_var("LLM_MODEL", "qwen2.5:14b");
                }
            } else {
                std::env::set_var("LLM_API_KEY", trimmed);
            }
        }
    }
}

fn saved_config_value() -> String {
    std::fs::read_to_string(config_path())
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Detect if input looks like an Ollama URL/hostname (not an API key)
fn is_ollama_input(input: &str) -> bool {
    input.starts_with("http://") || input.starts_with("https://")
        || (input.contains('.') && !input.starts_with("sk-"))
}

/// Normalize various Ollama URL formats to a full URL with /v1 path
fn normalize_ollama_url(input: &str) -> String {
    let mut url = input.to_string();

    // Add http:// if no scheme
    if !url.starts_with("http://") && !url.starts_with("https://") {
        url = format!("http://{}", url);
    }

    // Add default port if none specified
    // Parse to check: if there's no port after the host, add :11434
    if let Ok(parsed) = url.parse::<reqwest::Url>() {
        if parsed.port().is_none() {
            // Insert port before the path
            let scheme = parsed.scheme();
            let host = parsed.host_str().unwrap_or("localhost");
            let path = parsed.path();
            url = format!("{}://{}:11434{}", scheme, host, path);
        }
    }

    // Add /v1 if not present
    let url = url.trim_end_matches('/');
    if !url.ends_with("/v1") {
        format!("{}/v1", url)
    } else {
        url.to_string()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // Load .env if present
    match dotenvy::dotenv() {
        Ok(path) => println!("[config] loaded .env from {}", path.display()),
        Err(_) => println!("[config] no .env found"),
    }

    // Load saved config from OS config dir
    let config_file = config_path();
    match std::fs::read_to_string(&config_file) {
        Ok(val) if !val.trim().is_empty() => {
            println!("[config] loaded saved config from {}: {:?}", config_file.display(), val.trim());
        }
        Ok(_) => println!("[config] saved config at {} is empty", config_file.display()),
        Err(_) => println!("[config] no saved config at {}", config_file.display()),
    }
    load_config();

    println!("[config] LLM_BASE_URL = {:?}", std::env::var("LLM_BASE_URL").ok());
    println!("[config] LLM_API_KEY = {:?}", std::env::var("LLM_API_KEY").ok().map(|k| {
        if k.len() > 8 { format!("{}...{}", &k[..4], &k[k.len()-4..]) } else { "(set)".into() }
    }));
    println!("[config] LLM_MODEL = {:?}", std::env::var("LLM_MODEL").ok());

    let ui_font = load_ttf_font_from_bytes(include_bytes!("../assets/JetBrainsMono-Regular.ttf"))
        .expect("Failed to load embedded UI font");
    let ui_font_bold = load_ttf_font_from_bytes(include_bytes!("../assets/JetBrainsMono-Bold.ttf"))
        .expect("Failed to load embedded UI bold font");

    let sfx = sfx::Sfx::new();

    let mut state = GameState::new();
    let bundled_pack = gen::load_bundled_pack();
    let pack_strings = bundled_pack.as_ref().map(|p| p.strings.clone()).unwrap_or_default();
    let pack_item_sprites = bundled_pack.as_ref().map(|p| p.item_sprites.clone()).unwrap_or_default();
    state.item_sprites = pack_item_sprites.clone();
    let bundled_campaigns: Vec<gen::BundledCampaign> = bundled_pack.map(|p| p.campaigns).unwrap_or_default();
    let has_bundled = !bundled_campaigns.is_empty();
    let has_llm = !has_bundled && (!gen::llm_api_key().is_empty() || std::env::var("LLM_BASE_URL").is_ok());
    println!("[config] has_bundled={}, has_llm={} → screen={}",
        has_bundled, has_llm,
        if has_bundled || has_llm { "Start" } else { "KeyEntry" });
    let mut screen = if has_bundled { Screen::CampaignSelect } else if has_llm { Screen::Start } else { Screen::KeyEntry };
    let mut key_input = saved_config_value();
    let mut key_error: Option<String> = None;
    let mut key_validating = false;
    let mut key_rx: Option<mpsc::Receiver<Result<(), String>>> = None;
    let mut gen_rx: Option<mpsc::Receiver<GenMsg>> = None;
    let mut phase_text = String::new();
    let mut phase_detail = String::new();
    let mut loading_tiles: usize = 0;
    let mut confetti: Vec<Confetti> = vec![];
    let mut title_font: Option<Font> = None;
    let mut tile_textures: std::collections::HashMap<String, Texture2D> = std::collections::HashMap::new();
    let current_zoom: f32 = 3.0;
    let mut monster_textures: std::collections::HashMap<String, Texture2D> = std::collections::HashMap::new();
    let mut item_textures: std::collections::HashMap<String, Texture2D> = std::collections::HashMap::new();
    let mut active_signpost: Option<usize> = None;
    let mut signpost_fonts: std::collections::HashMap<String, Font> = std::collections::HashMap::new();
    let mut overworld_font: Option<Font> = None;
    let mut desc_font: Option<Font> = None;
    let mut label_font: Option<Font> = None;

    // Overworld state
    let mut current_campaign_id: Option<String> = None;
    let mut current_campaign_idx: usize = 0;
    let mut current_campaign_settings = gen::CampaignSettings::default();
    let mut current_campaign_monsters: Option<Vec<gen::MonsterTemplateRaw>> = None;
    let mut overworld: Option<Overworld> = None;
    let mut overworld_bg_tex: Option<Texture2D> = None;
    let mut level_designs: Vec<Option<gen::Phase2Result>> = Vec::new();
    let mut design_token_flashes: Vec<Vec<f64>> = Vec::new(); // per-node flash times
    let mut bg_gen_rx: Option<mpsc::Receiver<GenMsg>> = None;
    let mut player_snapshot: Option<Player> = None;
    let mut level_snapshot: Option<(usize, Level, [i32; 2])> = None; // (node_index, level, start) for retry

    // Campaign select state
    let mut campaign_select_idx: usize = pick_next_campaign(&bundled_campaigns).unwrap_or(0);
    let mut ghost_town: bool = false; // viewing a completed campaign
    let mut cheat_active: bool = false;
    let mut cs_hold_time: f64 = 0.0;
    let mut cs_last_fire: f64 = 0.0;

    // Store state
    let mut store_selection: usize = 0;

    // Cheat code state
    let mut cheat_buf: Vec<char> = Vec::new();

    // Sound test state
    let mut st_root: usize = 0;
    let mut st_scale: usize = 0;
    let mut st_selected: usize = 0;
    let mut st_reverb_on: bool = true;
    let mut st_reverb_room: f32 = 0.5;   // 0.0 = tight corridor, 1.0 = huge cavern
    let mut st_smooth_on: bool = true;

    // Key repeat
    let mut nav_hold_time: f64 = 0.0;
    let mut nav_last_fire: f64 = 0.0;
    let mut nav_last_dir: (f32, f32) = (0.0, 0.0);
    let mut nav_cycle_idx: usize = 0;
    let mut game_hold_time: f64 = 0.0;
    let mut teleport_last_mouse: Option<(f32, f32)> = None;
    let mut teleport_cam_offset: (f32, f32) = (0.0, 0.0);
    let mut teleport_hover_tile: (i32, i32) = (0, 0);
    let mut teleport_zoom: f32 = 1.0;
    let mut teleport_target_zoom: f32 = 0.2;
    let mut teleport_dest: Option<(i32, i32)> = None; // chosen destination
    let mut teleport_phase: u8 = 0; // 0=zoom_out, 1=browse, 2=travel, 3=zoom_in
    let mut teleport_is_cancel: bool = false;
    let mut teleport_cheat: bool = false; // xyzzy unlocks fog reveal + right-click teleport
    let mut game_last_fire: f64 = 0.0;

    loop {
        clear_background(Color::new(0.04, 0.04, 0.04, 1.0));
        match screen {
            Screen::KeyEntry => {
                draw_key_entry_screen(&ui_font, &ui_font_bold, &key_input, &key_error, key_validating);

                // Check validation result
                if let Some(rx) = &key_rx {
                    if let Ok(result) = rx.try_recv() {
                        key_validating = false;
                        match result {
                            Ok(()) => {
                                let trimmed = key_input.trim();
                                if is_ollama_input(trimmed) {
                                    let url = normalize_ollama_url(trimmed);
                                    std::env::set_var("LLM_BASE_URL", &url);
                                    if std::env::var("LLM_MODEL").is_err() {
                                        std::env::set_var("LLM_MODEL", "qwen2.5:14b");
                                    }
                                    save_config(&url);
                                } else {
                                    std::env::set_var("LLM_API_KEY", trimmed);
                                    save_config(trimmed);
                                }
                                screen = Screen::Start;
                            }
                            Err(e) => {
                                key_error = Some(e);
                            }
                        }
                        key_rx = None;
                    }
                }

                if !key_validating {
                    let cmd = is_key_down(KeyCode::LeftSuper) || is_key_down(KeyCode::RightSuper)
                        || is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);

                    // Paste (Cmd+V)
                    if cmd && is_key_pressed(KeyCode::V) {
                        if let Ok(mut clip) = arboard::Clipboard::new() {
                            if let Ok(text) = clip.get_text() {
                                let clean: String = text.chars().filter(|c| c.is_ascii_graphic()).collect();
                                key_input.push_str(&clean);
                                key_error = None;
                            }
                        }
                        while get_char_pressed().is_some() {}
                    } else {
                        // Normal text input
                        while let Some(ch) = get_char_pressed() {
                            if ch.is_ascii_graphic() {
                                key_input.push(ch);
                                key_error = None;
                            }
                        }
                    }
                    // Backspace with key repeat
                    if is_key_down(KeyCode::Backspace) {
                        let now = get_time();
                        if is_key_pressed(KeyCode::Backspace) {
                            key_input.pop();
                            key_error = None;
                            nav_hold_time = now;
                            nav_last_fire = now;
                        } else if nav_hold_time > 0.0
                            && now - nav_hold_time >= NAV_INITIAL_DELAY
                            && now - nav_last_fire >= GAME_REPEAT_RATE
                        {
                            key_input.pop();
                            key_error = None;
                            nav_last_fire = now;
                        }
                    } else if !is_key_down(KeyCode::Left) && !is_key_down(KeyCode::Right)
                        && !is_key_down(KeyCode::Up) && !is_key_down(KeyCode::Down)
                        && !is_key_down(KeyCode::A) && !is_key_down(KeyCode::W)
                        && !is_key_down(KeyCode::S) && !is_key_down(KeyCode::D) {
                        nav_hold_time = 0.0;
                    }
                    if is_key_pressed(KeyCode::Enter) {
                        let trimmed = key_input.trim().to_string();
                        if trimmed.is_empty() {
                            key_error = Some("The passphrase cannot be empty.".into());
                        } else if is_ollama_input(&trimmed) {
                            let url = normalize_ollama_url(&trimmed);
                            key_validating = true;
                            key_error = None;
                            let (tx, rx) = mpsc::channel();
                            key_rx = Some(rx);
                            std::thread::spawn(move || {
                                let _ = tx.send(validate_ollama_url(&url));
                            });
                        } else {
                            key_validating = true;
                            key_error = None;
                            let (tx, rx) = mpsc::channel();
                            key_rx = Some(rx);
                            let key = trimmed.clone();
                            std::thread::spawn(move || {
                                let _ = tx.send(validate_api_key(&key));
                            });
                        }
                    }
                }
            }

            Screen::CampaignSelect => {
                // Cheat code: xyzzy — unlock all campaigns
                let cheat_keys = [
                    (KeyCode::X, 'x'), (KeyCode::Y, 'y'), (KeyCode::Z, 'z'),
                ];
                for &(kc, ch) in &cheat_keys {
                    if is_key_pressed(kc) { cheat_buf.push(ch); }
                }
                if cheat_buf.len() > 10 { cheat_buf.drain(..cheat_buf.len() - 10); }
                let buf_str: String = cheat_buf.iter().collect();
                if buf_str.contains("xyzzy") {
                    cheat_buf.clear();
                    cheat_active = true;
                    if let Some(s) = &sfx { s.cheat_fanfare(); }
                    eprintln!("XYZZY cheat: all campaigns unlocked!");
                }

                let played = load_played_campaigns();
                let comps = load_completions();
                let save = load_campaign_progress();
                let save_id = save.as_ref().map(|s| s.campaign_id.as_str());
                draw_campaign_select(&ui_font, &ui_font_bold, &bundled_campaigns, &comps, &played, campaign_select_idx, save_id, &pack_strings, cheat_active);

                // Navigation — clamp to unlocked campaigns only
                let max_selectable = if cheat_active {
                    bundled_campaigns.len() - 1
                } else {
                    let mut max = 0usize;
                    for (i, c) in bundled_campaigns.iter().enumerate() {
                        let unlocked = i == 0
                            || played.contains(&bundled_campaigns[i - 1].id)
                            || save_id == Some(c.id.as_str());
                        if unlocked { max = i; } else { break; }
                    }
                    max
                };
                // Navigation with key repeat
                // Up/Down = prev/next in linear order
                // Left/Right = spatial (move to adjacent column in same row)
                let cols = 5usize;
                let press_down = is_key_down(KeyCode::Down) || is_key_down(KeyCode::S);
                let press_up = is_key_down(KeyCode::Up) || is_key_down(KeyCode::W);
                let press_right = is_key_down(KeyCode::Right) || is_key_down(KeyCode::D);
                let press_left = is_key_down(KeyCode::Left) || is_key_down(KeyCode::A);
                let any_dir = press_down || press_up || press_right || press_left;
                let now = get_time();
                let nav_fire = if any_dir {
                    if cs_hold_time == 0.0 {
                        cs_hold_time = now;
                        cs_last_fire = now;
                        true
                    } else if now - cs_hold_time > 0.3 && now - cs_last_fire > 0.08 {
                        cs_last_fire = now;
                        true
                    } else {
                        false
                    }
                } else {
                    cs_hold_time = 0.0;
                    false
                };
                if nav_fire {
                    if press_down && campaign_select_idx < max_selectable {
                        campaign_select_idx += 1;
                    }
                    if press_up && campaign_select_idx > 0 {
                        campaign_select_idx -= 1;
                    }
                    if press_right || press_left {
                        // Spatial: find the visual neighbor in the pressed direction
                        let cur_row = campaign_select_idx / cols;
                        let cur_col_in_row = campaign_select_idx % cols;
                        let cur_visual_col = if cur_row % 2 == 0 { cur_col_in_row } else { cols - 1 - cur_col_in_row };
                        let target_visual_col = if press_right {
                            if cur_visual_col < cols - 1 { cur_visual_col + 1 } else { cur_visual_col }
                        } else {
                            if cur_visual_col > 0 { cur_visual_col - 1 } else { cur_visual_col }
                        };
                        // Convert back to linear index
                        let target_col_in_row = if cur_row % 2 == 0 { target_visual_col } else { cols - 1 - target_visual_col };
                        let target_idx = cur_row * cols + target_col_in_row;
                        if target_idx <= max_selectable && target_idx < bundled_campaigns.len() {
                            campaign_select_idx = target_idx;
                        }
                    }
                }

                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                    let campaign = &bundled_campaigns[campaign_select_idx];
                    let is_played = played.contains(&campaign.id);
                    let is_in_progress = save_id == Some(campaign.id.as_str());
                    let is_unlocked = cheat_active || campaign_select_idx == 0 || played.contains(&bundled_campaigns[campaign_select_idx - 1].id) || is_in_progress;

                    if is_unlocked {
                        let resuming = is_in_progress;
                        let entering_ghost = is_played;
                        eprintln!("CampaignSelect: launching campaign {}: \"{}\"{}{}",
                            campaign_select_idx, campaign.overworld.name,
                            if resuming { " (RESUMING)" } else { "" },
                            if entering_ghost { " (GHOST TOWN)" } else { "" });

                        match gen::build_overworld_from_result(campaign.overworld.clone()) {
                            Ok(mut ow) => {
                                if entering_ghost {
                                    // Ghost town: mark all nodes completed and unlocked
                                    for node in &mut ow.nodes {
                                        node.completed = true;
                                        node.unlocked = true;
                                    }
                                    ghost_town = true;
                                    state = GameState::new();
                                    state.item_sprites = pack_item_sprites.clone();
                                } else if let (true, Some(progress)) = (resuming, save) {
                                    ghost_town = false;
                                    state.player = progress.player;
                                    for &i in &progress.completed_nodes {
                                        if i < ow.nodes.len() { ow.nodes[i].completed = true; }
                                    }
                                    for &i in &progress.unlocked_nodes {
                                        if i < ow.nodes.len() { ow.nodes[i].unlocked = true; }
                                    }
                                    ow.current_node = progress.current_node;
                                    if !progress.connections.is_empty() {
                                        ow.connections = progress.connections;
                                    }
                                } else {
                                    ghost_town = false;
                                    state = GameState::new();
                                    state.item_sprites = pack_item_sprites.clone();
                                    if let Some(save) = load_player_save() {
                                        apply_player_save(&mut state.player, &save);
                                        eprintln!("Applied player save: level {}, {} gold", save.level, save.gold);
                                    }
                                }

                                if let Some(bytes) = fetch_google_font(&ow.font) {
                                    if let Ok(f) = load_ttf_font_from_bytes(&bytes) { overworld_font = Some(f); }
                                }
                                if let Some(bytes) = fetch_google_font(&ow.description_font) {
                                    if let Ok(f) = load_ttf_font_from_bytes(&bytes) { desc_font = Some(f); }
                                }
                                if let Some(bytes) = fetch_google_font(&ow.label_font) {
                                    if let Ok(f) = load_ttf_font_from_bytes(&bytes) { label_font = Some(f); }
                                }

                                // Load all designs (including completed ones for ghost town viewing)
                                let level_nodes: Vec<usize> = ow.nodes.iter().enumerate()
                                    .filter(|(_, n)| n.node_type == NodeType::Level)
                                    .map(|(i, _)| i)
                                    .collect();
                                level_designs = campaign.designs.iter()
                                    .take(level_nodes.len())
                                    .map(|d| Some(d.clone()))
                                    .collect();
                                while level_designs.len() < level_nodes.len() {
                                    level_designs.push(None);
                                }
                                design_token_flashes = vec![Vec::new(); level_nodes.len()];
                                bg_gen_rx = None;

                                current_campaign_id = Some(campaign.id.clone());
                                current_campaign_idx = campaign_select_idx;
                                ow.scale_store_prices(campaign_select_idx as i32);
                                current_campaign_settings = campaign.settings.clone();
                                current_campaign_monsters = campaign.monster_templates.clone();

                                if !resuming && !entering_ghost && !cheat_active {
                                    save_campaign_progress(&campaign.id, &state.player, &ow);
                                }

                                if entering_ghost {
                                    for slot in &mut ow.store_stock {
                                        slot.stock = 0;
                                    }
                                }

                                // Load bg texture if present
                                overworld_bg_tex = ow.bg_image.as_ref().and_then(|b64| {
                                    decode_base64(b64).and_then(|bytes| {
                                        Image::from_file_with_format(&bytes, Some(ImageFormat::Png)).ok().map(|img| {
                                            let tex = Texture2D::from_image(&img);
                                            tex.set_filter(FilterMode::Linear);
                                            tex
                                        })
                                    })
                                });
                                // Check for WYSIWYG unified map
                                if campaign.prebuilt_overworld_map.is_some() {
                                    match gen::build_unified_level(campaign, &pack_item_sprites) {
                                        Ok((level, start)) => {
                                            state.level = level;
                                            let resume_walkable = resuming && {
                                                let px = state.player.x;
                                                let py = state.player.y;
                                                px >= 0 && py >= 0 && py < state.level.height && px < state.level.width
                                                && state.level.tile_defs.get(&state.level.tiles[py as usize][px as usize])
                                                    .map(|t| t.walkable).unwrap_or(false)
                                            };
                                            if !resume_walkable {
                                                state.player.x = start[0];
                                                state.player.y = start[1];
                                            }
                                            state.game_over = false;
                                            state.victory = false;
                                            state.log.clear();
                                            state.vision_radius = 12;
                                            if !ghost_town {
                                                let (px, py, vis) = (state.player.x, state.player.y, state.vision_radius);
                                                crate::game::reveal_around(&mut state.level, px, py, vis);
                                            }
                                            state.log(&state.level.description.clone(), "#888");
                                            if ghost_town {
                                                state.log("Nothing stirs. The halls are empty.", "#666");
                                            } else {
                                                state.log("Your task: find and defeat the boss.", "#666");
                                            }
                                            load_level_textures(&state.level, &mut tile_textures, &mut monster_textures, &mut item_textures);
                                            // Preload signpost fonts
                                            for sign in &state.level.signposts {
                                                for fname in [&sign.title_font, &sign.description_font] {
                                                    if let Some(f) = fname {
                                                        if !f.is_empty() && !signpost_fonts.contains_key(f) {
                                                            if let Some(bytes) = fetch_google_font(f) {
                                                                if let Ok(font) = load_ttf_font_from_bytes(&bytes) {
                                                                    signpost_fonts.insert(f.clone(), font);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            if let Some(s) = &sfx {
                                                s.start_boss_drone(state.level.scale_at(state.player.x, state.player.y));
                                                let openness = game::measure_openness(&state.level, state.player.x, state.player.y);
                                                s.update_room_acoustics(openness);
                                            }
                                            overworld = Some(ow);
                                            screen = Screen::Playing;
                                            eprintln!("Unified map loaded: {}x{} with {} monsters, {} items, player at ({},{})",
                                                state.level.width, state.level.height,
                                                state.level.monsters.len(), state.level.items.len(),
                                                state.player.x, state.player.y);
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to build unified map: {}, falling back", e);
                                            overworld = Some(ow);
                                            screen = Screen::Playing;
                                        }
                                    }
                                } else {
                                    overworld = Some(ow);
                                    screen = Screen::Playing;
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to build overworld: {}", e);
                            }
                        }
                    }
                }

                if is_key_pressed(KeyCode::T) {
                    screen = Screen::SoundTest;
                }
            }

            Screen::Start => {
                if has_bundled {
                    let has_save = load_campaign_progress().is_some();
                    draw_bundled_start_screen(&ui_font, &ui_font_bold, bundled_campaigns.len(), &load_played_campaigns(), has_save, &pack_strings);
                } else {
                    draw_start_screen(&ui_font, &ui_font_bold);
                }
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                    if !bundled_campaigns.is_empty() {
                        // Check for saved in-progress campaign first
                        let saved = load_campaign_progress();
                        let (idx, resuming) = if let Some(ref progress) = saved {
                            // Find the campaign by ID
                            if let Some(i) = bundled_campaigns.iter().position(|c| c.id == progress.campaign_id) {
                                eprintln!("Resuming campaign {}: \"{}\"", i, bundled_campaigns[i].overworld.name);
                                (i, true)
                            } else {
                                eprintln!("Saved campaign not found, starting fresh");
                                clear_campaign_progress();
                                (pick_next_campaign(&bundled_campaigns).unwrap(), false)
                            }
                        } else {
                            (pick_next_campaign(&bundled_campaigns).unwrap(), false)
                        };

                        let campaign = &bundled_campaigns[idx];
                        let played = load_played_campaigns();
                        eprintln!("Using bundled campaign {}: \"{}\" (score: {}, {}/{} played{})",
                            idx, campaign.overworld.name, campaign.quality.score,
                            played.len(), bundled_campaigns.len(),
                            if resuming { ", RESUMING" } else { "" });

                        match gen::build_overworld_from_result(campaign.overworld.clone()) {
                            Ok(mut ow) => {
                                // Restore saved progress if resuming
                                if let (true, Some(progress)) = (resuming, saved) {
                                    state.player = progress.player;
                                    for &i in &progress.completed_nodes {
                                        if i < ow.nodes.len() { ow.nodes[i].completed = true; }
                                    }
                                    for &i in &progress.unlocked_nodes {
                                        if i < ow.nodes.len() { ow.nodes[i].unlocked = true; }
                                    }
                                    ow.current_node = progress.current_node;
                                    // Restore connections (branch point is randomized on build)
                                    if !progress.connections.is_empty() {
                                        ow.connections = progress.connections;
                                    }
                                    eprintln!("Restored: player level {}, {}/{} HP, {} nodes completed",
                                        state.player.level, state.player.hp, state.player.max_hp,
                                        progress.completed_nodes.len());
                                } else if let Some(psave) = load_player_save() {
                                    apply_player_save(&mut state.player, &psave);
                                    eprintln!("Applied player save: level {}, {} gold", psave.level, psave.gold);
                                }

                                // Load fonts
                                if let Some(bytes) = fetch_google_font(&ow.font) {
                                    if let Ok(f) = load_ttf_font_from_bytes(&bytes) { overworld_font = Some(f); }
                                }
                                if let Some(bytes) = fetch_google_font(&ow.description_font) {
                                    if let Ok(f) = load_ttf_font_from_bytes(&bytes) { desc_font = Some(f); }
                                }
                                if let Some(bytes) = fetch_google_font(&ow.label_font) {
                                    if let Ok(f) = load_ttf_font_from_bytes(&bytes) { label_font = Some(f); }
                                }

                                // Pre-populate all designs from bundled data
                                let playable = ow.nodes.iter()
                                    .filter(|n| n.node_type == NodeType::Level && !n.completed).count();
                                level_designs = campaign.designs.iter()
                                    .take(playable)
                                    .map(|d| Some(d.clone()))
                                    .collect();
                                // Pad if fewer designs than playable nodes
                                while level_designs.len() < playable {
                                    level_designs.push(None);
                                }
                                design_token_flashes = vec![Vec::new(); playable];
                                // No background design generation needed
                                bg_gen_rx = None;

                                current_campaign_id = Some(campaign.id.clone());
                                current_campaign_idx = idx;
                                ow.scale_store_prices(idx as i32);
                                current_campaign_settings = campaign.settings.clone();

                                // Save initial progress (captures connections/topology)
                                if !resuming && !cheat_active {
                                    save_campaign_progress(&campaign.id, &state.player, &ow);
                                }

                                // Load bg texture if present
                                overworld_bg_tex = ow.bg_image.as_ref().and_then(|b64| {
                                    decode_base64(b64).and_then(|bytes| {
                                        Image::from_file_with_format(&bytes, Some(ImageFormat::Png)).ok().map(|img| {
                                            let tex = Texture2D::from_image(&img);
                                            tex.set_filter(FilterMode::Linear);
                                            tex
                                        })
                                    })
                                });
                                // Check for WYSIWYG unified map
                                if campaign.prebuilt_overworld_map.is_some() {
                                    match gen::build_unified_level(campaign, &pack_item_sprites) {
                                        Ok((level, start)) => {
                                            state.level = level;
                                            state.player.x = start[0];
                                            state.player.y = start[1];
                                            state.game_over = false;
                                            state.victory = false;
                                            state.log.clear();
                                            state.vision_radius = 12;
                                            if !ghost_town {
                                                let (px, py, vis) = (state.player.x, state.player.y, state.vision_radius);
                                                crate::game::reveal_around(&mut state.level, px, py, vis);
                                            }
                                            state.log(&state.level.description.clone(), "#888");
                                            if ghost_town {
                                                state.log("Nothing stirs. The halls are empty.", "#666");
                                            } else {
                                                state.log("Your task: find and defeat the boss.", "#666");
                                            }
                                            // Load tile textures
                                            load_level_textures(&state.level, &mut tile_textures, &mut monster_textures, &mut item_textures);
                                            // Preload signpost fonts
                                            for sign in &state.level.signposts {
                                                for fname in [&sign.title_font, &sign.description_font] {
                                                    if let Some(f) = fname {
                                                        if !f.is_empty() && !signpost_fonts.contains_key(f) {
                                                            if let Some(bytes) = fetch_google_font(f) {
                                                                if let Ok(font) = load_ttf_font_from_bytes(&bytes) {
                                                                    signpost_fonts.insert(f.clone(), font);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            if let Some(s) = &sfx {
                                                s.start_boss_drone(state.level.scale_at(state.player.x, state.player.y));
                                                let openness = game::measure_openness(&state.level, state.player.x, state.player.y);
                                                s.update_room_acoustics(openness);
                                            }
                                            overworld = Some(ow);
                                            screen = Screen::Playing;
                                            println!("Unified map loaded: {}x{} with {} monsters, {} items",
                                                state.level.width, state.level.height,
                                                state.level.monsters.len(), state.level.items.len());
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to build unified map: {}, falling back to overworld", e);
                                            overworld = Some(ow);
                                            screen = Screen::Playing;
                                        }
                                    }
                                } else {
                                    overworld = Some(ow);
                                    screen = Screen::Playing;
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to build overworld from bundled campaign: {}", e);
                                // Fall back to LLM generation
                                start_overworld_generation(&mut gen_rx);
                                screen = Screen::GenOverworld;
                                phase_text = overworld_loading_phrase();
                                phase_detail.clear();
                                loading_tiles = 0;
                            }
                        }
                    } else {
                        start_overworld_generation(&mut gen_rx);
                        screen = Screen::GenOverworld;
                        phase_text = overworld_loading_phrase();
                        phase_detail.clear();
                        loading_tiles = 0;
                    }
                }
                if is_key_pressed(KeyCode::T) {
                    screen = Screen::SoundTest;
                }
            }

            Screen::SoundTest => {
                const ROOTS: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
                const SCALES: [&str; 7] = [
                    "ionian", "dorian", "phrygian", "lydian", "mixolydian", "aeolian", "locrian",
                ];
                const SOUNDS: [&str; 18] = [
                    "footstep", "hit", "crit", "player_hurt", "miss", "kill", "death",
                    "victory", "pickup_gold", "pickup_potion", "pickup_weapon", "pickup_armor",
                    "pickup_key", "level_up", "trap", "bomb", "boss_kill", "cheat_fanfare",
                ];

                let scale = gen::build_scale(ROOTS[st_root], SCALES[st_scale]);

                // Input
                if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Q) {
                    screen = Screen::Start;
                }
                if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
                    st_root = (st_root + ROOTS.len() - 1) % ROOTS.len();
                }
                if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
                    st_root = (st_root + 1) % ROOTS.len();
                }
                if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                    if st_selected > 0 { st_selected -= 1; }
                }
                if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                    if st_selected < SOUNDS.len() - 1 { st_selected += 1; }
                }
                if is_key_pressed(KeyCode::Tab) {
                    st_scale = (st_scale + 1) % SCALES.len();
                }
                // FX toggles
                if is_key_pressed(KeyCode::R) {
                    st_reverb_on = !st_reverb_on;
                    if let Some(s) = &sfx { s.set_reverb_enabled(st_reverb_on); }
                }
                if is_key_pressed(KeyCode::F) {
                    st_smooth_on = !st_smooth_on;
                    if let Some(s) = &sfx { s.set_smooth_enabled(st_smooth_on); }
                }
                // Room size (reverb amount) with - and =
                if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::LeftBracket) {
                    st_reverb_room = (st_reverb_room - 0.1).max(0.0);
                    if let Some(s) = &sfx { s.update_room_acoustics(st_reverb_room); }
                }
                if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::RightBracket) {
                    st_reverb_room = (st_reverb_room + 0.1).min(1.0);
                    if let Some(s) = &sfx { s.update_room_acoustics(st_reverb_room); }
                }
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                    if let Some(s) = &sfx {
                        match SOUNDS[st_selected] {
                            "footstep" => s.footstep(&scale),
                            "hit" => s.hit(&scale),
                            "crit" => s.crit(&scale),
                            "player_hurt" => s.player_hurt(&scale),
                            "miss" => s.miss(&scale),
                            "kill" => s.kill(&scale),
                            "death" => s.death(&scale),
                            "victory" => s.victory(&scale),
                            "pickup_gold" => s.pickup_gold(&scale),
                            "pickup_potion" => s.pickup_potion(&scale),
                            "pickup_weapon" => s.pickup_weapon(&scale),
                            "pickup_armor" => s.pickup_armor(&scale),
                            "pickup_key" => s.pickup_key(&scale),
                            "level_up" => s.level_up(&scale),
                            "trap" => s.trap(&scale),
                            "bomb" => s.bomb(&scale),
                            "boss_kill" => s.boss_kill(&scale),
                            "cheat_fanfare" => s.cheat_fanfare(),
                            _ => {}
                        }
                    }
                }

                // Draw
                let sw = screen_width();
                let title = "SOUND TEST";
                let ts = 36u16;
                let tw = measure_text(title, Some(&ui_font_bold), ts, 1.0).width;
                draw_text_ex(title, (sw - tw) / 2.0, 50.0, TextParams {
                    font: Some(&ui_font_bold), font_size: ts, color: hex_to_color("#e94560"), ..Default::default()
                });

                // Mode selector
                let mode_text = format!("<  {} {}  >", ROOTS[st_root], SCALES[st_scale]);
                let ms = 20u16;
                let mw = measure_text(&mode_text, Some(&ui_font_bold), ms, 1.0).width;
                draw_text_ex(&mode_text, (sw - mw) / 2.0, 90.0, TextParams {
                    font: Some(&ui_font_bold), font_size: ms, color: hex_to_color("#ffd700"), ..Default::default()
                });

                let hint = "LEFT/RIGHT: root   TAB: scale   ESC: back";
                let hs = 13u16;
                let hw = measure_text(hint, Some(&ui_font), hs, 1.0).width;
                draw_text_ex(hint, (sw - hw) / 2.0, 112.0, TextParams {
                    font: Some(&ui_font), font_size: hs, color: DARKGRAY, ..Default::default()
                });

                let hint2 = "R: reverb   F: smooth   [/]: room size";
                let h2w = measure_text(hint2, Some(&ui_font), hs, 1.0).width;
                draw_text_ex(hint2, (sw - h2w) / 2.0, 128.0, TextParams {
                    font: Some(&ui_font), font_size: hs, color: DARKGRAY, ..Default::default()
                });

                // Sound list
                let start_y = 145.0;
                let row_h = 28.0;
                let ls = 18u16;
                for (i, name) in SOUNDS.iter().enumerate() {
                    let sel = i == st_selected;
                    let label = if sel { format!("> {}", name) } else { format!("  {}", name) };
                    let color = if sel { WHITE } else { GRAY };
                    let lw = measure_text(&label, Some(&ui_font), ls, 1.0).width;
                    draw_text_ex(&label, (sw - lw) / 2.0, start_y + i as f32 * row_h, TextParams {
                        font: Some(&ui_font), font_size: ls, color, ..Default::default()
                    });
                }

                let play_hint = "ENTER: play sound";
                let ps = 14u16;
                let pw = measure_text(play_hint, Some(&ui_font), ps, 1.0).width;
                let bottom_y = start_y + SOUNDS.len() as f32 * row_h + 20.0;
                draw_text_ex(play_hint, (sw - pw) / 2.0, bottom_y, TextParams {
                    font: Some(&ui_font), font_size: ps, color: DARKGRAY, ..Default::default()
                });

                // FX status panel
                let fx_y = bottom_y + 30.0;
                let fx_size = 15u16;

                let reverb_status = if st_reverb_on { "ON" } else { "OFF" };
                let reverb_color = if st_reverb_on { hex_to_color("#4ecca3") } else { hex_to_color("#e94560") };
                let (delay, feedback, wet) = if let Some(s) = &sfx {
                    s.reverb_params()
                } else {
                    (0.0, 0.0, 0.0)
                };
                let reverb_text = format!(
                    "REVERB [R]: {}   delay:{:.0}ms  feedback:{:.0}%  wet:{:.0}%",
                    reverb_status, delay, feedback * 100.0, wet * 100.0
                );
                let rw = measure_text(&reverb_text, Some(&ui_font), fx_size, 1.0).width;
                draw_text_ex(&reverb_text, (sw - rw) / 2.0, fx_y, TextParams {
                    font: Some(&ui_font), font_size: fx_size, color: reverb_color, ..Default::default()
                });

                let room_text = format!(
                    "ROOM SIZE [-/=]: {:.0}%  (corridor <---> cavern)",
                    st_reverb_room * 100.0
                );
                let rmw = measure_text(&room_text, Some(&ui_font), fx_size, 1.0).width;
                draw_text_ex(&room_text, (sw - rmw) / 2.0, fx_y + 22.0, TextParams {
                    font: Some(&ui_font), font_size: fx_size, color: hex_to_color("#c4c4c4"), ..Default::default()
                });

                let smooth_status = if st_smooth_on { "ON" } else { "OFF" };
                let smooth_color = if st_smooth_on { hex_to_color("#4ecca3") } else { hex_to_color("#e94560") };
                let smooth_text = format!("ANTI-CLICK [F]: {}   (3ms fade-in/out)", smooth_status);
                let smw = measure_text(&smooth_text, Some(&ui_font), fx_size, 1.0).width;
                draw_text_ex(&smooth_text, (sw - smw) / 2.0, fx_y + 44.0, TextParams {
                    font: Some(&ui_font), font_size: fx_size, color: smooth_color, ..Default::default()
                });
            }

            Screen::GenOverworld => {
                draw_loading_screen(&ui_font, &phase_text, &phase_detail, loading_tiles);

                if let Some(rx) = &gen_rx {
                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            GenMsg::Token => {
                                loading_tiles += 1;
                            }
                            GenMsg::Phase(_p, _d) => {
                                // Don't update text — keep the loading phrase
                                // and tile blob visible until overworld is fully ready
                            }
                            GenMsg::OverworldReady(ow, font_bytes) => {
                                if let Some(bytes) = font_bytes {
                                    match load_ttf_font_from_bytes(&bytes) {
                                        Ok(f) => overworld_font = Some(f),
                                        Err(e) => eprintln!("Overworld font error: {}", e),
                                    }
                                }
                                if let Some(bytes) = fetch_google_font(&ow.description_font) {
                                    match load_ttf_font_from_bytes(&bytes) {
                                        Ok(f) => desc_font = Some(f),
                                        Err(e) => eprintln!("Desc font error: {}", e),
                                    }
                                }
                                if let Some(bytes) = fetch_google_font(&ow.label_font) {
                                    match load_ttf_font_from_bytes(&bytes) {
                                        Ok(f) => label_font = Some(f),
                                        Err(e) => eprintln!("Label font error: {}", e),
                                    }
                                }
                                // Initialize design slots (one per playable node)
                                let playable = ow.nodes.iter().filter(|n| n.node_type == NodeType::Level && !n.completed).count();
                                level_designs = vec![None; playable];
                                design_token_flashes = vec![Vec::new(); playable];
                                // Start background level design generation
                                start_background_designs(&ow, &mut bg_gen_rx);
                                // Load bg texture if present
                                overworld_bg_tex = ow.bg_image.as_ref().and_then(|b64| {
                                    decode_base64(b64).and_then(|bytes| {
                                        Image::from_file_with_format(&bytes, Some(ImageFormat::Png)).ok().map(|img| {
                                            let tex = Texture2D::from_image(&img);
                                            tex.set_filter(FilterMode::Linear);
                                            tex
                                        })
                                    })
                                });
                                overworld = Some(ow);
                                screen = Screen::Playing;
                            }
                            GenMsg::Error(e) => {
                                phase_text = format!("Error: {}", e);
                                phase_detail = "Press ENTER to retry".into();
                            }
                            _ => {}
                        }
                    }
                }

                if phase_detail == "Press ENTER to retry" && is_key_pressed(KeyCode::Enter) {
                    start_overworld_generation(&mut gen_rx);
                    phase_text = overworld_loading_phrase();
                    phase_detail.clear();
                    loading_tiles = 0;
                }
            }

            Screen::GenLevel => {
                draw_loading_screen(&ui_font, &phase_text, &phase_detail, loading_tiles);

                if let Some(rx) = &gen_rx {
                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            GenMsg::Token => {
                                loading_tiles += 1;
                            }
                            GenMsg::Phase(_p, _d) => {
                                // Keep loading phrase and blob visible
                            }
                            GenMsg::LevelDone(mut level, start, font_bytes) => {
                                if ghost_town {
                                    // Strip all monsters, items, and traps for ghost town viewing
                                    level.monsters.clear();
                                    level.items.clear();
                                    level.traps.clear();
                                    // Reveal entire map
                                    for y in 0..level.height {
                                        for x in 0..level.width {
                                            level.revealed.insert((x, y));
                                            level.visible.insert((x, y));
                                        }
                                    }
                                }
                                // Snapshot for death retry (clean level state)
                                let snap_node = overworld.as_ref().map_or(0, |ow| ow.current_node);
                                if !ghost_town {
                                    level_snapshot = Some((snap_node, level.clone(), start));
                                }
                                state.level = level;
                                load_level_textures(&state.level, &mut tile_textures, &mut monster_textures, &mut item_textures);
                                state.player.x = start[0];
                                state.player.y = start[1];
                                state.game_over = false;
                                state.victory = false;
                                state.log.clear();
                                if !ghost_town {
                                    reveal_around(
                                        &mut state.level,
                                        state.player.x,
                                        state.player.y,
                                        state.vision_radius,
                                    );
                                }
                                state.log(&state.level.description.clone(), "#888");
                                if ghost_town {
                                    state.log("Nothing stirs. The halls are empty.", "#666");
                                } else {
                                    state.log("Your task: find and defeat the boss.", "#666");
                                }
                                if let Some(bytes) = font_bytes {
                                    match load_ttf_font_from_bytes(&bytes) {
                                        Ok(f) => title_font = Some(f),
                                        Err(e) => eprintln!("Font load error: {}", e),
                                    }
                                } else {
                                    title_font = None;
                                }
                                if let Some(s) = &sfx {
                                    s.start_boss_drone(state.level.scale_at(state.player.x, state.player.y));
                                    let openness = game::measure_openness(&state.level, state.player.x, state.player.y);
                                    s.update_room_acoustics(openness);
                                }
                                screen = Screen::Playing;
                            }
                            GenMsg::Error(e) => {
                                phase_text = format!("Error: {}", e);
                                phase_detail = "Press ENTER to retry".into();
                            }
                            _ => {}
                        }
                    }
                }

                if phase_detail == "Press ENTER to retry" && is_key_pressed(KeyCode::Enter) {
                    if let Some(ow) = &overworld {
                        start_level_generation(&state, ow, &level_designs, &mut gen_rx, current_campaign_settings.clone(), ghost_town, current_campaign_idx as i32, current_campaign_monsters.clone(), &pack_item_sprites);
                        phase_text = "creating universe".into();
                        phase_detail.clear();
                    }
                }
            }

            Screen::Teleport => {
                let dt = get_frame_time();
                let anim_speed = 3.0;

                // Phase 0: Zoom out from gameplay view
                // Phase 1: Browse — pan/zoom, pick destination
                // Phase 2: Pan to destination (while zoomed out)
                // Phase 3: Zoom in at destination (or back to origin on cancel)
                match teleport_phase {
                    0 => {
                        teleport_zoom += (teleport_target_zoom - teleport_zoom) * (anim_speed * dt).min(1.0);
                        if (teleport_zoom - teleport_target_zoom).abs() < 0.01 {
                            teleport_zoom = teleport_target_zoom;
                            teleport_phase = 1;
                        }
                    }
                    2 => {
                        // Move player to dest immediately, adjust offset so view doesn't jump
                        if let Some((dx, dy)) = teleport_dest.take() {
                            let old_px = state.player.x as f32;
                            let old_py = state.player.y as f32;
                            state.player.x = dx;
                            state.player.y = dy;
                            // Camera = player + 0.5 - offset, keep camera unchanged:
                            // old_player - old_offset = new_player - new_offset
                            // new_offset = old_offset + (new_player - old_player)
                            teleport_cam_offset.0 += dx as f32 - old_px;
                            teleport_cam_offset.1 += dy as f32 - old_py;
                            teleport_phase = 4; // animate offset to 0
                        }
                    }
                    4 => {
                        // Animate offset back to (0,0) — centers on destination
                        let pan_speed = 5.0 * dt;
                        teleport_cam_offset.0 += (0.0 - teleport_cam_offset.0) * pan_speed.min(1.0);
                        teleport_cam_offset.1 += (0.0 - teleport_cam_offset.1) * pan_speed.min(1.0);
                        if teleport_cam_offset.0.abs() < 0.5 && teleport_cam_offset.1.abs() < 0.5 {
                            teleport_cam_offset = (0.0, 0.0);
                            teleport_phase = 3; // zoom in
                        }
                    }
                    3 => {
                        // Zoom in at current position (destination or original)
                        teleport_zoom += (1.0 - teleport_zoom) * (anim_speed * dt).min(1.0);
                        if (teleport_zoom - 1.0).abs() < 0.02 {
                            teleport_zoom = 1.0;
                            teleport_cam_offset = (0.0, 0.0);
                            if !teleport_is_cancel {
                                let (px, py) = (state.player.x, state.player.y);
                                state.level.visible.clear();
                                crate::game::reveal_around(&mut state.level, px, py, state.vision_radius);
                                state.log(&format!("Teleported to ({}, {})", px, py), "#ffcc00");
                                if let Some(s) = &sfx { s.confirm(); }
                            }
                            state.vision_radius = 12;
                            teleport_dest = None;
                            screen = Screen::Playing;
                        }
                    }
                    _ => {} // phase 1 handled below
                }

                render_game(&state, &ui_font, title_font.as_ref(), &tile_textures, &monster_textures, &item_textures, teleport_zoom, teleport_cam_offset);

                // Camera math for overlay (must match render_game)
                let tz = TILE * teleport_zoom;
                let sw = screen_width();
                let sh = screen_height();
                let top_height = 70.0_f32;
                let bottom_height = 28.0_f32;
                let log_width = 320.0_f32;
                let mid_top = top_height;
                let mid_height = sh - top_height - bottom_height;
                let map_width = sw - log_width;
                let cam_fx = state.player.x as f32 + 0.5 - teleport_cam_offset.0 - (map_width / tz) / 2.0;
                let cam_fy = state.player.y as f32 + 0.5 - teleport_cam_offset.1 - (mid_height / tz) / 2.0;
                let camera_x = cam_fx.floor() as i32 - 1;
                let camera_y = cam_fy.floor() as i32 - 1;
                let sub_x = -(cam_fx - camera_x as f32) * tz;
                let sub_y = -(cam_fy - camera_y as f32) * tz;
                let map_left = sub_x;
                let mid_top_adj = mid_top + sub_y;
                let (mouse_x, mouse_y) = mouse_position();

                if teleport_phase == 1 {
                    // xyzzy cheat: unlock fog reveal + right-click teleport
                    let cheat_keys_tp = [
                        (KeyCode::X, 'x'), (KeyCode::Y, 'y'), (KeyCode::Z, 'z'),
                    ];
                    for &(kc, ch) in &cheat_keys_tp {
                        if is_key_pressed(kc) { cheat_buf.push(ch); }
                    }
                    if cheat_buf.len() > 10 { cheat_buf.drain(..cheat_buf.len() - 10); }
                    {
                        let buf_str: String = cheat_buf.iter().collect();
                        if buf_str.contains("xyzzy") {
                            cheat_buf.clear();
                            teleport_cheat = true;
                            if let Some(s) = &sfx { s.cheat_fanfare(); }
                            state.vision_radius = 50;
                            for y in 0..state.level.height {
                                for x in 0..state.level.width {
                                    state.level.revealed.insert((x, y));
                                }
                            }
                        }
                    }

                    // Hover highlight
                    let hover_tx = camera_x + ((mouse_x - map_left) / tz) as i32;
                    let hover_ty = camera_y + ((mouse_y - mid_top_adj) / tz) as i32;
                    if hover_tx >= 0 && hover_ty >= 0 && hover_tx < state.level.width && hover_ty < state.level.height {
                        teleport_hover_tile = (hover_tx, hover_ty);
                        if teleport_cheat {
                            let hx = map_left + (hover_tx - camera_x) as f32 * tz;
                            let hy = mid_top_adj + (hover_ty - camera_y) as f32 * tz;
                            draw_rectangle(hx, hy, tz, tz, Color::new(0.4, 1.0, 0.4, 0.3));
                            draw_rectangle_lines(hx, hy, tz, tz, 2.0, Color::new(0.4, 1.0, 0.4, 0.8));
                        }
                    }
                    let status = if teleport_cheat {
                        format!("XYZZY  ({}, {})  zoom:{:.0}%  Drag=pan  RClick=teleport  Scroll=zoom  Shift+Tab=exit",
                            hover_tx, hover_ty, teleport_zoom * 100.0)
                    } else {
                        format!("MAP  zoom:{:.0}%  Drag=pan  Scroll=zoom  Shift+Tab=exit",
                            teleport_zoom * 100.0)
                    };
                    draw_text(&status, 10.0, sh - 34.0, 16.0, YELLOW);

                    // Scroll wheel zoom
                    let (_sx, scroll_y) = mouse_wheel();
                    if scroll_y != 0.0 {
                        let zoom_factor = if scroll_y > 0.0 { 1.15 } else { 1.0 / 1.15 };
                        teleport_zoom = (teleport_zoom * zoom_factor).clamp(0.05, 1.5);
                    }

                    // Left-click drag to pan
                    if is_mouse_button_pressed(MouseButton::Left) {
                        teleport_last_mouse = Some((mouse_x, mouse_y));
                    }
                    if is_mouse_button_down(MouseButton::Left) {
                        if let Some((ref mut lx, ref mut ly)) = teleport_last_mouse {
                            let dx = mouse_x - *lx;
                            let dy = mouse_y - *ly;
                            teleport_cam_offset.0 += dx / tz;
                            teleport_cam_offset.1 += dy / tz;
                            *lx = mouse_x;
                            *ly = mouse_y;
                        }
                    }
                    if is_mouse_button_released(MouseButton::Left) {
                        teleport_last_mouse = None;
                    }

                    // Right-click teleport (cheat only)
                    if teleport_cheat && is_mouse_button_pressed(MouseButton::Right) {
                        let (tx, ty) = teleport_hover_tile;
                        if tx >= 0 && ty >= 0 && tx < state.level.width && ty < state.level.height {
                            teleport_dest = Some((tx, ty));
                            teleport_is_cancel = false;
                            teleport_phase = 2;
                        }
                    }

                    // Shift+Tab: exit map view
                    if is_key_pressed(KeyCode::Tab) && (is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)) {
                        teleport_dest = Some((state.player.x, state.player.y));
                        teleport_is_cancel = true;
                        teleport_phase = 2;
                    }
                }
            }

            Screen::Playing => {
                // Shift+Tab: map view (cinematic zoom-out, only revealed tiles)
                if is_key_pressed(KeyCode::Tab) && (is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)) {
                    teleport_zoom = 1.0;
                    teleport_target_zoom = 0.2;
                    teleport_cam_offset = (0.0, 0.0);
                    teleport_dest = None;
                    teleport_cheat = false;
                    teleport_phase = 0;
                    screen = Screen::Teleport;
                }

                if ghost_town && (is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Q)) {
                    if let Some(s) = &sfx { s.stop_boss_drone(); }
                    screen = Screen::Playing;
                }
                if active_signpost.is_some() {
                    if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Space) {
                        active_signpost = None;
                    }
                } else {
                    handle_playing_input(&mut state, &mut screen, &mut confetti, &sfx,
                        &mut game_hold_time, &mut game_last_fire, &mut active_signpost);
                }
                // Update boss drone volume based on distance to nearest living boss
                if let Some(s) = &sfx {
                    let dist = state.level.monsters.iter()
                        .filter(|m| m.is_boss && m.is_alive())
                        .flat_map(|m| m.boss_body.iter().map(|&(bx, by)| {
                            let dx = state.player.x as f32 - bx as f32;
                            let dy = state.player.y as f32 - by as f32;
                            (dx * dx + dy * dy).sqrt()
                        }))
                        .fold(f32::MAX, f32::min);
                    s.update_boss_drone(dist);
                }
                // Load textures for newly dropped items (e.g. monster loot)
                for item in &state.level.items {
                    if !item_textures.contains_key(&item.name) {
                        if let Some(b64) = &item.image {
                            if let Some(tex) = decode_sprite_texture(b64) {
                                item_textures.insert(item.name.clone(), tex);
                            }
                        }
                    }
                }
                render_game(&state, &ui_font, title_font.as_ref(), &tile_textures, &monster_textures, &item_textures, current_zoom, (0.0, 0.0));

                // Signpost modal overlay
                if let Some(si) = active_signpost {
                    if let Some(sign) = state.level.signposts.get(si) {
                        // Load signpost fonts on demand
                        if let Some(ref fname) = sign.title_font {
                            if !fname.is_empty() && !signpost_fonts.contains_key(fname) {
                                if let Some(bytes) = fetch_google_font(fname) {
                                    if let Ok(f) = load_ttf_font_from_bytes(&bytes) { signpost_fonts.insert(fname.clone(), f); }
                                }
                            }
                        }
                        if let Some(ref fname) = sign.description_font {
                            if !fname.is_empty() && !signpost_fonts.contains_key(fname) {
                                if let Some(bytes) = fetch_google_font(fname) {
                                    if let Ok(f) = load_ttf_font_from_bytes(&bytes) { signpost_fonts.insert(fname.clone(), f); }
                                }
                            }
                        }
                        let sign_title_font = sign.title_font.as_ref().and_then(|f| signpost_fonts.get(f)).unwrap_or(&ui_font_bold);
                        let sign_desc_font = sign.description_font.as_ref().and_then(|f| signpost_fonts.get(f)).unwrap_or(&ui_font);

                        let sw = screen_width();
                        let sh = screen_height();
                        let title_size = 48u16;
                        let desc_size = 28u16;
                        let hint_size = 14u16;
                        let padding = 30.0f32;
                        let line_h = desc_size as f32 * 1.4;
                        let max_w = (sw * 0.6).min(600.0);

                        // Measure title
                        let tp = measure_text(&sign.title, Some(sign_title_font), title_size, 1.0);

                        // Word-wrap description and measure
                        let mut lines: Vec<String> = Vec::new();
                        for paragraph in sign.description.split('\n') {
                            let words: Vec<&str> = paragraph.split_whitespace().collect();
                            let mut cur = String::new();
                            for w in &words {
                                let test = if cur.is_empty() { w.to_string() } else { format!("{} {}", cur, w) };
                                if measure_text(&test, Some(sign_desc_font), desc_size, 1.0).width > max_w && !cur.is_empty() {
                                    lines.push(cur);
                                    cur = w.to_string();
                                } else {
                                    cur = test;
                                }
                            }
                            if !cur.is_empty() { lines.push(cur); }
                        }
                        let mut max_line_w = tp.width;
                        for line in &lines {
                            let lw = measure_text(line, Some(sign_desc_font), desc_size, 1.0).width;
                            if lw > max_line_w { max_line_w = lw; }
                        }

                        let hint = "Press ENTER to dismiss";

                        // Compute panel size to fit content
                        let pw = (max_line_w + padding * 2.0).max(200.0).min(sw * 0.8);
                        let text_h = title_size as f32 + 20.0 + lines.len() as f32 * line_h + 30.0 + hint_size as f32;
                        let ph = text_h + padding * 2.0;
                        let px = (sw - pw) / 2.0;
                        let py = (sh - ph) / 2.0;

                        // Draw
                        draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.6));
                        draw_rectangle(px, py, pw, ph, Color::new(0.15, 0.12, 0.08, 0.95));
                        draw_rectangle_lines(px, py, pw, ph, 2.0, Color::new(0.6, 0.5, 0.3, 1.0));

                        // Title — centered
                        draw_text_ex(&sign.title, px + (pw - tp.width) / 2.0, py + padding + title_size as f32,
                            TextParams { font: Some(sign_title_font), font_size: title_size, color: Color::new(0.9, 0.8, 0.6, 1.0), ..Default::default() });

                        // Description — centered lines
                        let mut dy = py + padding + title_size as f32 + 20.0 + desc_size as f32;
                        for line in &lines {
                            let lw = measure_text(line, Some(sign_desc_font), desc_size, 1.0).width;
                            draw_text_ex(line, px + (pw - lw) / 2.0, dy,
                                TextParams { font: Some(sign_desc_font), font_size: desc_size, color: WHITE, ..Default::default() });
                            dy += line_h;
                        }

                        // Dismiss hint — centered
                        let hp = measure_text(hint, Some(&ui_font), hint_size, 1.0);
                        draw_text_ex(hint, px + (pw - hp.width) / 2.0, py + ph - padding * 0.5,
                            TextParams { font: Some(&ui_font), font_size: hint_size, color: Color::new(0.5, 0.5, 0.5, 1.0), ..Default::default() });
                    }
                }
            }

            Screen::Dead => {
                render_game(&state, &ui_font, title_font.as_ref(), &tile_textures, &monster_textures, &item_textures, current_zoom, (0.0, 0.0));
                draw_death_overlay(&ui_font, &ui_font_bold, &state);

                if is_key_pressed(KeyCode::Enter) {
                    let death_x = state.player.x;
                    let death_y = state.player.y;

                    if let Some(snap) = &player_snapshot {
                        state.player = snap.clone();
                    }
                    state.game_over = false;
                    state.victory = false;
                    state.log.clear();

                    // Unified map: respawn at the entry of the region where player died
                    if !state.level.region_scales.is_empty() {
                        // Find which region the player died in
                        let mut respawn_x = state.player.x;
                        let mut respawn_y = state.player.y;
                        for &(ox, oy, w, h, _) in &state.level.region_scales {
                            if death_x >= ox && death_x < ox + w && death_y >= oy && death_y < oy + h {
                                // Respawn at region entry (top-left walkable tile)
                                respawn_x = ox + 2;
                                respawn_y = oy + 2;
                                break;
                            }
                        }
                        state.player.x = respawn_x;
                        state.player.y = respawn_y;
                        state.player.hp = state.player.max_hp;
                        // Re-reveal around new position
                        state.level.visible.clear();
                        let vis = state.vision_radius;
                        crate::game::reveal_around(&mut state.level, respawn_x, respawn_y, vis);
                        screen = Screen::Playing;
                    } else {
                        screen = Screen::Playing;
                    }
                }
            }

            Screen::Victory => {
                render_game(&state, &ui_font, title_font.as_ref(), &tile_textures, &monster_textures, &item_textures, current_zoom, (0.0, 0.0));
                update_confetti(&mut confetti);
                draw_confetti(&confetti);
                draw_victory_overlay(&ui_font, &ui_font_bold, &state);

                if is_key_pressed(KeyCode::Enter) {
                    if let Some(ow) = &mut overworld {
                        let completed_node = ow.current_node;
                        ow.nodes[completed_node].completed = true;

                        // Unlock connected nodes
                        for &(a, b) in &ow.connections.clone() {
                            if a == completed_node {
                                ow.nodes[b].unlocked = true;
                            } else if b == completed_node {
                                ow.nodes[a].unlocked = true;
                            }
                        }

                        // Clear level snapshot (completed, no retry needed)
                        level_snapshot = None;

                        // Between-level transition: keep everything, restore HP
                        state.player.hp = state.player.max_hp;
                        state.player.speed_turns = 0;
                        state.victory = false;
                        state.game_over = false;
                        state.log.clear();
                        confetti.clear();

                        // Beating the final level wins the game
                        if ow.nodes[completed_node].is_final {
                            if !cheat_active {
                                if let Some(id) = &current_campaign_id {
                                    save_completion(&state.player, id);
                                    save_player(&state.player);
                                    mark_campaign_played(id);
                                    eprintln!("Campaign \"{}\" marked as played", ow.name);
                                }
                                clear_campaign_progress();
                            }
                            spawn_confetti(&mut confetti);
                            screen = Screen::GameWon;
                        } else {
                            // Save progress after completing a level
                            if !cheat_active {
                                if let Some(id) = &current_campaign_id {
                                    save_campaign_progress(id, &state.player, ow);
                                }
                            }
                            screen = Screen::Playing;
                        }
                    }
                }
            }

            Screen::Store => {
                if let Some(ow) = &overworld {
                    draw_store_screen(&ui_font, &ui_font_bold, &state, &ow.store_stock, store_selection);
                }

                if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
                    if store_selection > 0 { store_selection -= 1; }
                }
                if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
                    let stock_len = overworld.as_ref().map_or(0, |ow| ow.store_stock.len());
                    if store_selection < stock_len.saturating_sub(1) { store_selection += 1; }
                }
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                    if let Some(ow) = &mut overworld {
                        if let Some(item) = ow.store_stock.get_mut(store_selection) {
                            let at_cap = item.item_type == "potion_cap" && state.player.potion_cap >= 30;
                            if item.stock > 0 && state.player.gold >= item.price && !at_cap {
                                state.player.gold -= item.price;
                                item.stock -= 1;
                                match item.item_type.as_str() {
                                    "potion" => state.player.potions = (state.player.potions + 1).min(state.player.potion_cap),
                                    "speed_potion" => state.player.speed_potions += 1,
                                    "bomb" => state.player.bombs += 1,
                                    "max_hp" => {
                                        let v = item.value;
                                        state.player.max_hp += v;
                                        state.player.hp += v;
                                    }
                                    "potion_cap" => {
                                        let v = item.value;
                                        state.player.potion_cap = (state.player.potion_cap + v).min(30);
                                    }
                                    "antidote" => {
                                        state.player.antidotes = (state.player.antidotes + 1).min(3);
                                    }
                                    _ => {}
                                }
                                if let Some(s) = &sfx { s.confirm(); }
                            }
                        }
                    }
                }
                if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Q) {
                    screen = Screen::Playing;
                }
            }

            Screen::GameWon => {
                update_confetti(&mut confetti);
                draw_confetti(&confetti);
                draw_game_won_overlay(&ui_font, &ui_font_bold, &state, overworld_font.as_ref(), &overworld, &pack_strings);

                // Continuously spawn confetti
                if confetti.len() < 200 {
                    let mut rng = ::rand::thread_rng();
                    if rng.gen::<f32>() < 0.3 {
                        spawn_confetti(&mut confetti);
                    }
                }

                if is_key_pressed(KeyCode::Enter) {
                    // Full reset — preserve persistent player progression
                    state = GameState::new();
                    state.item_sprites = pack_item_sprites.clone();
                    if let Some(psave) = load_player_save() {
                        apply_player_save(&mut state.player, &psave);
                    }
                    current_campaign_id = None;
                    overworld = None;
                    level_designs.clear();
                    design_token_flashes.clear();
                    bg_gen_rx = None;
                    player_snapshot = None;
                    confetti.clear();
                    overworld_font = None;
                    title_font = None;
                    campaign_select_idx = pick_next_campaign(&bundled_campaigns).unwrap_or(0);
                    screen = if has_bundled { Screen::CampaignSelect } else { Screen::Start };
                }
            }
        }

        next_frame().await;
    }
}

fn handle_playing_input(
    state: &mut GameState,
    screen: &mut Screen,
    confetti: &mut Vec<Confetti>,
    sfx: &Option<sfx::Sfx>,
    hold_time: &mut f64,
    last_fire: &mut f64,
    active_signpost: &mut Option<usize>,
) {
    let sc = state.level.scale_at(state.player.x, state.player.y).to_vec();
    if is_key_pressed(KeyCode::C) {
        use_potion(state);
        if let Some(s) = sfx { s.pickup_potion(&sc); }
    }
    if is_key_pressed(KeyCode::X) {
        if use_bomb(state) {
            if let Some(s) = sfx { s.bomb(&sc); }
        }
    }
    if is_key_pressed(KeyCode::Z) {
        use_speed_potion(state);
    }

    let mut dx = 0i32;
    let mut dy = 0i32;
    if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) { dy = -1; }
    if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) { dy = 1; }
    if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) { dx = -1; }
    if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) { dx = 1; }

    let now = get_time();
    let fire = if dx != 0 || dy != 0 {
        if *hold_time == 0.0 {
            *hold_time = now;
            *last_fire = now;
            true
        } else if now - *hold_time >= GAME_INITIAL_DELAY && now - *last_fire >= GAME_REPEAT_RATE {
            *last_fire = now;
            true
        } else {
            false
        }
    } else {
        *hold_time = 0.0;
        false
    };

    if fire {
        // Update facing direction
        state.player.facing = (dy as f32).atan2(dx as f32);

        let log_before = state.log.len();
        let gold_before = state.player.gold;
        let potions_before = state.player.potions;
        let weapon_before = state.player.weapon.clone();
        let armor_before = state.player.armor.clone();
        let keys_before = state.player.keys;

        let result = try_move(state, dx, dy);
        let moved = result["moved"].as_bool().unwrap_or(false);
        let combat = result["combat"].as_bool().unwrap_or(false);
        if result["store"].as_bool().unwrap_or(false) {
            if let Some(s) = sfx { s.confirm(); }
            *screen = Screen::Store;
            return;
        }
        if let Some(sign_idx) = result["signpost"].as_u64() {
            *active_signpost = Some(sign_idx as usize);
        }
        if moved || combat {
            if state.player.speed_turns > 0 {
                state.player.speed_turns -= 1;
                if state.player.speed_turns == 0 {
                    state.log("Speed wears off.", "#888");
                }
            } else {
                monster_turns(state);
            }
        }

        // Trigger sounds based on what happened
        if let Some(s) = sfx {
            if moved {
                // Update reverb based on how open the space is around the player
                let openness = game::measure_openness(&state.level, state.player.x, state.player.y);
                s.update_room_acoustics(openness);
            }
            if moved && !combat {
                s.footstep(&sc);
            }
            // Scan new log entries for combat/pickup events
            for entry in &state.log[log_before..] {
                let t = &entry.text;
                if t.contains("CRITICAL") { s.crit(&sc); }
                else if t.contains("You hit") { s.hit(&sc); }
                else if t.contains("You miss") { s.miss(&sc); }
                else if t.contains("hits you") || t.contains("CRITS you") { s.player_hurt(&sc); }
                else if t.contains("TRAP!") { s.trap(&sc); }
                else if t.contains("THE BOSS IS SLAIN") { s.boss_kill(&sc); }
                else if t.contains("exit door has opened") { s.door_unlock(&sc); }
                else if t.contains("gate is sealed") { s.hit(&sc); }
                else if t.contains("defeated the") { s.kill(&sc); }
                else if t.contains("LEVEL UP") { s.level_up(&sc); }
            }
            if state.player.gold > gold_before { s.pickup_gold(&sc); }
            if state.player.potions > potions_before { s.pickup_potion(&sc); }
            if state.player.weapon != weapon_before { s.pickup_weapon(&sc); }
            if state.player.armor != armor_before { s.pickup_armor(&sc); }
            if state.player.keys > keys_before { s.pickup_key(&sc); }
        }

        if state.victory {
            spawn_confetti(confetti);
            if let Some(s) = sfx { s.stop_boss_drone(); s.victory(&sc); }
            *screen = Screen::Victory;
        } else if state.game_over {
            if let Some(s) = sfx { s.stop_boss_drone(); s.death(&sc); }
            *screen = Screen::Dead;
        }
    }
}

/// Get held direction from arrow/WASD keys (for overworld key repeat)
fn get_held_direction() -> (f32, f32) {
    let mut dx = 0.0f32;
    let mut dy = 0.0f32;
    if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) { dx = -1.0; }
    if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) { dx = 1.0; }
    if is_key_down(KeyCode::Up) || is_key_down(KeyCode::W) { dy = -1.0; }
    if is_key_down(KeyCode::Down) || is_key_down(KeyCode::S) { dy = 1.0; }
    (dx, dy)
}

// ── Generation ──

fn start_overworld_generation(gen_rx: &mut Option<mpsc::Receiver<GenMsg>>) {
    let (tx, rx) = mpsc::channel();
    *gen_rx = Some(rx);

    std::thread::spawn(move || {
        let max_retries = 3;
        for attempt in 1..=max_retries {
            let tx_tok = tx.clone();
            match gen::generate_overworld(
                |phase| { let _ = tx.send(GenMsg::Phase(phase.phase, phase.detail)); },
                move || { let _ = tx_tok.send(GenMsg::Token); },
            ) {
                Ok(ow) => {
                    let font_bytes = fetch_google_font(&ow.font);
                    let _ = tx.send(GenMsg::OverworldReady(ow, font_bytes));
                    return;
                }
                Err(e) => {
                    eprintln!("Overworld attempt {}/{} failed: {}", attempt, max_retries, e);
                    if attempt == max_retries {
                        let _ = tx.send(GenMsg::Error(e));
                    } else {
                        let _ = tx.send(GenMsg::Phase("retrying overworld".into(), format!("attempt {}/{}", attempt + 1, max_retries)));
                    }
                }
            }
        }
    });
}

fn start_background_designs(
    ow: &Overworld,
    bg_rx: &mut Option<mpsc::Receiver<GenMsg>>,
) {
    let (tx, rx) = mpsc::channel();
    *bg_rx = Some(rx);

    // Collect playable Level node configs (skip Start and Store)
    let configs: Vec<(usize, gen::LevelConfig)> = ow.nodes.iter().enumerate()
        .filter(|(_, n)| !n.completed && n.node_type == NodeType::Level)
        .enumerate()
        .map(|(design_idx, (node_idx, node))| {
            (design_idx, gen::LevelConfig {
                title: node.name.clone(),
                font: node.font.clone(),
                description: node.description.clone(),
                theme: node.theme.clone(),
                palette: node.palette.clone(),
                budget: node.budget,
                floor: node_idx as i32,
                campaign_tier: 0,
            })
        })
        .collect();

    let campaign_name = ow.name.clone();
    let campaign_desc = ow.description.clone();

    std::thread::spawn(move || {
        let api_key = gen::llm_api_key();
        let model = gen::llm_model();
        let client = reqwest::blocking::Client::new();

        for (design_idx, config) in &configs {
            let prompt = gen::build_single_level_design_prompt(
                &campaign_name, &campaign_desc, config,
            );
            let tx_tok = tx.clone();
            let di = *design_idx;
            match gen::call_llm_for_design(&client, &api_key, &model, &prompt,
                Some(move || { let _ = tx_tok.send(GenMsg::DesignToken(di)); }),
            ) {
                Ok(design) => {
                    eprintln!("Design ready: {} — boss '{}'", config.title, design.boss.name);
                    let _ = tx.send(GenMsg::LevelDesignReady(*design_idx, design));
                }
                Err(e) => {
                    eprintln!("Design error for '{}': {}", config.title, e);
                }
            }
        }
    });
}

fn start_level_generation(
    state: &GameState,
    ow: &Overworld,
    designs: &[Option<gen::Phase2Result>],
    gen_rx: &mut Option<mpsc::Receiver<GenMsg>>,
    campaign_settings: gen::CampaignSettings,
    ghost: bool,
    campaign_tier: i32,
    campaign_monsters: Option<Vec<gen::MonsterTemplateRaw>>,
    pack_item_sprites: &std::collections::HashMap<String, String>,
) {
    let (tx, rx) = mpsc::channel();
    *gen_rx = Some(rx);
    let node = &ow.nodes[ow.current_node];
    let node_idx = ow.current_node;
    let config = gen::LevelConfig {
        title: node.name.clone(),
        font: node.font.clone(),
        description: node.description.clone(),
        theme: node.theme.clone(),
        palette: node.palette.clone(),
        budget: node.budget,
        floor: ow.current_node as i32 + 1,
        campaign_tier,
    };

    // Design index = count of Level nodes before this one (skip Start/Store)
    let design_idx = ow.nodes[..node_idx].iter()
        .filter(|n| n.node_type == NodeType::Level)
        .count();
    if let Some(Some(design)) = designs.get(design_idx) {
        // Use pre-generated design — no LLM call needed
        let design = design.clone();
        let cm = campaign_monsters.clone();
        let sprites = pack_item_sprites.clone();
        std::thread::spawn(move || {
            match gen::build_level_from_design_with_settings(&config, &design, &campaign_settings, cm.as_deref(), &sprites) {
                Ok((level, start, _remaining)) => {
                    let font_bytes = fetch_google_font(&level.font);
                    let _ = tx.send(GenMsg::LevelDone(level, start, font_bytes));
                }
                Err(e) => {
                    let _ = tx.send(GenMsg::Error(e));
                }
            }
        });
    } else {
        // Fallback: generate on the fly (shouldn't happen normally)
        let player = state.player.clone();
        std::thread::spawn(move || {
            let tx2 = tx.clone();
            match gen::generate_level(&config, &player,
                |phase| { let _ = tx.send(GenMsg::Phase(phase.phase, phase.detail)); },
                move || { let _ = tx2.send(GenMsg::Token); },
            ) {
                Ok((level, start, _remaining)) => {
                    let font_bytes = fetch_google_font(&level.font);
                    let _ = tx.send(GenMsg::LevelDone(level, start, font_bytes));
                }
                Err(e) => {
                    let _ = tx.send(GenMsg::Error(e));
                }
            }
        });
    }
}

/// Fetch a Google Font TTF at runtime. Returns None on any failure.
fn fetch_google_font(font_name: &str) -> Option<Vec<u8>> {
    if font_name.is_empty() {
        return None;
    }

    // Try embedded fonts first (offline/batteries-included)
    if let Some(bytes) = fonts::lookup_embedded_font(font_name) {
        eprintln!("Loaded embedded font '{}' ({} bytes)", font_name, bytes.len());
        return Some(bytes.to_vec());
    }

    // Fall back to network fetch
    let client = reqwest::blocking::Client::new();
    let css_url = format!(
        "https://fonts.googleapis.com/css2?family={}&display=swap",
        font_name.replace(' ', "+")
    );
    let css = client
        .get(&css_url)
        .header("User-Agent", "Mozilla/4.0")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .ok()?
        .text()
        .ok()?;

    let url_start = css.find("url(")? + 4;
    let url_end = css[url_start..].find(')')? + url_start;
    let ttf_url = &css[url_start..url_end];

    let bytes = client
        .get(ttf_url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .ok()?
        .bytes()
        .ok()?;

    eprintln!("Loaded font '{}' ({} bytes)", font_name, bytes.len());
    Some(bytes.to_vec())
}

fn validate_api_key(key: &str) -> Result<(), String> {
    let client = reqwest::blocking::Client::new();
    let resp = client.get("https://openrouter.ai/api/v1/auth/key")
        .header("Authorization", format!("Bearer {}", key))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .map_err(|e| format!("Could not reach OpenRouter: {}", e))?;

    if resp.status().is_success() {
        Ok(())
    } else if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 {
        Err("Key not recognized.".into())
    } else {
        Err(format!("OpenRouter error ({})", resp.status()))
    }
}

fn validate_ollama_url(url: &str) -> Result<(), String> {
    let client = reqwest::blocking::Client::new();
    let models_url = format!("{}/models", url.trim_end_matches('/'));
    let resp = client.get(&models_url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .map_err(|e| format!("Could not reach {}: {}", url, e))?;

    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("Server responded with status {}", resp.status()))
    }
}

// ── Screens ──

fn draw_key_entry_screen(font: &Font, bold: &Font, input: &str, error: &Option<String>, validating: bool) {
    let sw = screen_width();
    let sh = screen_height();

    // Title
    let title = "SCAPEGRACE";
    let ts = 52u16;
    let tw = measure_text(title, Some(bold), ts, 1.0).width;
    draw_text_ex(title, (sw - tw) / 2.0, sh * 0.28, TextParams {
        font: Some(bold), font_size: ts, color: hex_to_color("#e94560"), ..Default::default()
    });

    // Flavor text
    let lines = [
        "You were warned.",
        "You summoned us anyway.",
        "",
        "Speak the passphrase, and the gate opens.",
    ];
    let ls = 17u16;
    let mut y = sh * 0.40;
    for line in &lines {
        if line.is_empty() { y += 10.0; continue; }
        let lw = measure_text(line, Some(font), ls, 1.0).width;
        draw_text_ex(line, (sw - lw) / 2.0, y, TextParams {
            font: Some(font), font_size: ls, color: Color::new(0.55, 0.55, 0.55, 1.0), ..Default::default()
        });
        y += 24.0;
    }

    // Input field
    let field_w = 440.0;
    let field_h = 36.0;
    let field_x = (sw - field_w) / 2.0;
    let field_y = sh * 0.58;
    let border_color = if error.is_some() { hex_to_color("#e94560") } else { Color::new(0.3, 0.3, 0.3, 1.0) };
    draw_rectangle(field_x, field_y, field_w, field_h, Color::new(0.08, 0.08, 0.08, 1.0));
    draw_rectangle_lines(field_x, field_y, field_w, field_h, 1.5, border_color);

    let fs = 16u16;
    let max_inner = field_w - 24.0; // padding on both sides
    if input.is_empty() {
        draw_text_ex("key or hostname...", field_x + 12.0, field_y + 24.0, TextParams {
            font: Some(font), font_size: fs, color: Color::new(0.3, 0.3, 0.3, 1.0), ..Default::default()
        });
    } else {
        let display = if is_ollama_input(input) {
            // Show URL in full
            input.to_string()
        } else {
            // Mask API key: ••••••••last4
            let tail: String = input.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
            let tail_w = measure_text(&tail, Some(font), fs, 1.0).width;
            let dot = "\u{2022}";
            let dot_w = measure_text(dot, Some(font), fs, 1.0).width;
            let available = max_inner - tail_w;
            let dot_count = if input.len() > 4 {
                ((available / dot_w) as usize).min(input.len() - 4)
            } else {
                0
            };
            format!("{}{}", dot.repeat(dot_count), tail)
        };
        draw_text_ex(&display, field_x + 12.0, field_y + 24.0, TextParams {
            font: Some(font), font_size: fs, color: hex_to_color("#e0d5c0"), ..Default::default()
        });
    }

    // Blinking cursor — always at the right edge of displayed text
    let cursor_x = if input.is_empty() {
        field_x + 12.0
    } else {
        let tail: String = input.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
        let tail_w = measure_text(&tail, Some(font), fs, 1.0).width;
        let dot = "\u{2022}";
        let dot_w = measure_text(dot, Some(font), fs, 1.0).width;
        let available = max_inner - tail_w;
        let dot_count = if input.len() > 4 {
            ((available / dot_w) as usize).min(input.len() - 4)
        } else {
            0
        };
        field_x + 12.0 + dot_count as f32 * dot_w + tail_w
    };
    if (get_time() * 2.0) as i32 % 2 == 0 {
        draw_line(cursor_x, field_y + 8.0, cursor_x, field_y + field_h - 8.0, 1.5, hex_to_color("#e94560"));
    }

    // Submit hint or status
    let hs = 14u16;
    if validating {
        let hint = "Verifying...";
        let hw = measure_text(hint, Some(font), hs, 1.0).width;
        draw_text_ex(hint, (sw - hw) / 2.0, field_y + field_h + 30.0, TextParams {
            font: Some(font), font_size: hs, color: GRAY, ..Default::default()
        });
    } else {
        let hint = "Press ENTER to begin";
        let hw = measure_text(hint, Some(font), hs, 1.0).width;
        draw_text_ex(hint, (sw - hw) / 2.0, field_y + field_h + 30.0, TextParams {
            font: Some(font), font_size: hs, color: DARKGRAY, ..Default::default()
        });
    }

    if let Some(err) = error {
        let es = 14u16;
        let ew = measure_text(err, Some(font), es, 1.0).width;
        draw_text_ex(err, (sw - ew) / 2.0, field_y + field_h + 55.0, TextParams {
            font: Some(font), font_size: es, color: hex_to_color("#e94560"), ..Default::default()
        });
    }
}

fn draw_start_screen(font: &Font, bold: &Font) {
    let sw = screen_width();
    let sh = screen_height();

    let title = "SCAPEGRACE";
    let title_size = 56u16;
    let tw = measure_text(title, Some(bold), title_size, 1.0).width;
    draw_text_ex(title, (sw - tw) / 2.0, sh / 2.0 - 30.0, TextParams {
        font: Some(bold), font_size: title_size, color: hex_to_color("#e94560"), ..Default::default()
    });

    let prompt = "Press ENTER to generate overworld";
    let ps = 18u16;
    let pw = measure_text(prompt, Some(font), ps, 1.0).width;
    draw_text_ex(prompt, (sw - pw) / 2.0, sh / 2.0 + 30.0, TextParams {
        font: Some(font), font_size: ps, color: GRAY, ..Default::default()
    });
}

fn draw_bundled_start_screen(font: &Font, bold: &Font, total: usize, played: &std::collections::HashSet<String>, has_save: bool, strings: &gen::PackStrings) {
    let sw = screen_width();
    let sh = screen_height();

    // Title
    let title_size = 64u16;
    let tw = measure_text(&strings.title, Some(bold), title_size, 1.0).width;
    draw_text_ex(&strings.title, (sw - tw) / 2.0, sh * 0.28, TextParams {
        font: Some(bold), font_size: title_size, color: hex_to_color("#e94560"), ..Default::default()
    });

    // Subtitle
    let sub_size = 22u16;
    let subw = measure_text(&strings.subtitle, Some(font), sub_size, 1.0).width;
    draw_text_ex(&strings.subtitle, (sw - subw) / 2.0, sh * 0.28 + 40.0, TextParams {
        font: Some(font), font_size: sub_size, color: hex_to_color("#888899"), ..Default::default()
    });

    // Intro paragraph
    let line_size = 16u16;
    let start_y = sh * 0.45;
    for (i, line) in strings.intro.iter().enumerate() {
        let lw = measure_text(line, Some(font), line_size, 1.0).width;
        draw_text_ex(line, (sw - lw) / 2.0, start_y + i as f32 * 24.0, TextParams {
            font: Some(font), font_size: line_size, color: hex_to_color("#aaaaaa"), ..Default::default()
        });
    }

    // Progress
    let played_count = played.len();
    if played_count > 0 {
        let progress = format!("{}/{} systems cleared", played_count, total);
        let ps = 14u16;
        let pw = measure_text(&progress, Some(font), ps, 1.0).width;
        draw_text_ex(&progress, (sw - pw) / 2.0, sh * 0.72, TextParams {
            font: Some(font), font_size: ps, color: hex_to_color("#66aa88"), ..Default::default()
        });
    }

    // Prompt
    let prompt = if has_save {
        &strings.prompt_resume
    } else if played_count == 0 {
        &strings.prompt_first
    } else if played_count >= total {
        &strings.prompt_restart
    } else {
        &strings.prompt_next
    };
    let ps = 18u16;
    let pw = measure_text(prompt, Some(font), ps, 1.0).width;
    let alpha = ((get_time() * 2.0).sin() * 0.3 + 0.7) as f32;
    draw_text_ex(prompt, (sw - pw) / 2.0, sh * 0.82, TextParams {
        font: Some(font), font_size: ps, color: Color::new(0.7, 0.7, 0.7, alpha), ..Default::default()
    });
}

fn campaign_card_pos(i: usize, cols: usize, card_w: f32, card_h: f32, gap: f32, grid_left: f32, grid_top: f32, row_h: f32, time: f64) -> (f32, f32) {
    let row = i / cols;
    let col_in_row = i % cols;
    // Zigzag: even rows go left-to-right, odd rows go right-to-left
    let col = if row % 2 == 0 { col_in_row } else { cols - 1 - col_in_row };
    let cx = grid_left + col as f32 * (card_w + gap);
    let cy = grid_top + row as f32 * row_h;
    // Gentle bobbing per card
    let phase = i as f64 * 1.7;
    let bob_x = (time * 0.4 + phase).sin() * 4.0;
    let bob_y = (time * 0.6 + phase * 1.3).cos() * 3.0;
    (cx + bob_x as f32, cy + bob_y as f32)
}

/// Same as campaign_card_pos but without bobbing (for scroll calculation)
fn campaign_card_pos_static(i: usize, cols: usize, card_w: f32, card_h: f32, gap: f32, grid_left: f32, grid_top: f32, row_h: f32) -> (f32, f32) {
    let row = i / cols;
    let col_in_row = i % cols;
    let col = if row % 2 == 0 { col_in_row } else { cols - 1 - col_in_row };
    let cx = grid_left + col as f32 * (card_w + gap);
    let cy = grid_top + row as f32 * row_h;
    (cx, cy)
}

fn draw_campaign_select(
    font: &Font, bold: &Font,
    campaigns: &[gen::BundledCampaign],
    completions: &[CampaignCompletion],
    played: &std::collections::HashSet<String>,
    selected: usize,
    has_save: Option<&str>,
    strings: &gen::PackStrings,
    all_unlocked: bool,
) {
    let sw = screen_width();
    let sh = screen_height();
    let time = get_time();

    // Grid layout
    let tile_px = 12.0f32;
    let card_tiles_w = 14;
    let card_tiles_h = 10;
    let card_w = card_tiles_w as f32 * tile_px;
    let card_h = card_tiles_h as f32 * tile_px;
    let gap = 40.0f32;
    let cols = 5usize;
    let total_w = cols as f32 * card_w + (cols - 1) as f32 * gap;
    let grid_left = (sw - total_w) / 2.0;
    let header_h = 120.0; // space for title+subtitle
    let footer_h = 70.0;  // space for prompt+progress
    let grid_top = header_h + 10.0;
    let row_h = card_h + 80.0; // card + name + status + spacing

    // Total content height
    let total_rows = (campaigns.len() + cols - 1) / cols;
    let content_h = total_rows as f32 * row_h;
    let viewport_h = sh - header_h - footer_h;

    // Camera: selected card's Y position determines scroll
    let (_, sel_y) = campaign_card_pos_static(selected, cols, card_w, card_h, gap, grid_left, 0.0, row_h);
    let sel_center = sel_y + card_h / 2.0;

    // Scroll offset (how far the grid content is shifted up)
    let max_scroll = (content_h - viewport_h).max(0.0);
    let scroll = if content_h <= viewport_h {
        0.0 // everything fits, no scroll
    } else {
        // Target: keep selection at center of viewport
        let target = sel_center - viewport_h / 2.0;
        target.clamp(0.0, max_scroll)
    };

    // Title (fixed, not scrolled)
    let title_size = 48u16;
    let tw = measure_text(&strings.title, Some(bold), title_size, 1.0).width;
    draw_text_ex(&strings.title, (sw - tw) / 2.0, 60.0, TextParams {
        font: Some(bold), font_size: title_size, color: hex_to_color("#e94560"), ..Default::default()
    });

    let sub_size = 16u16;
    let subw = measure_text(&strings.subtitle, Some(font), sub_size, 1.0).width;
    draw_text_ex(&strings.subtitle, (sw - subw) / 2.0, 88.0, TextParams {
        font: Some(font), font_size: sub_size, color: hex_to_color("#888899"), ..Default::default()
    });

    // Draw connecting lines between consecutive campaigns (behind cards)
    let conn_tile = 7.0f32;
    for i in 0..campaigns.len().saturating_sub(1) {
        let (ax, ay) = campaign_card_pos(i, cols, card_w, card_h, gap, grid_left, grid_top - scroll, row_h, time);
        let (bx, by) = campaign_card_pos(i + 1, cols, card_w, card_h, gap, grid_left, grid_top - scroll, row_h, time);

        let a_center_x = ax + card_w / 2.0;
        let a_center_y = ay + card_h / 2.0;
        let b_center_x = bx + card_w / 2.0;
        let b_center_y = by + card_h / 2.0;

        // Color based on whether the connection is "active"
        let a_played = played.contains(&campaigns[i].id);
        let conn_color = if a_played {
            hex_to_color("#555555")
        } else {
            hex_to_color("#383838")
        };

        // Draw tiled path between centers
        let dx = b_center_x - a_center_x;
        let dy = b_center_y - a_center_y;
        let dist = (dx * dx + dy * dy).sqrt();
        let steps = (dist / conn_tile) as i32;
        if steps > 0 {
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let px = a_center_x + dx * t;
                let py = a_center_y + dy * t;
                draw_rectangle(px - conn_tile / 2.0, py - conn_tile / 2.0, conn_tile, conn_tile, conn_color);
            }
        }
    }

    // Draw campaign cards
    for (i, campaign) in campaigns.iter().enumerate() {
        let (cx, cy) = campaign_card_pos(i, cols, card_w, card_h, gap, grid_left, grid_top - scroll, row_h, time);

        // Skip if fully off screen
        if cy + card_h + 40.0 < header_h || cy > sh - footer_h { continue; }

        let is_played = played.contains(&campaign.id);
        let is_in_progress = has_save == Some(campaign.id.as_str());
        let is_unlocked = all_unlocked || i == 0 || played.contains(&campaigns[i.saturating_sub(1)].id) || is_in_progress;
        let is_selected = i == selected;

        // Get palette from first level node
        let palette: Vec<Color> = campaign.overworld.levels.first()
            .and_then(|l| l.palette.as_ref())
            .map(|p| p.iter().map(|c| hex_to_color(c)).collect())
            .unwrap_or_else(|| vec![hex_to_color("#446688"), hex_to_color("#668844")]);

        // Seeded RNG for deterministic tile colors
        let mut seed = (i as u32).wrapping_mul(2654435761);
        let next = |s: &mut u32| -> u32 {
            *s = s.wrapping_mul(1103515245).wrapping_add(12345);
            (*s >> 16) & 0x7FFF
        };

        // Draw rectangular tile grid
        for gy in 0..card_tiles_h {
            for gx in 0..card_tiles_w {
                let ci = next(&mut seed) as usize % palette.len();
                let brightness = 0.7 + (next(&mut seed) % 300) as f32 / 1000.0;
                let mut c = palette[ci];
                c.r *= brightness;
                c.g *= brightness;
                c.b *= brightness;

                if is_played {
                    c = Color::new(c.r * 0.4, c.g * 0.4, c.b * 0.4, 1.0);
                } else if !is_unlocked {
                    c = Color::new(c.r * 0.15, c.g * 0.15, c.b * 0.15, 1.0);
                }

                let tx = cx + gx as f32 * tile_px;
                let ty = cy + gy as f32 * tile_px;
                draw_rectangle(tx, ty, tile_px, tile_px, c);
            }
        }

        // Outline
        if is_selected && is_unlocked {
            let pulse = ((time * 3.0).sin() * 0.3 + 0.7) as f32;
            let outline_color = if is_played {
                Color::new(0.4, 0.4, 0.4, pulse)
            } else {
                Color::new(0.27, 1.0, 0.27, pulse)
            };
            draw_rectangle_lines(cx - 4.0, cy - 4.0, card_w + 8.0, card_h + 8.0, 4.0, outline_color);
        } else if is_played {
            draw_rectangle_lines(cx - 1.0, cy - 1.0, card_w + 2.0, card_h + 2.0, 1.0, hex_to_color("#333333"));
        } else if !is_unlocked {
            draw_rectangle_lines(cx - 1.0, cy - 1.0, card_w + 2.0, card_h + 2.0, 1.0, hex_to_color("#2a2a2a"));
        }

        // Campaign name (floating on top of card, centered) — hidden for locked
        let ns = 14u16;
        let name_y = cy + card_h / 2.0 + 5.0;
        if is_unlocked {
            let name = &campaign.overworld.name;
            let name_w = measure_text(name, Some(font), ns, 1.0).width;
            let name_x = cx + (card_w - name_w) / 2.0;
            let name_color = if is_played {
                hex_to_color("#999999")
            } else {
                hex_to_color("#ffffff")
            };
            draw_text_ex(name, name_x + 1.0, name_y + 1.0, TextParams {
                font: Some(bold), font_size: ns, color: Color::new(0.0, 0.0, 0.0, 0.7), ..Default::default()
            });
            draw_text_ex(name, name_x, name_y, TextParams {
                font: Some(bold), font_size: ns, color: name_color, ..Default::default()
            });
        } else {
            let label = "locked";
            let label_w = measure_text(label, Some(font), ns, 1.0).width;
            let label_x = cx + (card_w - label_w) / 2.0;
            draw_text_ex(label, label_x, name_y, TextParams {
                font: Some(font), font_size: ns, color: hex_to_color("#444444"), ..Default::default()
            });
        }

        // Status line (below card)
        let status_y = cy + card_h + 16.0;
        let ss = 11u16;
        if is_played {
            if let Some(comp) = completions.iter().find(|c| c.campaign_id == campaign.id) {
                let mut parts: Vec<String> = Vec::new();
                if comp.potions > 0 { parts.push(format!("{}hp", comp.potions)); }
                if comp.speed_potions > 0 { parts.push(format!("{}spd", comp.speed_potions)); }
                if comp.bombs > 0 { parts.push(format!("{}bomb", comp.bombs)); }
                if comp.gold > 0 { parts.push(format!("{}g", comp.gold)); }
                let loot = if parts.is_empty() { "cleared".into() } else { parts.join(" ") };
                let lw = measure_text(&loot, Some(font), ss, 1.0).width;
                draw_text_ex(&loot, cx + (card_w - lw) / 2.0, status_y, TextParams {
                    font: Some(font), font_size: ss, color: hex_to_color("#66aa88"), ..Default::default()
                });
            } else {
                let t = "cleared";
                let tw = measure_text(t, Some(font), ss, 1.0).width;
                draw_text_ex(t, cx + (card_w - tw) / 2.0, status_y, TextParams {
                    font: Some(font), font_size: ss, color: hex_to_color("#66aa88"), ..Default::default()
                });
            }
        } else if is_in_progress {
            let t = "in progress";
            let tw = measure_text(t, Some(font), ss, 1.0).width;
            draw_text_ex(t, cx + (card_w - tw) / 2.0, status_y, TextParams {
                font: Some(font), font_size: ss, color: hex_to_color("#e9a845"), ..Default::default()
            });
        }
    }

    // Cover header/footer areas so scrolling content doesn't bleed through
    draw_rectangle(0.0, 0.0, sw, header_h, Color::new(0.04, 0.04, 0.04, 1.0));
    draw_rectangle(0.0, sh - footer_h, sw, footer_h, Color::new(0.04, 0.04, 0.04, 1.0));

    // Redraw title/subtitle on top of cover
    let tw = measure_text(&strings.title, Some(bold), title_size, 1.0).width;
    draw_text_ex(&strings.title, (sw - tw) / 2.0, 60.0, TextParams {
        font: Some(bold), font_size: title_size, color: hex_to_color("#e94560"), ..Default::default()
    });
    let subw = measure_text(&strings.subtitle, Some(font), sub_size, 1.0).width;
    draw_text_ex(&strings.subtitle, (sw - subw) / 2.0, 88.0, TextParams {
        font: Some(font), font_size: sub_size, color: hex_to_color("#888899"), ..Default::default()
    });

    // Bottom prompt (fixed)
    let prompt = "ENTER to embark    \u{2190}\u{2192} to browse    ESC to return";
    let ps = 14u16;
    let pw = measure_text(prompt, Some(font), ps, 1.0).width;
    let alpha = ((time * 2.0).sin() * 0.3 + 0.7) as f32;
    draw_text_ex(prompt, (sw - pw) / 2.0, sh - 30.0, TextParams {
        font: Some(font), font_size: ps, color: Color::new(0.5, 0.5, 0.5, alpha), ..Default::default()
    });

    // Progress
    let cleared = played.len();
    if cleared > 0 {
        let progress = format!("{}/{} cleared", cleared, campaigns.len());
        let ps = 13u16;
        let pw = measure_text(&progress, Some(font), ps, 1.0).width;
        draw_text_ex(&progress, (sw - pw) / 2.0, sh - 52.0, TextParams {
            font: Some(font), font_size: ps, color: hex_to_color("#66aa88"), ..Default::default()
        });
    }
}

fn draw_loading_screen(font: &Font, phase_text: &str, phase_detail: &str, tile_count: usize) {
    let sw = screen_width();
    let sh = screen_height();

    // Draw tile blob BEHIND text
    if tile_count > 0 {
        let tile_sz = 12.0;
        let grid_w = (sw / tile_sz) as i32 + 1;
        let grid_h = (sh / tile_sz) as i32 + 1;

        let mut filled: Vec<(i32, i32)> = Vec::new();
        let mut is_filled = std::collections::HashSet::new();

        // Seed: single tile behind the text center
        let cx = (sw / 2.0 / tile_sz) as i32;
        let cy = ((sh / 2.0 + 15.0) / tile_sz) as i32;
        is_filled.insert((cx, cy));
        filled.push((cx, cy));

        // Grow organically upward/outward
        let mut seed: u32 = 42;
        let next = |s: &mut u32| -> u32 {
            *s = s.wrapping_mul(1103515245).wrapping_add(12345);
            (*s >> 16) & 0x7FFF
        };
        let target = tile_count + filled.len();
        let mut attempts = 0;
        while filled.len() < target && attempts < target * 30 {
            attempts += 1;
            let base_idx = next(&mut seed) as usize % filled.len();
            let (bx, by) = filled[base_idx];
            let dir = next(&mut seed) % 4;
            let (nx, ny) = match dir {
                0 => (bx + 1, by),
                1 => (bx - 1, by),
                2 => (bx, by + 1),
                _ => (bx, by - 1),
            };
            if nx < 0 || ny < 0 || nx >= grid_w || ny >= grid_h { continue; }
            let p = (nx, ny);
            if is_filled.contains(&p) { continue; }
            is_filled.insert(p);
            filled.push(p);
        }

        // Draw: shadows first, then tiles
        let shadow_off = 3.0;
        let vibrant_colors = [
            Color::new(0.91, 0.27, 0.37, 1.0), // rose
            Color::new(0.20, 0.60, 0.85, 1.0), // blue
            Color::new(0.95, 0.65, 0.15, 1.0), // amber
            Color::new(0.30, 0.75, 0.45, 1.0), // green
            Color::new(0.70, 0.35, 0.85, 1.0), // purple
            Color::new(0.85, 0.45, 0.20, 1.0), // burnt orange
            Color::new(0.25, 0.80, 0.75, 1.0), // teal
            Color::new(0.90, 0.40, 0.60, 1.0), // pink
        ];

        // Shadows pass — only draw shadow where there's no tile behind it
        for &(tx, ty) in &filled {
            let sx = tx as f32 * tile_sz + shadow_off;
            let sy = ty as f32 * tile_sz + shadow_off;
            draw_rectangle(sx, sy, tile_sz, tile_sz, Color::new(0.0, 0.0, 0.0, 0.3));
        }

        // Tiles pass
        let mut color_seed: u32 = 7;
        for &(tx, ty) in &filled {
            let ci = next(&mut color_seed) as usize % vibrant_colors.len();
            let base = vibrant_colors[ci];
            let v = 0.7 + (next(&mut color_seed) % 60) as f32 * 0.005;
            let c = Color::new(base.r * v, base.g * v, base.b * v, 1.0);
            draw_rectangle(tx as f32 * tile_sz, ty as f32 * tile_sz, tile_sz, tile_sz, c);
        }
    }

    // Text with soft drop shadow
    let ps = 20u16;
    let ptw = measure_text(phase_text, Some(font), ps, 1.0).width;
    let tx = (sw - ptw) / 2.0;
    let ty = sh / 2.0 + 20.0;
    let shadow_layers: [(f32, f32); 4] = [(3.0, 0.35), (5.0, 0.25), (8.0, 0.15), (12.0, 0.06)];
    for &(off, alpha) in &shadow_layers {
        draw_text_ex(phase_text, tx + off, ty + off, TextParams {
            font: Some(font), font_size: ps, color: Color::new(0.0, 0.0, 0.0, alpha), ..Default::default()
        });
    }
    draw_text_ex(phase_text, tx, ty, TextParams {
        font: Some(font), font_size: ps, color: WHITE, ..Default::default()
    });

    if !phase_detail.is_empty() {
        let ds = 16u16;
        let pdw = measure_text(phase_detail, Some(font), ds, 1.0).width;
        let dx = (sw - pdw) / 2.0;
        let dy = sh / 2.0 + 48.0;
        for &(off, alpha) in &shadow_layers {
            draw_text_ex(phase_detail, dx + off, dy + off, TextParams {
                font: Some(font), font_size: ds, color: Color::new(0.0, 0.0, 0.0, alpha), ..Default::default()
            });
        }
        draw_text_ex(phase_detail, dx, dy, TextParams {
            font: Some(font), font_size: ds, color: WHITE, ..Default::default()
        });
    }
}

// ── Overworld rendering ──

fn draw_death_overlay(font: &Font, bold: &Font, state: &GameState) {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.92));

    let title = "YOU DIED";
    let ts = 52u16;
    let tw = measure_text(title, Some(bold), ts, 1.0).width;
    draw_text_ex(title, (sw - tw) / 2.0, sh / 2.0 - 20.0, TextParams {
        font: Some(bold), font_size: ts, color: hex_to_color("#ef5350"), ..Default::default()
    });

    let mut y = sh / 2.0 + 20.0;

    if !state.level.defeat_message.is_empty() {
        let ms = 18u16;
        let max_w = sw * 0.8;
        let mut lines: Vec<String> = Vec::new();
        let mut cur = String::new();
        for word in state.level.defeat_message.split_whitespace() {
            let candidate = if cur.is_empty() { word.to_string() } else { format!("{} {}", cur, word) };
            if measure_text(&candidate, Some(font), ms, 1.0).width > max_w && !cur.is_empty() {
                lines.push(cur);
                cur = word.to_string();
            } else {
                cur = candidate;
            }
        }
        if !cur.is_empty() { lines.push(cur); }
        let line_h = ms as f32 + 4.0;
        for (i, line) in lines.iter().enumerate() {
            let lw = measure_text(line, Some(font), ms, 1.0).width;
            draw_text_ex(line, (sw - lw) / 2.0, y + i as f32 * line_h, TextParams {
                font: Some(font), font_size: ms, color: GRAY, ..Default::default()
            });
        }
        y += lines.len() as f32 * (ms as f32 + 4.0) + 10.0;
    }

    let summary = format!("Level {}  {} gold", state.player.level, state.player.gold);
    let ss = 18u16;
    let smw = measure_text(&summary, Some(font), ss, 1.0).width;
    draw_text_ex(&summary, (sw - smw) / 2.0, y, TextParams {
        font: Some(font), font_size: ss, color: GRAY, ..Default::default()
    });
    y += 35.0;

    let prompt = "Press ENTER to return to overworld";
    let ps = 16u16;
    let pw = measure_text(prompt, Some(font), ps, 1.0).width;
    draw_text_ex(prompt, (sw - pw) / 2.0, y, TextParams {
        font: Some(font), font_size: ps, color: DARKGRAY, ..Default::default()
    });
}

fn draw_victory_overlay(font: &Font, bold: &Font, _state: &GameState) {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.88));

    let title = "VICTORY";
    let ts = 52u16;
    let tw = measure_text(title, Some(bold), ts, 1.0).width;
    draw_text_ex(title, (sw - tw) / 2.0, sh / 2.0 - 20.0, TextParams {
        font: Some(bold), font_size: ts, color: hex_to_color("#ffd700"), ..Default::default()
    });

    if !_state.level.victory_message.is_empty() {
        let ms = 18u16;
        let max_w = sw * 0.8;
        let mut lines: Vec<String> = Vec::new();
        let mut cur = String::new();
        for word in _state.level.victory_message.split_whitespace() {
            let candidate = if cur.is_empty() { word.to_string() } else { format!("{} {}", cur, word) };
            if measure_text(&candidate, Some(font), ms, 1.0).width > max_w && !cur.is_empty() {
                lines.push(cur);
                cur = word.to_string();
            } else {
                cur = candidate;
            }
        }
        if !cur.is_empty() { lines.push(cur); }
        let line_h = ms as f32 + 4.0;
        let start_y = sh / 2.0 + 20.0;
        for (i, line) in lines.iter().enumerate() {
            let lw = measure_text(line, Some(font), ms, 1.0).width;
            draw_text_ex(line, (sw - lw) / 2.0, start_y + i as f32 * line_h, TextParams {
                font: Some(font), font_size: ms, color: GRAY, ..Default::default()
            });
        }
    }

    let prompt = "Press ENTER to continue";
    let ps = 16u16;
    let pw = measure_text(prompt, Some(font), ps, 1.0).width;
    draw_text_ex(prompt, (sw - pw) / 2.0, sh / 2.0 + 65.0, TextParams {
        font: Some(font), font_size: ps, color: DARKGRAY, ..Default::default()
    });
}

fn draw_game_won_overlay(font: &Font, bold: &Font, state: &GameState, ow_font: Option<&Font>, ow: &Option<Overworld>, strings: &gen::PackStrings) {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.85));

    let tfont = ow_font.unwrap_or(bold);

    let ts = 64u16;
    let tw = measure_text(&strings.campaign_cleared, Some(tfont), ts, 1.0).width;
    draw_text_ex(&strings.campaign_cleared, (sw - tw) / 2.0, sh / 2.0 - 40.0, TextParams {
        font: Some(tfont), font_size: ts, color: hex_to_color("#ffd700"), ..Default::default()
    });

    if let Some(ow) = ow {
        let sub = strings.campaign_conquered.replace("{name}", &ow.name);
        let ss = 22u16;
        let sw2 = measure_text(&sub, Some(font), ss, 1.0).width;
        draw_text_ex(&sub, (sw - sw2) / 2.0, sh / 2.0 + 10.0, TextParams {
            font: Some(font), font_size: ss, color: hex_to_color("#e0d5c0"), ..Default::default()
        });
    }

    let summary = format!("Level {}  {} gold", state.player.level, state.player.gold);
    let ss = 18u16;
    let smw = measure_text(&summary, Some(font), ss, 1.0).width;
    draw_text_ex(&summary, (sw - smw) / 2.0, sh / 2.0 + 45.0, TextParams {
        font: Some(font), font_size: ss, color: GRAY, ..Default::default()
    });

    let prompt = &strings.prompt_after_clear;
    let ps = 16u16;
    let pw = measure_text(prompt, Some(font), ps, 1.0).width;
    draw_text_ex(prompt, (sw - pw) / 2.0, sh / 2.0 + 80.0, TextParams {
        font: Some(font), font_size: ps, color: DARKGRAY, ..Default::default()
    });
}

// ── Confetti ──

struct Confetti {
    x: f32,
    y: f32,
    speed: f32,
    color: Color,
    size: f32,
    rotation: f32,
    rot_speed: f32,
}

fn spawn_confetti(confetti: &mut Vec<Confetti>) {
    let colors = [
        hex_to_color("#ffd700"),
        hex_to_color("#e94560"),
        hex_to_color("#44ff44"),
        hex_to_color("#4fc3f7"),
        hex_to_color("#ff8844"),
        hex_to_color("#ab47bc"),
        WHITE,
    ];
    let mut rng = ::rand::thread_rng();
    let sw = screen_width();
    for _ in 0..80 {
        confetti.push(Confetti {
            x: rng.gen_range(0.0..sw),
            y: rng.gen_range(-50.0..-10.0),
            speed: rng.gen_range(80.0..250.0),
            color: colors[rng.gen_range(0..colors.len())],
            size: rng.gen_range(6.0..14.0),
            rotation: rng.gen_range(0.0..std::f32::consts::TAU),
            rot_speed: rng.gen_range(-4.0..4.0),
        });
    }
}

fn update_confetti(confetti: &mut Vec<Confetti>) {
    let dt = get_frame_time();
    let sh = screen_height();
    for c in confetti.iter_mut() {
        c.y += c.speed * dt;
        c.rotation += c.rot_speed * dt;
    }
    confetti.retain(|c| c.y < sh + 20.0);
}

fn draw_confetti(confetti: &[Confetti]) {
    for c in confetti {
        draw_rectangle(c.x, c.y, c.size, c.size * 0.6, c.color);
    }
}

// ── Game rendering ──

fn render_game(state: &GameState, ui_font: &Font, title_font: Option<&Font>, tile_textures: &std::collections::HashMap<String, Texture2D>, monster_textures: &std::collections::HashMap<String, Texture2D>, item_textures: &std::collections::HashMap<String, Texture2D>, zoom: f32, cam_offset: (f32, f32)) {
    if state.level.tiles.is_empty() {
        return;
    }

    let sw = screen_width();
    let sh = screen_height();

    // ── Layout constants ──
    let top_height = 70.0;
    let bottom_height = 28.0;
    let mid_top = top_height;
    let mid_height = sh - top_height - bottom_height;
    let log_width = 320.0;
    let map_width = sw - log_width;

    // ── TOP ROW: Title + description, centered ──
    draw_rectangle(0.0, 0.0, sw, top_height, Color::new(0.05, 0.05, 0.05, 1.0));
    draw_line(0.0, top_height, sw, top_height, 1.0, Color::new(0.13, 0.13, 0.13, 1.0));

    if !state.level.title.is_empty() {
        let tfont = title_font.unwrap_or(ui_font);
        let ts = 32u16;
        let tw = measure_text(&state.level.title, Some(tfont), ts, 1.0).width;
        draw_text_ex(&state.level.title, (sw - tw) / 2.0, 48.0, TextParams {
            font: Some(tfont), font_size: ts, color: hex_to_color("#e0d5c0"), ..Default::default()
        });
    }

    // ── MIDDLE ROW: Map (left) + Log (right) ──

    let map_left = 0.0;
    // Zoom: scale tile size, player always pixel-perfectly centered
    let tz = TILE * zoom;
    let tiles_x = (map_width / tz) as i32 + 4;
    let tiles_y = (mid_height / tz) as i32 + 4;
    // Float camera: player is exactly at center of map area, offset by cam_offset
    let cam_fx = state.player.x as f32 + 0.5 - cam_offset.0 - (map_width / tz) / 2.0;
    let cam_fy = state.player.y as f32 + 0.5 - cam_offset.1 - (mid_height / tz) / 2.0;
    let camera_x = cam_fx.floor() as i32 - 1;
    let camera_y = cam_fy.floor() as i32 - 1;
    // Sub-tile pixel offset for smooth centering
    let sub_x = -(cam_fx - camera_x as f32) * tz;
    let sub_y = -(cam_fy - camera_y as f32) * tz;
    // Fold sub-tile offset into map_left/mid_top so all position calculations are correct
    let map_left = map_left + sub_x;
    let mid_top = mid_top + sub_y;
    let player_screen_x = map_left + (state.player.x - camera_x) as f32 * tz + tz / 2.0;
    let player_screen_y = mid_top + (state.player.y - camera_y) as f32 * tz + tz / 2.0;
    let light_radius = state.vision_radius as f32 * tz;

    // Fill map area with wall/void color
    let wall_color = if !state.level.region_scales.is_empty() {
        // Unified map: use void/black as background
        state.level.tile_defs.get("void")
            .map(|d| hex_to_color(&d.color))
            .unwrap_or(BLACK)
    } else {
        state.level.tile_defs.values()
            .find(|d| !d.walkable)
            .map(|d| hex_to_color(&d.color))
            .unwrap_or(Color::new(0.1, 0.1, 0.1, 1.0))
    };
    draw_rectangle(0.0, top_height, sw, sh - top_height, wall_color);

    // Pre-compute tile colors to avoid hex_to_color per tile per frame
    let tile_colors: std::collections::HashMap<&str, Color> = state.level.tile_defs.iter()
        .map(|(name, def)| (name.as_str(), hex_to_color(&def.color)))
        .collect();

    // Tiles
    for sy in 0..=tiles_y {
        for sx in 0..=tiles_x {
            let tx = camera_x + sx;
            let ty = camera_y + sy;
            let out_of_bounds = tx < 0 || ty < 0 || tx >= state.level.width || ty >= state.level.height;
            let unrevealed = !out_of_bounds && !state.level.revealed.contains(&(tx, ty));
            if out_of_bounds || unrevealed {
                continue; // background fill already covers these
            }

            let tile_name = &state.level.tiles[ty as usize][tx as usize];
            // Skip void tiles — background already the right color
            if tile_name == "void" { continue; }
            let def = match state.level.tile_defs.get(tile_name) {
                Some(d) => d,
                None => continue,
            };

            let screen_x = map_left + sx as f32 * tz;
            let screen_y = mid_top + sy as f32 * tz;

            if screen_y + tz < top_height || screen_y > top_height + (sh - top_height - bottom_height) {
                continue;
            }
            if screen_x + tz > map_width {
                continue;
            }

            // At low zoom, skip expensive effects but still draw textures
            let low_zoom = tz < 8.0;

            if let Some(tex) = tile_textures.get(tile_name) {
                draw_texture_ex(tex, screen_x, screen_y, WHITE, DrawTextureParams {
                    dest_size: Some(Vec2::new(tz, tz)),
                    ..Default::default()
                });
            } else {
                if let Some(&c) = tile_colors.get(tile_name.as_str()) {
                    draw_rectangle(screen_x, screen_y, tz, tz, c);
                } else {
                    draw_rectangle(screen_x, screen_y, tz, tz, hex_to_color(&def.color));
                }
            }

            if low_zoom { continue; } // skip effects below at low zoom

            let in_vision = state.level.visible.contains(&(tx, ty));

            // Bomb scorch overlay
            if let Some(&intensity) = state.level.char_marks.get(&(tx, ty)) {
                let alpha = intensity * 0.6;
                draw_rectangle(screen_x, screen_y, tz, tz, Color::new(0.05, 0.02, 0.0, alpha));
            }

            if !def.char_display.is_empty() && !tile_textures.contains_key(tile_name) {
                let alpha = if in_vision { 0.27 } else { 0.13 };
                let font_size = (tz * 0.55) as u16;
                let text = &def.char_display;
                let tm = measure_text(text, None, font_size, 1.0);
                draw_text(
                    text,
                    screen_x + (tz - tm.width) / 2.0,
                    screen_y + tz / 2.0 + tm.height / 2.0,
                    font_size as f32,
                    Color::new(1.0, 1.0, 1.0, alpha),
                );
            }

            if !in_vision {
                draw_rectangle(screen_x, screen_y, tz, tz, Color::new(0.0, 0.0, 0.0, 0.5));
            }
        }
    }

    // ── Map ambient occlusion: darken walkable tiles near walls ──
    // Skip at low zoom (tiles too small to see the effect, and very expensive)
    if tz >= 8.0 {
    for sy in 0..=tiles_y {
        for sx in 0..=tiles_x {
            let tx = camera_x + sx;
            let ty = camera_y + sy;
            if tx < 0 || ty < 0 || tx >= state.level.width || ty >= state.level.height { continue; }
            if !state.level.revealed.contains(&(tx, ty)) { continue; }
            let tile_name = &state.level.tiles[ty as usize][tx as usize];
            let def = match state.level.tile_defs.get(tile_name) { Some(d) => d, None => continue };
            if !def.walkable { continue; }

            let screen_x = map_left + sx as f32 * tz;
            let screen_y = mid_top + sy as f32 * tz;
            if screen_y + tz < mid_top || screen_y > mid_top + mid_height || screen_x + tz > map_width { continue; }

            // Count adjacent walls
            let mut wall_count = 0u8;
            for &(dx, dy) in &[(0i32,1i32),(0,-1),(1,0),(-1,0),(1,1),(1,-1),(-1,1),(-1,-1)] {
                let nx = tx + dx;
                let ny = ty + dy;
                if nx < 0 || ny < 0 || nx >= state.level.width || ny >= state.level.height {
                    wall_count += 1;
                    continue;
                }
                let n_name = &state.level.tiles[ny as usize][nx as usize];
                if let Some(n_def) = state.level.tile_defs.get(n_name) {
                    if !n_def.walkable { wall_count += 1; }
                }
            }
            if wall_count > 0 {
                let ao = (wall_count as f32 / 8.0) * 0.25;
                draw_rectangle(screen_x, screen_y, tz, tz, Color::new(0.0, 0.0, 0.0, ao));
            }
        }
    }
    } // end AO skip at low zoom

    // Items
    for item in &state.level.items {
        if !state.level.visible.contains(&(item.x, item.y)) { continue; }
        let sx = map_left + (item.x - camera_x) as f32 * tz;
        let sy = mid_top + (item.y - camera_y) as f32 * tz;
        if sy + tz < mid_top || sy > mid_top + mid_height || sx + tz > map_width { continue; }

        if let Some(tex) = item_textures.get(&item.name) {
            let size = tz * 0.85;
            let ox = sx + (tz - size) / 2.0;
            let oy = sy + (tz - size) / 2.0;
            draw_texture_ex(tex, ox, oy, WHITE, DrawTextureParams {
                dest_size: Some(Vec2::new(size, size)),
                ..Default::default()
            });
        } else {
            let cx = sx + tz / 2.0;
            let cy = sy + tz / 2.0;
            let r = tz * 0.38;
            let color = item_color(&item.item_type);

            match item.item_type.as_str() {
                "weapon" => {
                    draw_soft_poly_shadow(cx, cy, 3, r, 0.0);
                    draw_poly(cx, cy, 3, r, 0.0, color);
                }
                "armor" => {
                    draw_soft_circle_shadow(cx, cy, r);
                    draw_circle(cx, cy, r, color);
                }
                _ => {
                    let half = r * 0.85;
                    draw_soft_rect_shadow(cx - half, cy - half, half * 2.0, half * 2.0);
                    draw_rectangle(cx - half, cy - half, half * 2.0, half * 2.0, color);
                }
            }
        }
    }

    // Triggered traps
    for trap in &state.level.traps {
        if !trap.triggered { continue; }
        if !state.level.revealed.contains(&(trap.x, trap.y)) { continue; }
        let sx = map_left + (trap.x - camera_x) as f32 * tz;
        let sy = mid_top + (trap.y - camera_y) as f32 * tz;
        if sy + tz < mid_top || sy > mid_top + mid_height || sx + tz > map_width { continue; }

        if let Some(tex) = item_textures.get(&trap.name) {
            let size = tz * 0.85;
            let ox = sx + (tz - size) / 2.0;
            let oy = sy + (tz - size) / 2.0;
            draw_texture_ex(tex, ox, oy, WHITE, DrawTextureParams {
                dest_size: Some(Vec2::new(size, size)),
                ..Default::default()
            });
        } else {
            let cx = sx + tz / 2.0;
            let cy = sy + tz / 2.0;
            let half = tz * 0.38 * 0.85;
            let trap_fill = Color::new(1.0, 0.0, 0.0, 0.4);
            let trap_line = hex_to_color("#ff4444");
            draw_rectangle(cx - half, cy - half, half * 2.0, half * 2.0, trap_fill);
            draw_line(cx - half + 3.0, cy - half + 3.0, cx + half - 3.0, cy + half - 3.0, 2.0, trap_line);
            draw_line(cx + half - 3.0, cy - half + 3.0, cx - half + 3.0, cy + half - 3.0, 2.0, trap_line);
        }
    }

    // Signposts
    for sign in &state.level.signposts {
        if !state.level.visible.contains(&(sign.x, sign.y)) { continue; }
        let sx = map_left + (sign.x - camera_x) as f32 * tz;
        let sy = mid_top + (sign.y - camera_y) as f32 * tz;
        if sy + tz < mid_top || sy > mid_top + mid_height || sx + tz > map_width { continue; }
        let tint = if sign.read { Color::new(0.5, 0.5, 0.5, 0.7) } else { WHITE };
        if let Some(tex) = item_textures.get("__sign__") {
            let size = tz * 0.85;
            let ox = sx + (tz - size) / 2.0;
            let oy = sy + (tz - size) / 2.0;
            draw_texture_ex(tex, ox, oy, tint,
                DrawTextureParams { dest_size: Some(Vec2::new(size, size)), ..Default::default() });
        } else {
            let cx = sx + tz / 2.0;
            let cy = sy + tz / 2.0;
            let r = tz * 0.35;
            let c = if sign.read { Color::new(0.3, 0.5, 0.2, 0.5) } else { Color::new(0.53, 0.8, 0.27, 1.0) };
            draw_poly(cx, cy, 4, r, 45.0, c);
        }
    }

    // Monsters — hexagons
    for mon in &state.level.monsters {
        if !mon.is_alive() { continue; }
        // Boss uses dynamic body tiles for visibility
        let mon_visible = if mon.is_boss {
            mon.boss_body.iter().any(|&(bx, by)| state.level.visible.contains(&(bx, by)))
        } else {
            state.level.visible.contains(&(mon.x, mon.y))
        };
        if !mon_visible { continue; }

        let sx = map_left + (mon.x - camera_x) as f32 * tz;
        let sy = mid_top + (mon.y - camera_y) as f32 * tz;
        if sy + tz < mid_top || sy > mid_top + mid_height || sx + tz > map_width { continue; }

        // Monsters face toward the player
        let mon_facing = ((state.player.y - mon.y) as f32).atan2((state.player.x - mon.x) as f32);

        if mon.is_boss {
            let base_color = hex_to_color(COLOR_BOSS);
            let pct = mon.hp as f32 / mon.max_hp as f32;
            let body = &mon.boss_body;

            // If boss has a sprite, draw it over the 2x2 area
            if let Some(tex) = monster_textures.get(&mon.name) {
                let bsx = map_left + (mon.x - camera_x) as f32 * tz;
                let bsy = mid_top + (mon.y - camera_y) as f32 * tz;
                draw_texture_ex(tex, bsx, bsy, WHITE, DrawTextureParams {
                    dest_size: Some(Vec2::new(tz * 2.0, tz * 2.0)),
                    ..Default::default()
                });
                // HP bar below
                if pct < 1.0 {
                    let bar_w = tz * 1.6;
                    let bar_h = 3.0;
                    let bar_x = bsx + (tz * 2.0 - bar_w) / 2.0;
                    let bar_y = bsy + tz * 2.0 + 2.0;
                    draw_rectangle(bar_x, bar_y, bar_w, bar_h, Color::new(0.2, 0.2, 0.2, 0.8));
                    draw_rectangle(bar_x, bar_y, bar_w * pct, bar_h, hp_bar_color(pct));
                }
            } else {

            // Pulse
            let time = get_time() as f32;
            let pulse = (time * 1.2 * std::f32::consts::TAU).sin() * 0.5 + 0.5;
            let expand = pulse * 1.5;

            let body_set: std::collections::HashSet<(i32,i32)> = body.iter().copied().collect();

            // Helper: expand only on outer edges (internal edges seamless)
            let tile_edges = |bx: i32, by: i32| -> (f32, f32, f32, f32) {
                (
                    if body_set.contains(&(bx - 1, by)) { 0.0 } else { expand },
                    if body_set.contains(&(bx + 1, by)) { 0.0 } else { expand },
                    if body_set.contains(&(bx, by - 1)) { 0.0 } else { expand },
                    if body_set.contains(&(bx, by + 1)) { 0.0 } else { expand },
                )
            };

            // Illuminate surrounding tiles
            let glow_alpha = pulse * 0.15;
            let glow_range = 2i32;
            for &(bx, by) in body {
                for oy in -glow_range..=glow_range {
                    for ox in -glow_range..=glow_range {
                        if ox == 0 && oy == 0 { continue; }
                        let tx = bx + ox;
                        let ty = by + oy;
                        if body_set.contains(&(tx, ty)) { continue; }
                        if !state.level.visible.contains(&(tx, ty)) { continue; }
                        let tile_sx = map_left + (tx - camera_x) as f32 * tz;
                        let tile_sy = mid_top + (ty - camera_y) as f32 * tz;
                        let min_dist = body.iter()
                            .map(|&(bbx, bby)| ((tx - bbx) as f32).hypot((ty - bby) as f32))
                            .fold(f32::MAX, f32::min);
                        let falloff = 1.0 - (min_dist / (glow_range as f32 + 1.0));
                        if falloff > 0.0 {
                            draw_rectangle(tile_sx, tile_sy, tz, tz,
                                Color::new(base_color.r, base_color.g, base_color.b, glow_alpha * falloff));
                        }
                    }
                }
            }

            let brightness = 0.7 + pulse * 0.3;
            let dark_core = Color::new(0.12, 0.08, 0.08, 1.0);
            let fill_color = Color::new(
                base_color.r * brightness,
                base_color.g * brightness * 0.3,
                base_color.b * brightness * 0.3,
                1.0,
            );

            // ── Draw boss as one connected shape ──

            // Shadow pass
            for &(bx, by) in body {
                let tsx = map_left + (bx - camera_x) as f32 * tz;
                let tsy = mid_top + (by - camera_y) as f32 * tz;
                let (el, er, et, eb) = tile_edges(bx, by);
                let layers: [(f32, f32); 3] = [(1.5, 0.16), (3.0, 0.10), (5.0, 0.04)];
                for &(off, alpha) in &layers {
                    draw_rectangle(tsx - el + off, tsy - et + off,
                        tz + el + er + off * 0.5, tz + et + eb + off * 0.5,
                        Color::new(0.0, 0.0, 0.0, alpha));
                }
            }

            // Unified HP fill height
            let min_y = body.iter().map(|t| t.1).min().unwrap();
            let max_y = body.iter().map(|t| t.1).max().unwrap();
            let shape_top = mid_top + (min_y - camera_y) as f32 * tz - expand;
            let shape_bottom = mid_top + (max_y - camera_y) as f32 * tz + tz + expand;
            let total_h = shape_bottom - shape_top;
            let fill_h = total_h * pct;
            let fill_top = shape_bottom - fill_h;

            // Dark core + HP fill
            for &(bx, by) in body {
                let tsx = map_left + (bx - camera_x) as f32 * tz;
                let tsy = mid_top + (by - camera_y) as f32 * tz;
                let (el, er, et, eb) = tile_edges(bx, by);
                let rx = tsx - el;
                let ry = tsy - et;
                let rw = tz + el + er;
                let rh = tz + et + eb;

                draw_rectangle(rx, ry, rw, rh, dark_core);

                if pct > 0.0 {
                    let tile_bottom = ry + rh;
                    if fill_top < tile_bottom {
                        let clip_top = fill_top.max(ry);
                        let clip_h = tile_bottom - clip_top;
                        if clip_h > 0.0 {
                            draw_rectangle(rx, clip_top, rw, clip_h, fill_color);
                        }
                    }
                }
            }

            // Border: pulsing, outer edges only
            let border_alpha = 0.4 + pulse * 0.6;
            let border_color = Color::new(base_color.r, base_color.g * 0.4, base_color.b * 0.4, border_alpha);
            let t = 2.0;
            for &(bx, by) in body {
                let tsx = map_left + (bx - camera_x) as f32 * tz;
                let tsy = mid_top + (by - camera_y) as f32 * tz;
                let (el, er, et, eb) = tile_edges(bx, by);
                if !body_set.contains(&(bx, by - 1)) {
                    draw_rectangle(tsx - el, tsy - et, tz + el + er, t, border_color);
                }
                if !body_set.contains(&(bx, by + 1)) {
                    draw_rectangle(tsx - el, tsy + tz + eb - t, tz + el + er, t, border_color);
                }
                if !body_set.contains(&(bx - 1, by)) {
                    draw_rectangle(tsx - el, tsy - et, t, tz + et + eb, border_color);
                }
                if !body_set.contains(&(bx + 1, by)) {
                    draw_rectangle(tsx + tz + er - t, tsy - et, t, tz + et + eb, border_color);
                }
            }
            } // end else (no boss texture)
        } else if let Some(tex) = monster_textures.get(&mon.name) {
            // Draw sprite texture
            let size = tz * 0.9;
            let ox = sx + (tz - size) / 2.0;
            let oy = sy + (tz - size) / 2.0;
            draw_texture_ex(tex, ox, oy, WHITE, DrawTextureParams {
                dest_size: Some(Vec2::new(size, size)),
                ..Default::default()
            });
            // HP bar below sprite
            let pct = mon.hp as f32 / mon.max_hp as f32;
            if pct < 1.0 {
                let bar_w = tz * 0.8;
                let bar_h = 3.0;
                let bar_x = sx + (tz - bar_w) / 2.0;
                let bar_y = sy + tz - 2.0;
                draw_rectangle(bar_x, bar_y, bar_w, bar_h, Color::new(0.2, 0.2, 0.2, 0.8));
                draw_rectangle(bar_x, bar_y, bar_w * pct, bar_h, hp_bar_color(pct));
            }
        } else {
            let cx = sx + tz / 2.0;
            let cy = sy + tz / 2.0;
            let r = tz * 0.4;
            let pct = mon.hp as f32 / mon.max_hp as f32;
            let base_color = hex_to_color(COLOR_MONSTER);
            draw_soft_circle_shadow(cx, cy, r);
            draw_circle(cx, cy, r, Color::new(0.15, 0.15, 0.15, 1.0));
            if pct > 0.0 {
                draw_pie(cx, cy, r, pct, base_color, mon_facing);
            }
        }
    }

    // Player — HP pie + shield outline + weapon line
    let px = map_left + (state.player.x - camera_x) as f32 * tz + tz / 2.0;
    let py = mid_top + (state.player.y - camera_y) as f32 * tz + tz / 2.0;
    let has_shield = state.player.armor != "None";
    let has_sword = state.player.weapon != "Fists";
    let r = tz * 0.38;

    let hp_pct = if state.player.max_hp > 0 {
        state.player.hp as f32 / state.player.max_hp as f32
    } else {
        0.0
    };

    // Soft shadow
    draw_soft_circle_shadow(px, py, r);

    // Dark background + HP pie
    draw_circle(px, py, r, Color::new(0.15, 0.15, 0.15, 1.0));
    if hp_pct > 0.0 {
        draw_pie(px, py, r, hp_pct, hp_bar_color(hp_pct), state.player.facing);
    }

    // Shield: bright blue outline ring
    if has_shield {
        let segments = 32;
        let shield_color = hex_to_color(COLOR_SHIELD);
        for i in 0..segments {
            let a1 = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let a2 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
            draw_line(
                px + a1.cos() * r, py + a1.sin() * r,
                px + a2.cos() * r, py + a2.sin() * r,
                2.5, shield_color,
            );
        }
    }

    // Weapon: inner line from center to facing direction edge
    if has_sword {
        let sword_color = hex_to_color(COLOR_WEAPON);
        let tip_x = px + state.player.facing.cos() * (r - 1.0);
        let tip_y = py + state.player.facing.sin() * (r - 1.0);
        draw_line(px, py, tip_x, tip_y, 2.5, sword_color);
    }

    // ── Smooth radial light falloff (sub-tile grid, not aligned to tiles) ──
    // Skip at low zoom — vision radius covers the whole screen anyway
    if tz >= 8.0 {
    let cell = 6.0_f32; // sub-tile cell size for smooth gradient
    let light_max_alpha = 0.5; // match fog-of-war darkness at edges
    let light_area_top = top_height;
    let light_area_height = sh - top_height - bottom_height;
    let cx_count = (map_width / cell) as i32 + 2;
    let cy_count = (light_area_height / cell) as i32 + 2;
    for cy in 0..cy_count {
        let y = light_area_top + cy as f32 * cell;
        if y > light_area_top + light_area_height { continue; }
        let dy = y + cell / 2.0 - player_screen_y;
        for cx in 0..cx_count {
            let x = cx as f32 * cell;
            if x > map_width { continue; }
            let dx = x + cell / 2.0 - player_screen_x;
            let dist = (dx * dx + dy * dy).sqrt();
            let t = (dist / light_radius).min(1.0);
            let darkness = t * t; // quadratic
            if darkness > 0.01 {
                draw_rectangle(x, y, cell, cell, Color::new(0.0, 0.0, 0.0, darkness * light_max_alpha));
            }
        }
    }
    } // end radial light skip at low zoom

    // ── Redraw header/footer on top of map to clip any tile bleeding ──
    draw_rectangle(0.0, 0.0, sw, top_height, Color::new(0.05, 0.05, 0.05, 1.0));
    draw_line(0.0, top_height, sw, top_height, 1.0, Color::new(0.13, 0.13, 0.13, 1.0));
    if !state.level.title.is_empty() {
        let tfont = title_font.unwrap_or(ui_font);
        let ts = 32u16;
        let tw = measure_text(&state.level.title, Some(tfont), ts, 1.0).width;
        draw_text_ex(&state.level.title, (sw - tw) / 2.0, 48.0, TextParams {
            font: Some(tfont), font_size: ts, color: hex_to_color("#e0d5c0"), ..Default::default()
        });
    }

    // ── Log panel (right side) — uses fixed top_height, not shifted mid_top ──
    let log_left = sw - log_width;
    let panel_top = top_height;
    let panel_height = sh - top_height - bottom_height;
    draw_rectangle(log_left, panel_top, log_width, panel_height, Color::new(0.04, 0.04, 0.04, 1.0));
    draw_line(log_left, panel_top, log_left, panel_top + panel_height, 1.0, Color::new(0.15, 0.15, 0.15, 1.0));

    let log_font_size = 13u16;
    let line_h = 18.0;
    let log_pad = 12.0;
    let log_text_top = panel_top + log_pad + log_font_size as f32;
    let log_max_w = log_width - log_pad * 2.0;
    // Word-wrap log entries into visual lines
    let entry_gap = 4.0_f32; // small gap between log entries
    let mut wrapped: Vec<(String, String, bool)> = Vec::new(); // (text, color, is_last_line_of_entry)
    let log_start = state.log.len().saturating_sub(50);
    for entry in &state.log[log_start..] {
        let mut current = String::new();
        for word in entry.text.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", current, word)
            };
            if measure_text(&candidate, Some(ui_font), log_font_size, 1.0).width > log_max_w && !current.is_empty() {
                wrapped.push((current, entry.color.clone(), false));
                current = word.to_string();
            } else {
                current = candidate;
            }
        }
        if !current.is_empty() {
            wrapped.push((current, entry.color.clone(), true));
        }
    }

    // Calculate visible lines from the bottom, accounting for entry gaps
    let mut y_cursor = panel_top + panel_height - 8.0;
    let mut vis_start = wrapped.len();
    for i in (0..wrapped.len()).rev() {
        y_cursor -= line_h;
        if wrapped[i].2 { y_cursor -= entry_gap; }
        if y_cursor < log_text_top { break; }
        vis_start = i;
    }

    let mut y = log_text_top;
    for (text, color, last) in &wrapped[vis_start..] {
        draw_text_ex(text, log_left + log_pad, y, TextParams {
            font: Some(ui_font), font_size: log_font_size, color: hex_to_color(color), ..Default::default()
        });
        y += line_h;
        if *last { y += entry_gap; }
    }

    // ── BOTTOM ROW: Status bar ──
    let bot_y = sh - bottom_height;
    draw_rectangle(0.0, bot_y, sw, bottom_height, Color::new(0.05, 0.05, 0.05, 1.0));
    draw_line(0.0, bot_y, sw, bot_y, 1.0, Color::new(0.13, 0.13, 0.13, 1.0));

    let cy = bot_y + bottom_height / 2.0;
    let text_y = cy + 5.0;
    let fs = 13u16;
    let gap = 14.0;
    let mut x = 12.0;
    let dim = Color::new(0.3, 0.3, 0.3, 1.0);

    let total_atk = state.player.attack + state.player.weapon_damage;
    let total_def = state.player.defense + state.player.armor_defense;
    let atk_color = hex_to_color(COLOR_WEAPON);
    let def_color = hex_to_color(COLOR_SHIELD);
    let gold_color = hex_to_color("#ffd700");
    let potion_color = hex_to_color("#44ff44");
    let speed_color = hex_to_color("#44ddff");
    let bomb_color = hex_to_color("#ff6600");

    // Sword icon + ATK (dim if no weapon)
    let has_weapon = state.player.weapon != "Fists";
    let sword_color = if has_weapon { atk_color } else { dim };
    draw_line(x + 1.0, cy - 6.0, x + 1.0, cy + 6.0, 2.0, sword_color);
    draw_line(x - 3.0, cy - 2.0, x + 5.0, cy - 2.0, 1.5, sword_color);
    x += 10.0;
    let atk_text = format!("{}", total_atk);
    draw_text_ex(&atk_text, x, text_y, TextParams {
        font: Some(ui_font), font_size: fs, color: atk_color, ..Default::default()
    });
    x += measure_text(&atk_text, Some(ui_font), fs, 1.0).width + gap;

    // Shield icon + DEF (dim icon if no armor, number always bright)
    let has_armor = state.player.armor != "None";
    let shield_color = if has_armor { def_color } else { dim };
    draw_rectangle(x, cy - 5.0, 8.0, 10.0, shield_color);
    draw_triangle(Vec2::new(x, cy + 5.0), Vec2::new(x + 8.0, cy + 5.0), Vec2::new(x + 4.0, cy + 9.0), shield_color);
    x += 12.0;
    let def_text = format!("{}", total_def);
    draw_text_ex(&def_text, x, text_y, TextParams {
        font: Some(ui_font), font_size: fs, color: def_color, ..Default::default()
    });
    x += measure_text(&def_text, Some(ui_font), fs, 1.0).width + gap;

    // Level
    let lvl_text = format!("L{}", state.player.level);
    draw_text_ex(&lvl_text, x, text_y, TextParams {
        font: Some(ui_font), font_size: fs, color: WHITE, ..Default::default()
    });
    x += measure_text(&lvl_text, Some(ui_font), fs, 1.0).width + gap;

    // Gold diamond + count
    draw_diamond(x + 4.0, cy, 4.0, gold_color);
    x += 12.0;
    let gold_text = format!("{}", state.player.gold);
    draw_text_ex(&gold_text, x, text_y, TextParams {
        font: Some(ui_font), font_size: fs, color: gold_color, ..Default::default()
    });
    x += measure_text(&gold_text, Some(ui_font), fs, 1.0).width + gap;

    // Key indicator — same square shape as in-game
    if state.player.keys > 0 {
        let key_color = hex_to_color("#ffd700");
        let half = 5.0;
        draw_rectangle(x, cy - half, half * 2.0, half * 2.0, key_color);
        x += half * 2.0 + 4.0;
        if state.player.keys > 1 {
            let kt = format!("{}", state.player.keys);
            draw_text_ex(&kt, x, text_y, TextParams {
                font: Some(ui_font), font_size: fs, color: key_color, ..Default::default()
            });
            x += measure_text(&kt, Some(ui_font), fs, 1.0).width;
        }
        x += gap;
    }

    // Potions: green dots + count
    if state.player.potions > 0 {
        draw_circle(x + 4.0, cy, 4.0, potion_color);
        x += 12.0;
        let pt = format!("{}", state.player.potions);
        draw_text_ex(&pt, x, text_y, TextParams {
            font: Some(ui_font), font_size: fs, color: potion_color, ..Default::default()
        });
        x += measure_text(&pt, Some(ui_font), fs, 1.0).width + gap;
    }

    // Speed potions: cyan triangle + count
    if state.player.speed_potions > 0 {
        draw_triangle(Vec2::new(x + 4.0, cy - 5.0), Vec2::new(x, cy + 4.0), Vec2::new(x + 8.0, cy + 4.0), speed_color);
        x += 12.0;
        let st = format!("{}", state.player.speed_potions);
        draw_text_ex(&st, x, text_y, TextParams {
            font: Some(ui_font), font_size: fs, color: speed_color, ..Default::default()
        });
        x += measure_text(&st, Some(ui_font), fs, 1.0).width + gap;
    }

    // Bombs: orange triangle + count
    if state.player.bombs > 0 {
        draw_triangle(Vec2::new(x + 4.0, cy - 5.0), Vec2::new(x, cy + 4.0), Vec2::new(x + 8.0, cy + 4.0), bomb_color);
        x += 12.0;
        let bt = format!("{}", state.player.bombs);
        draw_text_ex(&bt, x, text_y, TextParams {
            font: Some(ui_font), font_size: fs, color: bomb_color, ..Default::default()
        });
    }

    // Speed effect indicator (right side)
    if state.player.speed_turns > 0 {
        let fast = format!("FAST {}", state.player.speed_turns);
        let fw = measure_text(&fast, Some(ui_font), fs, 1.0).width;
        draw_text_ex(&fast, sw - fw - 16.0, text_y, TextParams {
            font: Some(ui_font), font_size: fs, color: speed_color, ..Default::default()
        });
    } else {
        let keys = "WASD  C:heal  X:bomb  Z:speed";
        let kw = measure_text(keys, Some(ui_font), 11, 1.0).width;
        draw_text_ex(keys, sw - kw - 16.0, text_y - 1.0, TextParams {
            font: Some(ui_font), font_size: 11, color: dim, ..Default::default()
        });
    }
}

fn draw_diamond(cx: f32, cy: f32, r: f32, color: Color) {
    draw_triangle(
        Vec2::new(cx, cy - r),
        Vec2::new(cx - r, cy),
        Vec2::new(cx, cy + r),
        color,
    );
    draw_triangle(
        Vec2::new(cx, cy - r),
        Vec2::new(cx + r, cy),
        Vec2::new(cx, cy + r),
        color,
    );
}

fn draw_pie(cx: f32, cy: f32, r: f32, pct: f32, color: Color, facing: f32) {
    let segments = 32;
    let angle_span = pct.clamp(0.0, 1.0) * std::f32::consts::TAU;
    let start_angle = facing - angle_span / 2.0; // center the filled arc on facing direction
    for i in 0..segments {
        let a1 = start_angle + (i as f32 / segments as f32) * angle_span;
        let a2 = start_angle + ((i + 1) as f32 / segments as f32) * angle_span;
        draw_triangle(
            Vec2::new(cx, cy),
            Vec2::new(cx + a1.cos() * r, cy + a1.sin() * r),
            Vec2::new(cx + a2.cos() * r, cy + a2.sin() * r),
            color,
        );
    }
}

fn hp_bar_color(pct: f32) -> Color {
    if pct > 0.5 {
        hex_to_color("#66bb6a")
    } else if pct > 0.25 {
        hex_to_color("#ffa726")
    } else {
        hex_to_color("#ef5350")
    }
}

fn draw_store_screen(font: &Font, bold: &Font, state: &GameState, items: &[StoreSlot], selection: usize) {
    let sw = screen_width();
    let sh = screen_height();
    clear_background(Color::new(0.05, 0.04, 0.02, 1.0));

    // Title
    let title = "STORE";
    let ts = 48u16;
    let tw = measure_text(title, Some(bold), ts, 1.0).width;
    draw_text_ex(title, (sw - tw) / 2.0, sh * 0.12, TextParams {
        font: Some(bold), font_size: ts, color: hex_to_color("#ffd700"), ..Default::default()
    });

    // Gold display
    let gold_text = format!("Gold: {}", state.player.gold);
    let gs = 20u16;
    let gw = measure_text(&gold_text, Some(font), gs, 1.0).width;
    draw_text_ex(&gold_text, (sw - gw) / 2.0, sh * 0.19, TextParams {
        font: Some(font), font_size: gs, color: hex_to_color("#ffd700"), ..Default::default()
    });

    // Inventory summary with icons
    let inv_y = sh * 0.24;
    let inv_fs = 16u16;
    let potion_c = hex_to_color("#44ff44");
    let speed_c = hex_to_color("#44ddff");
    let bomb_c = hex_to_color("#ff6600");

    // Measure total width to center: icon+num + gap + icon+num + gap + icon+num
    let p_text = format!("{}", state.player.potions);
    let s_text = format!("{}", state.player.speed_potions);
    let b_text = format!("{}", state.player.bombs);
    let icon_sz = 6.0;
    let icon_gap = 6.0;
    let group_gap = 20.0;
    let total_w = (icon_sz * 2.0 + icon_gap + measure_text(&p_text, Some(font), inv_fs, 1.0).width) +
        group_gap +
        (icon_sz * 2.0 + icon_gap + measure_text(&s_text, Some(font), inv_fs, 1.0).width) +
        group_gap +
        (icon_sz * 2.0 + icon_gap + measure_text(&b_text, Some(font), inv_fs, 1.0).width);
    let mut ix = (sw - total_w) / 2.0;

    // Potion: green circle
    draw_circle(ix + icon_sz, inv_y - 4.0, icon_sz, potion_c);
    ix += icon_sz * 2.0 + icon_gap;
    draw_text_ex(&p_text, ix, inv_y, TextParams {
        font: Some(font), font_size: inv_fs, color: potion_c, ..Default::default()
    });
    ix += measure_text(&p_text, Some(font), inv_fs, 1.0).width + group_gap;

    // Speed: cyan triangle
    draw_triangle(Vec2::new(ix + icon_sz, inv_y - icon_sz - 4.0), Vec2::new(ix, inv_y + 2.0), Vec2::new(ix + icon_sz * 2.0, inv_y + 2.0), speed_c);
    ix += icon_sz * 2.0 + icon_gap;
    draw_text_ex(&s_text, ix, inv_y, TextParams {
        font: Some(font), font_size: inv_fs, color: speed_c, ..Default::default()
    });
    ix += measure_text(&s_text, Some(font), inv_fs, 1.0).width + group_gap;

    // Bomb: orange triangle
    draw_triangle(Vec2::new(ix + icon_sz, inv_y - icon_sz - 4.0), Vec2::new(ix, inv_y + 2.0), Vec2::new(ix + icon_sz * 2.0, inv_y + 2.0), bomb_c);
    ix += icon_sz * 2.0 + icon_gap;
    draw_text_ex(&b_text, ix, inv_y, TextParams {
        font: Some(font), font_size: inv_fs, color: bomb_c, ..Default::default()
    });

    // Items
    let item_x = sw * 0.25;
    let mut y = sh * 0.35;
    let line_h = 50.0;

    for (i, item) in items.iter().enumerate() {
        let selected = i == selection;
        let can_buy = item.stock > 0 && state.player.gold >= item.price;

        // Selection indicator
        if selected {
            draw_rectangle(item_x - 20.0, y - 18.0, sw * 0.5 + 40.0, line_h - 6.0,
                Color::new(0.15, 0.12, 0.05, 1.0));
            draw_text_ex(">", item_x - 16.0, y + 5.0, TextParams {
                font: Some(bold), font_size: 20, color: hex_to_color("#ffd700"), ..Default::default()
            });
        }

        let name_color = if can_buy {
            if selected { hex_to_color("#ffffff") } else { hex_to_color("#cccccc") }
        } else {
            Color::new(0.4, 0.4, 0.4, 1.0)
        };

        // Item icon (matches status bar shapes, full row height)
        let icon_cx = item_x + 20.0;
        let icon_cy = y + 2.0;
        let icon_r = 14.0;
        let icon_color = if can_buy {
            match item.item_type.as_str() {
                "potion" => hex_to_color("#44ff44"),
                "speed_potion" => hex_to_color("#44ddff"),
                "bomb" => hex_to_color("#ff6600"),
                _ => WHITE,
            }
        } else {
            Color::new(0.25, 0.25, 0.25, 1.0)
        };
        match item.item_type.as_str() {
            "potion" => {
                draw_circle(icon_cx, icon_cy, icon_r, icon_color);
            }
            "speed_potion" | "bomb" => {
                draw_triangle(
                    Vec2::new(icon_cx, icon_cy - icon_r),
                    Vec2::new(icon_cx - icon_r, icon_cy + icon_r),
                    Vec2::new(icon_cx + icon_r, icon_cy + icon_r),
                    icon_color,
                );
            }
            _ => {}
        }

        // Name (shifted right to make room for icon)
        draw_text_ex(&item.name, item_x + 46.0, y + 5.0, TextParams {
            font: Some(bold), font_size: 20, color: name_color, ..Default::default()
        });

        // Description
        draw_text_ex(&item.description, item_x + 46.0, y + 22.0, TextParams {
            font: Some(font), font_size: 14, color: Color::new(0.5, 0.5, 0.5, 1.0), ..Default::default()
        });

        // Price and stock (right-aligned)
        let price_text = format!("{}g  x{}", item.price, item.stock);
        let price_color = if can_buy { hex_to_color("#ffd700") } else { Color::new(0.4, 0.3, 0.1, 1.0) };
        let pw = measure_text(&price_text, Some(font), 18, 1.0).width;
        draw_text_ex(&price_text, item_x + sw * 0.5 - pw, y + 5.0, TextParams {
            font: Some(font), font_size: 18, color: price_color, ..Default::default()
        });

        y += line_h;
    }

    // Controls
    let help = "[ENTER] Buy   [ESC] Leave";
    let hw = measure_text(help, Some(font), 16, 1.0).width;
    draw_text_ex(help, (sw - hw) / 2.0, sh * 0.88, TextParams {
        font: Some(font), font_size: 16, color: Color::new(0.4, 0.4, 0.4, 1.0), ..Default::default()
    });
}
