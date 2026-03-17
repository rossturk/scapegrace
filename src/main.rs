mod game;
mod gen;
mod maps;
mod sfx;

use game::*;
use macroquad::prelude::*;
use ::rand::Rng;
use std::sync::mpsc;

const TILE: f32 = 24.0;
const NAV_INITIAL_DELAY: f64 = 0.3;
const NAV_REPEAT_RATE: f64 = 0.15;
const GAME_INITIAL_DELAY: f64 = 0.18;
const GAME_REPEAT_RATE: f64 = 0.10;

enum Screen {
    KeyEntry,
    Start,
    GenOverworld,
    Overworld,
    GenLevel,
    Playing,
    Dead,
    Victory,
    GameWon,
}

enum GenMsg {
    Phase(String, String),
    Token,
    OverworldReady(game::Overworld, Option<Vec<u8>>),
    LevelDone(Level, [i32; 2], Option<Vec<u8>>),
    Error(String),
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Scapegrace".to_owned(),
        window_width: 1200,
        window_height: 800,
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
        "weapon" => hex_to_color("#ff8844"),
        "armor" => hex_to_color("#4488ff"),
        "potion" => hex_to_color("#44ff44"),
        "gold" => hex_to_color("#ffd700"),
        _ => WHITE,
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    dotenvy::dotenv().ok();

    let ui_font = load_ttf_font_from_bytes(include_bytes!("../assets/JetBrainsMono-Regular.ttf"))
        .expect("Failed to load embedded UI font");
    let ui_font_bold = load_ttf_font_from_bytes(include_bytes!("../assets/JetBrainsMono-Bold.ttf"))
        .expect("Failed to load embedded UI bold font");

    let sfx = sfx::Sfx::new();

    let mut state = GameState::new();
    let has_key = !std::env::var("OPENROUTER_API_KEY").unwrap_or_default().is_empty();
    let mut screen = if has_key { Screen::Start } else { Screen::KeyEntry };
    let mut key_input = String::new();
    let mut key_error: Option<String> = None;
    let mut key_validating = false;
    let mut key_rx: Option<mpsc::Receiver<Result<(), String>>> = None;
    let mut gen_rx: Option<mpsc::Receiver<GenMsg>> = None;
    let mut phase_text = String::new();
    let mut phase_detail = String::new();
    let mut loading_tiles: usize = 0;
    let mut confetti: Vec<Confetti> = vec![];
    let mut title_font: Option<Font> = None;
    let mut overworld_font: Option<Font> = None;

    // Overworld state
    let mut overworld: Option<Overworld> = None;
    let mut player_snapshot: Option<Player> = None;
    let mut level_snapshot: Option<(usize, Level, [i32; 2])> = None; // (node_index, level, start) for retry

    // Key repeat
    let mut nav_hold_time: f64 = 0.0;
    let mut nav_last_fire: f64 = 0.0;
    let mut nav_last_dir: (f32, f32) = (0.0, 0.0);
    let mut nav_cycle_idx: usize = 0;
    let mut game_hold_time: f64 = 0.0;
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
                                std::env::set_var("OPENROUTER_API_KEY", key_input.trim());
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
                        } else {
                            // Validate the key in background
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

            Screen::Start => {
                draw_start_screen(&ui_font, &ui_font_bold);
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                    start_overworld_generation(&mut gen_rx);
                    screen = Screen::GenOverworld;
                    phase_text = overworld_loading_phrase();
                    phase_detail.clear();
                    loading_tiles = 0;
                }
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
                                overworld = Some(ow);
                                screen = Screen::Overworld;
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

            Screen::Overworld => {
                if let Some(ow) = &mut overworld {
                    draw_overworld(&*ow, &ui_font, &ui_font_bold, overworld_font.as_ref());

                    // Navigation with key repeat
                    let cur = ow.current_node;
                    let (dx, dy) = get_held_direction();
                    let now = get_time();
                    let nav_fire = if dx != 0.0 || dy != 0.0 {
                        if nav_hold_time == 0.0 {
                            // Key just pressed
                            nav_hold_time = now;
                            nav_last_fire = now;
                            true
                        } else if now - nav_hold_time >= NAV_INITIAL_DELAY
                            && now - nav_last_fire >= NAV_REPEAT_RATE
                        {
                            nav_last_fire = now;
                            true
                        } else {
                            false
                        }
                    } else {
                        nav_hold_time = 0.0;
                        false
                    };
                    if nav_fire {
                        let cur_x = ow.nodes[cur].x;
                        let cur_y = ow.nodes[cur].y;
                        let dir_len = (dx * dx + dy * dy).sqrt();
                        // Collect all candidates in this direction, sorted by cosine
                        let mut candidates: Vec<(usize, f32)> = Vec::new();
                        for &(a, b) in &ow.connections {
                            let neighbor = if a == cur { b } else if b == cur { a } else { continue };
                            if !ow.nodes[neighbor].unlocked { continue; }
                            let nx = ow.nodes[neighbor].x - cur_x;
                            let ny = ow.nodes[neighbor].y - cur_y;
                            let node_len = (nx * nx + ny * ny).sqrt();
                            if node_len < 0.001 { continue; }
                            let cosine = (nx * dx + ny * dy) / (node_len * dir_len);
                            if cosine > 0.3 {
                                candidates.push((neighbor, cosine));
                            }
                        }
                        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

                        // If same direction as last press, cycle; otherwise reset
                        let same_dir = (dx - nav_last_dir.0).abs() < 0.01 && (dy - nav_last_dir.1).abs() < 0.01;
                        if same_dir && candidates.len() > 1 {
                            nav_cycle_idx = (nav_cycle_idx + 1) % candidates.len();
                        } else {
                            nav_cycle_idx = 0;
                        }
                        nav_last_dir = (dx, dy);

                        let best = candidates.get(nav_cycle_idx).map(|c| c.0);
                        if let Some(next) = best {
                            ow.current_node = next;
                            if let Some(s) = &sfx { s.navigate(); }
                        }
                    }

                    if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                        let node = &ow.nodes[ow.current_node];
                        if node.unlocked && !node.completed {
                            if let Some(s) = &sfx { s.confirm(); }
                            player_snapshot = Some(state.player.clone());
                            // Reuse saved level if retrying after death on same node
                            if let Some((snap_node, _, _)) = &level_snapshot {
                                if *snap_node != ow.current_node {
                                    // Different node — clear snapshot and generate fresh
                                    level_snapshot = None;
                                }
                            }
                            if let Some((_snap_node, lvl, start)) = &level_snapshot {
                                state.level = lvl.clone();
                                state.player.x = start[0];
                                state.player.y = start[1];
                                state.game_over = false;
                                state.victory = false;
                                state.log.clear();
                                reveal_around(
                                    &mut state.level,
                                    state.player.x,
                                    state.player.y,
                                    state.vision_radius,
                                );
                                state.log(&state.level.description.clone(), "#888");
                                state.log("Your task: find and defeat the boss.", "#666");
                                if let Some(s) = &sfx { s.start_boss_drone(&state.level.scale); }
                                screen = Screen::Playing;
                            } else {
                                start_level_generation(&state, ow, &mut gen_rx);
                                screen = Screen::GenLevel;
                                phase_text = overworld_loading_phrase();
                                phase_detail.clear();
                                loading_tiles = 0;
                            }
                        }
                    }
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
                            GenMsg::LevelDone(level, start, font_bytes) => {
                                // Snapshot for death retry (clean level state)
                                let snap_node = overworld.as_ref().map_or(0, |ow| ow.current_node);
                                level_snapshot = Some((snap_node, level.clone(), start));
                                state.level = level;
                                state.player.x = start[0];
                                state.player.y = start[1];
                                state.game_over = false;
                                state.victory = false;
                                state.log.clear();
                                reveal_around(
                                    &mut state.level,
                                    state.player.x,
                                    state.player.y,
                                    state.vision_radius,
                                );
                                state.log(&state.level.description.clone(), "#888");
                                state.log("Your task: find and defeat the boss.", "#666");
                                if let Some(bytes) = font_bytes {
                                    match load_ttf_font_from_bytes(&bytes) {
                                        Ok(f) => title_font = Some(f),
                                        Err(e) => eprintln!("Font load error: {}", e),
                                    }
                                } else {
                                    title_font = None;
                                }
                                if let Some(s) = &sfx { s.start_boss_drone(&state.level.scale); }
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
                        start_level_generation(&state, ow, &mut gen_rx);
                        phase_text = "creating universe".into();
                        phase_detail.clear();
                    }
                }
            }

            Screen::Playing => {
                handle_playing_input(&mut state, &mut screen, &mut confetti, &sfx,
                    &mut game_hold_time, &mut game_last_fire);
                // Update boss drone volume based on distance to nearest living boss
                if let Some(s) = &sfx {
                    let dist = state.level.monsters.iter()
                        .filter(|m| m.is_boss && m.is_alive())
                        .map(|m| {
                            let dx = state.player.x as f32 - (m.x as f32 + 0.5);
                            let dy = state.player.y as f32 - (m.y as f32 + 0.5);
                            (dx * dx + dy * dy).sqrt()
                        })
                        .fold(f32::MAX, f32::min);
                    s.update_boss_drone(dist);
                }
                render_game(&state, &ui_font, title_font.as_ref());
            }

            Screen::Dead => {
                render_game(&state, &ui_font, title_font.as_ref());
                draw_death_overlay(&ui_font, &ui_font_bold, &state);

                if is_key_pressed(KeyCode::Enter) {
                    // Restore player from snapshot, return to overworld
                    // (level_snapshot is kept so re-entering replays the same map)
                    if let Some(snap) = &player_snapshot {
                        state.player = snap.clone();
                    }
                    state.game_over = false;
                    state.victory = false;
                    state.log.clear();
                    screen = Screen::Overworld;
                }
            }

            Screen::Victory => {
                render_game(&state, &ui_font, title_font.as_ref());
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

                        // Between-level transition: keep gold, XP, level, potions
                        // Lose weapon/armor, restore HP to max
                        state.player.weapon = "Fists".into();
                        state.player.weapon_damage = 0;
                        state.player.armor = "None".into();
                        state.player.armor_defense = 0;
                        state.player.hp = state.player.max_hp;
                        state.victory = false;
                        state.game_over = false;
                        state.log.clear();
                        confetti.clear();

                        // Beating the final level wins the game
                        if ow.nodes[completed_node].is_final {
                            spawn_confetti(&mut confetti);
                            screen = Screen::GameWon;
                        } else {
                            screen = Screen::Overworld;
                        }
                    }
                }
            }

            Screen::GameWon => {
                update_confetti(&mut confetti);
                draw_confetti(&confetti);
                draw_game_won_overlay(&ui_font, &ui_font_bold, &state, overworld_font.as_ref(), &overworld);

                // Continuously spawn confetti
                if confetti.len() < 200 {
                    let mut rng = ::rand::thread_rng();
                    if rng.gen::<f32>() < 0.3 {
                        spawn_confetti(&mut confetti);
                    }
                }

                if is_key_pressed(KeyCode::Enter) {
                    // Full reset
                    state = GameState::new();
                    overworld = None;
                    player_snapshot = None;
                    confetti.clear();
                    overworld_font = None;
                    title_font = None;
                    screen = Screen::Start;
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
) {
    let sc = state.level.scale.clone();
    if is_key_pressed(KeyCode::P) {
        use_potion(state);
        if let Some(s) = sfx { s.pickup_potion(&sc); }
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

        let result = try_move(state, dx, dy);
        let moved = result["moved"].as_bool().unwrap_or(false);
        let combat = result["combat"].as_bool().unwrap_or(false);
        if moved || combat {
            monster_turns(state);
        }

        // Trigger sounds based on what happened
        if let Some(s) = sfx {
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
                else if t.contains("defeated the") { s.kill(&sc); }
                else if t.contains("LEVEL UP") { s.level_up(&sc); }
            }
            if state.player.gold > gold_before { s.pickup_gold(&sc); }
            if state.player.potions > potions_before { s.pickup_potion(&sc); }
            if state.player.weapon != weapon_before { s.pickup_weapon(&sc); }
            if state.player.armor != armor_before { s.pickup_armor(&sc); }
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
        let tx2 = tx.clone();
        match gen::generate_overworld(
            |phase| { let _ = tx.send(GenMsg::Phase(phase.phase, phase.detail)); },
            move || { let _ = tx2.send(GenMsg::Token); },
        ) {
            Ok(ow) => {
                let font_bytes = fetch_google_font(&ow.font);
                let _ = tx.send(GenMsg::OverworldReady(ow, font_bytes));
            }
            Err(e) => {
                let _ = tx.send(GenMsg::Error(e));
            }
        }
    });
}

fn start_level_generation(
    state: &GameState,
    ow: &Overworld,
    gen_rx: &mut Option<mpsc::Receiver<GenMsg>>,
) {
    let (tx, rx) = mpsc::channel();
    *gen_rx = Some(rx);
    let player = state.player.clone();
    let node = &ow.nodes[ow.current_node];
    let config = gen::LevelConfig {
        title: node.name.clone(),
        font: node.font.clone(),
        description: node.description.clone(),
        theme: node.theme.clone(),
        budget: node.budget,
        floor: ow.current_node as i32 + 1,
    };

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

/// Fetch a Google Font TTF at runtime. Returns None on any failure.
fn fetch_google_font(font_name: &str) -> Option<Vec<u8>> {
    if font_name.is_empty() {
        return None;
    }
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
    // /auth/key actually checks if the key is valid
    let resp = client.get("https://openrouter.ai/api/v1/auth/key")
        .header("Authorization", format!("Bearer {}", key))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .map_err(|e| format!("Could not reach the gate: {}", e))?;

    if resp.status().is_success() {
        Ok(())
    } else if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 {
        Err("The passphrase was not recognized.".into())
    } else {
        Err(format!("The gate refused entry. ({})", resp.status()))
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
        "You came anyway.",
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
        draw_text_ex("sk-or-...", field_x + 12.0, field_y + 24.0, TextParams {
            font: Some(font), font_size: fs, color: Color::new(0.3, 0.3, 0.3, 1.0), ..Default::default()
        });
    } else {
        // Show: ••••••••last4  — but cap dots to fit the field
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
        let display = format!("{}{}", dot.repeat(dot_count), tail);
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
    let shadow_layers: [(f32, f32); 4] = [(1.0, 0.30), (2.0, 0.20), (3.5, 0.12), (5.0, 0.05)];
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

fn draw_overworld(ow: &Overworld, ui_font: &Font, ui_bold: &Font, ow_font: Option<&Font>) {
    let sw = screen_width();
    let sh = screen_height();

    // Layout
    let margin = 80.0;
    let top_bar = 100.0;
    let bottom_bar = 60.0;
    let map_left = margin;
    let map_right = sw - margin;
    let map_bottom = sh - bottom_bar;
    let map_w = map_right - map_left;

    // Title
    let tfont = ow_font.unwrap_or(ui_bold);
    let ts = 36u16;
    let tw = measure_text(&ow.name, Some(tfont), ts, 1.0).width;
    draw_text_ex(&ow.name, (sw - tw) / 2.0, 50.0, TextParams {
        font: Some(tfont), font_size: ts, color: hex_to_color("#e0d5c0"), ..Default::default()
    });

    // Description (word-wrapped, balanced to avoid orphans)
    let ds = 16u16;
    let max_desc_w = sw - margin * 2.0;
    let wrap_at = |max_w: f32| -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        let mut cur = String::new();
        for word in ow.description.split_whitespace() {
            let candidate = if cur.is_empty() { word.to_string() } else { format!("{} {}", cur, word) };
            if measure_text(&candidate, Some(ui_font), ds, 1.0).width > max_w && !cur.is_empty() {
                lines.push(cur);
                cur = word.to_string();
            } else {
                cur = candidate;
            }
        }
        if !cur.is_empty() { lines.push(cur); }
        lines
    };
    let mut desc_lines = wrap_at(max_desc_w);
    // If last line is an orphan (< 40% of max width), re-wrap narrower to balance
    if desc_lines.len() >= 2 {
        let last_w = measure_text(desc_lines.last().unwrap(), Some(ui_font), ds, 1.0).width;
        if last_w < max_desc_w * 0.4 {
            desc_lines = wrap_at(max_desc_w * 0.65);
        }
    }
    let line_h = ds as f32 + 4.0;
    let desc_height = desc_lines.len() as f32 * line_h;
    for (i, line) in desc_lines.iter().enumerate() {
        let lw = measure_text(line, Some(ui_font), ds, 1.0).width;
        draw_text_ex(line, (sw - lw) / 2.0, 78.0 + i as f32 * line_h, TextParams {
            font: Some(ui_font), font_size: ds, color: hex_to_color("#9e9e9e"), ..Default::default()
        });
    }

    // Adjust top_bar to account for wrapped description
    let top_bar = top_bar + (desc_height - line_h).max(0.0);
    let map_top = top_bar + 20.0;
    let map_h = map_bottom - map_top;

    // Helper to convert node coords to screen coords with bobbing
    let time = get_time(); // f64 for smooth precision
    let node_screen = |i: usize, n: &OverworldNode| -> (f32, f32) {
        let base_x = map_left + n.x * map_w;
        let base_y = map_top + n.y * map_h;
        let phase = i as f64 * 1.7;
        let bob_x = (time * 0.5 + phase).sin() * 8.0;
        let bob_y = (time * 0.7 + phase * 1.3).cos() * 6.0;
        (base_x + bob_x as f32, base_y + bob_y as f32)
    };

    // Draw connections as tile trails
    let conn_tile = 5.0;
    for &(a, b) in &ow.connections {
        if a >= ow.nodes.len() || b >= ow.nodes.len() { continue; }
        let (ax, ay) = node_screen(a, &ow.nodes[a]);
        let (bx, by) = node_screen(b, &ow.nodes[b]);
        let dx = bx - ax;
        let dy = by - ay;
        let dist = (dx * dx + dy * dy).sqrt();
        let steps = (dist / (conn_tile * 2.0)).ceil() as usize;
        if steps == 0 { continue; }

        let a_playable = ow.nodes[a].unlocked;
        let b_playable = ow.nodes[b].unlocked;
        let pal_a: Vec<Color> = ow.nodes[a].palette.iter().map(|c| {
            let c = hex_to_color(c);
            if a_playable { c } else { desaturate(c, 0.85) }
        }).collect();
        let pal_b: Vec<Color> = ow.nodes[b].palette.iter().map(|c| {
            let c = hex_to_color(c);
            if b_playable { c } else { desaturate(c, 0.85) }
        }).collect();
        let dim = if ow.nodes[a].unlocked && ow.nodes[b].unlocked { 0.6f32 } else { 0.25 };

        let mut seed = (a as u32 * 7919 + b as u32 * 104729).wrapping_mul(2654435761);
        let next = |s: &mut u32| -> u32 {
            *s = s.wrapping_mul(1103515245).wrapping_add(12345);
            (*s >> 16) & 0x7FFF
        };

        for s in 0..steps {
            let t = s as f32 / steps as f32;
            let tx = ax + dx * t;
            let ty = ay + dy * t;

            let ci = next(&mut seed) as usize % pal_a.len();
            let bi = next(&mut seed) as usize % pal_b.len();
            let ca = pal_a[ci];
            let cb = pal_b[bi];
            let c = Color::new(
                (ca.r * (1.0 - t) + cb.r * t) * dim,
                (ca.g * (1.0 - t) + cb.g * t) * dim,
                (ca.b * (1.0 - t) + cb.b * t) * dim,
                1.0,
            );
            draw_rectangle(tx - conn_tile / 2.0, ty - conn_tile / 2.0, conn_tile, conn_tile, c);
        }
    }

    // Draw nodes as organic tile blobs
    let tile_px = 15.0;
    let grid_w: i32 = 8;
    let grid_h: i32 = 8;
    for (i, node) in ow.nodes.iter().enumerate() {
        let (nx, ny) = node_screen(i, node);
        let playable = node.unlocked;
        let palette: Vec<Color> = node.palette.iter().map(|c| {
            let c = hex_to_color(c);
            if playable { c } else { desaturate(c, 0.85) }
        }).collect();

        // Seeded RNG for deterministic shape
        let mut seed = (i as u32).wrapping_mul(2654435761);
        let next = |s: &mut u32| -> u32 {
            *s = s.wrapping_mul(1103515245).wrapping_add(12345);
            (*s >> 16) & 0x7FFF
        };

        // Grow an organic blob: start with a 2x2 core, expand by adding neighbors
        let mut filled = [[false; 8]; 8];
        // Core
        for cy in 3..5 {
            for cx in 3..5 {
                filled[cy][cx] = true;
            }
        }
        // Grow ~16 more tiles by picking random empty neighbors of filled tiles
        let mut grown = 0;
        let target = 14 + (next(&mut seed) % 5) as i32; // 14-18 tiles added
        for _ in 0..300 {
            if grown >= target { break; }
            let ry = (next(&mut seed) % grid_h as u32) as i32;
            let rx = (next(&mut seed) % grid_w as u32) as i32;
            if filled[ry as usize][rx as usize] { continue; }
            // Must be adjacent to a filled tile
            let mut adj = false;
            for &(dx, dy) in &[(0,1),(0,-1),(1,0),(-1,0)] {
                let ax = rx + dx;
                let ay = ry + dy;
                if ax >= 0 && ax < grid_w && ay >= 0 && ay < grid_h && filled[ay as usize][ax as usize] {
                    adj = true;
                    break;
                }
            }
            if !adj { continue; }
            // Avoid thin arms: count how many filled neighbors this would have
            let mut nbrs = 0;
            for &(dx, dy) in &[(0,1),(0,-1),(1,0),(-1,0)] {
                let ax = rx + dx;
                let ay = ry + dy;
                if ax >= 0 && ax < grid_w && ay >= 0 && ay < grid_h && filled[ay as usize][ax as usize] {
                    nbrs += 1;
                }
            }
            if nbrs < 1 + (next(&mut seed) % 2) as i32 { continue; }
            filled[ry as usize][rx as usize] = true;
            grown += 1;
        }

        // Find bounding box of filled tiles to center the shape
        let mut min_x = grid_w; let mut max_x = 0i32;
        let mut min_y = grid_h; let mut max_y = 0i32;
        for gy in 0..grid_h {
            for gx in 0..grid_w {
                if filled[gy as usize][gx as usize] {
                    min_x = min_x.min(gx); max_x = max_x.max(gx);
                    min_y = min_y.min(gy); max_y = max_y.max(gy);
                }
            }
        }
        let shape_w = (max_x - min_x + 1) as f32 * tile_px;
        let shape_h = (max_y - min_y + 1) as f32 * tile_px;
        let ox = nx - shape_w / 2.0 - min_x as f32 * tile_px;
        let oy = ny - shape_h / 2.0 - min_y as f32 * tile_px;

        // Draw filled tiles
        for gy in 0..grid_h {
            for gx in 0..grid_w {
                if !filled[gy as usize][gx as usize] { continue; }
                let ci = next(&mut seed) as usize % palette.len();
                let mut c = palette[ci];

                if node.completed {
                    c = Color::new(c.r * 0.5, c.g * 0.5, c.b * 0.5, 1.0);
                } else if !node.unlocked {
                    c = Color::new(c.r * 0.25, c.g * 0.25, c.b * 0.25, 1.0);
                }

                let tx = ox + gx as f32 * tile_px;
                let ty = oy + gy as f32 * tile_px;
                draw_rectangle(tx, ty, tile_px, tile_px, c);
            }
        }

        // Final level: red outline tracing the blob perimeter
        if node.is_final {
            let red = hex_to_color("#e94560");
            for gy in 0..grid_h {
                for gx in 0..grid_w {
                    if !filled[gy as usize][gx as usize] { continue; }
                    let tx = ox + gx as f32 * tile_px;
                    let ty = oy + gy as f32 * tile_px;
                    // Draw edge lines where neighbor is empty
                    if gx == 0 || !filled[gy as usize][(gx - 1) as usize] {
                        draw_line(tx, ty, tx, ty + tile_px, 2.0, red);
                    }
                    if gx == grid_w - 1 || !filled[gy as usize][(gx + 1) as usize] {
                        draw_line(tx + tile_px, ty, tx + tile_px, ty + tile_px, 2.0, red);
                    }
                    if gy == 0 || !filled[(gy - 1) as usize][gx as usize] {
                        draw_line(tx, ty, tx + tile_px, ty, 2.0, red);
                    }
                    if gy == grid_h - 1 || !filled[(gy + 1) as usize][gx as usize] {
                        draw_line(tx, ty + tile_px, tx + tile_px, ty + tile_px, 2.0, red);
                    }
                }
            }
        }

        // Current node: green circle with soft shadow
        if i == ow.current_node {
            draw_soft_circle_shadow(nx, ny, tile_px * 1.2);
            draw_circle(nx, ny, tile_px * 1.2, hex_to_color("#44ff44"));
        }

        // Level name below node
        if node.unlocked {
            let ns = 14u16;
            let bot_y = oy + (max_y + 1) as f32 * tile_px;
            let nw = measure_text(&node.name, Some(ui_font), ns, 1.0).width;
            draw_text_ex(&node.name, nx - nw / 2.0, bot_y + 16.0, TextParams {
                font: Some(ui_font), font_size: ns, color: WHITE, ..Default::default()
            });
        }
    }

}

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

    let summary = format!("Level {}  {} gold", state.player.level, state.player.gold);
    let ss = 18u16;
    let smw = measure_text(&summary, Some(font), ss, 1.0).width;
    draw_text_ex(&summary, (sw - smw) / 2.0, sh / 2.0 + 20.0, TextParams {
        font: Some(font), font_size: ss, color: GRAY, ..Default::default()
    });

    let prompt = "Press ENTER to return to overworld";
    let ps = 16u16;
    let pw = measure_text(prompt, Some(font), ps, 1.0).width;
    draw_text_ex(prompt, (sw - pw) / 2.0, sh / 2.0 + 55.0, TextParams {
        font: Some(font), font_size: ps, color: DARKGRAY, ..Default::default()
    });
}

fn draw_victory_overlay(font: &Font, bold: &Font, state: &GameState) {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.88));

    let title = "VICTORY";
    let ts = 52u16;
    let tw = measure_text(title, Some(bold), ts, 1.0).width;
    draw_text_ex(title, (sw - tw) / 2.0, sh / 2.0 - 20.0, TextParams {
        font: Some(bold), font_size: ts, color: hex_to_color("#ffd700"), ..Default::default()
    });

    let summary = format!("Level {}  {} gold  {}", state.player.level, state.player.gold, state.player.weapon);
    let ss = 18u16;
    let smw = measure_text(&summary, Some(font), ss, 1.0).width;
    draw_text_ex(&summary, (sw - smw) / 2.0, sh / 2.0 + 20.0, TextParams {
        font: Some(font), font_size: ss, color: GRAY, ..Default::default()
    });

    let prompt = "Press ENTER to continue";
    let ps = 16u16;
    let pw = measure_text(prompt, Some(font), ps, 1.0).width;
    draw_text_ex(prompt, (sw - pw) / 2.0, sh / 2.0 + 55.0, TextParams {
        font: Some(font), font_size: ps, color: DARKGRAY, ..Default::default()
    });
}

fn draw_game_won_overlay(font: &Font, bold: &Font, state: &GameState, ow_font: Option<&Font>, ow: &Option<Overworld>) {
    let sw = screen_width();
    let sh = screen_height();
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.85));

    let tfont = ow_font.unwrap_or(bold);

    let title = "GAME WON";
    let ts = 64u16;
    let tw = measure_text(title, Some(tfont), ts, 1.0).width;
    draw_text_ex(title, (sw - tw) / 2.0, sh / 2.0 - 40.0, TextParams {
        font: Some(tfont), font_size: ts, color: hex_to_color("#ffd700"), ..Default::default()
    });

    if let Some(ow) = ow {
        let sub = format!("{} conquered!", ow.name);
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

    let prompt = "Press ENTER for new game";
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

fn render_game(state: &GameState, ui_font: &Font, title_font: Option<&Font>) {
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
    let tiles_x = (map_width / TILE) as i32;
    let tiles_y = (mid_height / TILE) as i32;
    let camera_x = state.player.x - tiles_x / 2;
    let camera_y = state.player.y - tiles_y / 2;
    // Player center in screen coords (for light falloff)
    let player_screen_x = map_left + (state.player.x - camera_x) as f32 * TILE + TILE / 2.0;
    let player_screen_y = mid_top + (state.player.y - camera_y) as f32 * TILE + TILE / 2.0;
    let light_radius = state.vision_radius as f32 * TILE;

    // Tiles
    for sy in 0..=tiles_y {
        for sx in 0..=tiles_x {
            let tx = camera_x + sx;
            let ty = camera_y + sy;
            if tx < 0 || ty < 0 || tx >= state.level.width || ty >= state.level.height {
                continue;
            }
            if !state.level.revealed.contains(&(tx, ty)) {
                continue;
            }

            let tile_name = &state.level.tiles[ty as usize][tx as usize];
            let def = match state.level.tile_defs.get(tile_name) {
                Some(d) => d,
                None => continue,
            };

            let screen_x = map_left + sx as f32 * TILE;
            let screen_y = mid_top + sy as f32 * TILE;

            if screen_y + TILE < mid_top || screen_y > mid_top + mid_height {
                continue;
            }
            if screen_x + TILE > map_width {
                continue;
            }

            let in_vision = state.level.visible.contains(&(tx, ty));

            draw_rectangle(screen_x, screen_y, TILE, TILE, hex_to_color(&def.color));

            if !def.char_display.is_empty() {
                let alpha = if in_vision { 0.27 } else { 0.13 };
                let font_size = (TILE * 0.55) as u16;
                let text = &def.char_display;
                let tm = measure_text(text, None, font_size, 1.0);
                draw_text(
                    text,
                    screen_x + (TILE - tm.width) / 2.0,
                    screen_y + TILE / 2.0 + tm.height / 2.0,
                    font_size as f32,
                    Color::new(1.0, 1.0, 1.0, alpha),
                );
            }

            if !in_vision {
                draw_rectangle(screen_x, screen_y, TILE, TILE, Color::new(0.0, 0.0, 0.0, 0.5));
            }
        }
    }

    // ── Map ambient occlusion: darken walkable tiles near walls ──
    for sy in 0..=tiles_y {
        for sx in 0..=tiles_x {
            let tx = camera_x + sx;
            let ty = camera_y + sy;
            if tx < 0 || ty < 0 || tx >= state.level.width || ty >= state.level.height { continue; }
            if !state.level.revealed.contains(&(tx, ty)) { continue; }
            let tile_name = &state.level.tiles[ty as usize][tx as usize];
            let def = match state.level.tile_defs.get(tile_name) { Some(d) => d, None => continue };
            if !def.walkable { continue; }

            let screen_x = map_left + sx as f32 * TILE;
            let screen_y = mid_top + sy as f32 * TILE;
            if screen_y + TILE < mid_top || screen_y > mid_top + mid_height || screen_x + TILE > map_width { continue; }

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
                draw_rectangle(screen_x, screen_y, TILE, TILE, Color::new(0.0, 0.0, 0.0, ao));
            }
        }
    }

    // Items
    for item in &state.level.items {
        if !state.level.visible.contains(&(item.x, item.y)) { continue; }
        let sx = map_left + (item.x - camera_x) as f32 * TILE;
        let sy = mid_top + (item.y - camera_y) as f32 * TILE;
        if sy + TILE < mid_top || sy > mid_top + mid_height || sx + TILE > map_width { continue; }

        let cx = sx + TILE / 2.0;
        let cy = sy + TILE / 2.0;
        let r = TILE * 0.38;
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

    // Triggered traps
    for trap in &state.level.traps {
        if !trap.triggered { continue; }
        if !state.level.revealed.contains(&(trap.x, trap.y)) { continue; }
        let sx = map_left + (trap.x - camera_x) as f32 * TILE;
        let sy = mid_top + (trap.y - camera_y) as f32 * TILE;
        if sy + TILE < mid_top || sy > mid_top + mid_height || sx + TILE > map_width { continue; }

        let cx = sx + TILE / 2.0;
        let cy = sy + TILE / 2.0;
        let half = TILE * 0.38 * 0.85;
        let trap_fill = Color::new(1.0, 0.0, 0.0, 0.4);
        let trap_line = hex_to_color("#ff4444");
        draw_rectangle(cx - half, cy - half, half * 2.0, half * 2.0, trap_fill);
        draw_line(cx - half + 3.0, cy - half + 3.0, cx + half - 3.0, cy + half - 3.0, 2.0, trap_line);
        draw_line(cx + half - 3.0, cy - half + 3.0, cx - half + 3.0, cy + half - 3.0, 2.0, trap_line);
    }

    // Monsters — hexagons
    for mon in &state.level.monsters {
        if !mon.is_alive() { continue; }
        // Boss is 2x2 — visible if any of its tiles are visible
        let mon_visible = if mon.is_boss {
            state.level.visible.contains(&(mon.x, mon.y))
                || state.level.visible.contains(&(mon.x + 1, mon.y))
                || state.level.visible.contains(&(mon.x, mon.y + 1))
                || state.level.visible.contains(&(mon.x + 1, mon.y + 1))
        } else {
            state.level.visible.contains(&(mon.x, mon.y))
        };
        if !mon_visible { continue; }

        let sx = map_left + (mon.x - camera_x) as f32 * TILE;
        let sy = mid_top + (mon.y - camera_y) as f32 * TILE;
        if sy + TILE < mid_top || sy > mid_top + mid_height || sx + TILE > map_width { continue; }

        // Monsters face toward the player
        let mon_facing = ((state.player.y - mon.y) as f32).atan2((state.player.x - mon.x) as f32);

        if mon.is_boss {
            let cx = sx + TILE;
            let cy = sy + TILE;
            let r = TILE * 0.85;
            let pct = mon.hp as f32 / mon.max_hp as f32;
            let base_color = hex_to_color("#ffd700");
            draw_soft_circle_shadow(cx, cy, r);
            draw_circle(cx, cy, r + 6.0, Color::new(1.0, 0.84, 0.0, 0.06));
            draw_circle(cx, cy, r + 3.0, Color::new(1.0, 0.84, 0.0, 0.12));
            draw_circle(cx, cy, r, Color::new(0.15, 0.15, 0.15, 1.0));
            if pct > 0.0 {
                draw_pie(cx, cy, r, pct, base_color, mon_facing);
            }
        } else {
            let cx = sx + TILE / 2.0;
            let cy = sy + TILE / 2.0;
            let r = TILE * 0.4;
            let pct = mon.hp as f32 / mon.max_hp as f32;
            let base_color = Color::new(0.9, 0.25, 0.25, 1.0);
            draw_soft_circle_shadow(cx, cy, r);
            draw_circle(cx, cy, r, Color::new(0.15, 0.15, 0.15, 1.0));
            if pct > 0.0 {
                draw_pie(cx, cy, r, pct, base_color, mon_facing);
            }
        }
    }

    // Player — shield ring + HP pie + sword triangle, all within one tile
    let px = map_left + (state.player.x - camera_x) as f32 * TILE + TILE / 2.0;
    let py = mid_top + (state.player.y - camera_y) as f32 * TILE + TILE / 2.0;
    let has_shield = state.player.armor != "None";
    let has_sword = state.player.weapon != "Fists";
    let shield_color = Color::new(0.45, 0.55, 0.75, 0.9);

    let outer_r = TILE * 0.42;
    let ring_w = 3.0;
    let inner_r = outer_r - ring_w - 1.0;
    let r = if has_shield { inner_r } else { TILE * 0.35 };
    let shield_outer = if has_shield { outer_r } else { r };

    let hp_pct = if state.player.max_hp > 0 {
        state.player.hp as f32 / state.player.max_hp as f32
    } else {
        0.0
    };

    // Soft shadow for player
    draw_soft_circle_shadow(px, py, shield_outer);

    if has_shield {
        draw_circle(px, py, outer_r, shield_color);
    }

    // Dark background + HP pie
    draw_circle(px, py, r, Color::new(0.15, 0.15, 0.15, 1.0));
    if hp_pct > 0.0 {
        draw_pie(px, py, r, hp_pct, hp_bar_color(hp_pct), state.player.facing);
    }

    // Sword triangle sticking out from the edge in facing direction
    if has_sword {
        let sword_r = r * 0.4;
        let sword_base = shield_outer; // base of triangle sits at the outer edge
        let sx = px + state.player.facing.cos() * sword_base;
        let sy = py + state.player.facing.sin() * sword_base;
        let tip_x = sx + state.player.facing.cos() * sword_r;
        let tip_y = sy + state.player.facing.sin() * sword_r;
        let perp = state.player.facing + std::f32::consts::FRAC_PI_2;
        let base1_x = sx + perp.cos() * sword_r * 0.4;
        let base1_y = sy + perp.sin() * sword_r * 0.4;
        let base2_x = sx - perp.cos() * sword_r * 0.4;
        let base2_y = sy - perp.sin() * sword_r * 0.4;
        draw_triangle(
            Vec2::new(tip_x, tip_y),
            Vec2::new(base1_x, base1_y),
            Vec2::new(base2_x, base2_y),
            Color::new(0.8, 0.8, 0.8, 0.9),
        );
    }

    // ── Smooth radial light falloff (sub-tile grid, not aligned to tiles) ──
    let cell = 6.0_f32; // sub-tile cell size for smooth gradient
    let light_max_alpha = 0.5; // match fog-of-war darkness at edges
    let cx_count = (map_width / cell) as i32 + 1;
    let cy_count = (mid_height / cell) as i32 + 1;
    for cy in 0..cy_count {
        let y = mid_top + cy as f32 * cell;
        if y + cell < mid_top || y > mid_top + mid_height { continue; }
        let dy = y + cell / 2.0 - player_screen_y;
        for cx in 0..cx_count {
            let x = map_left + cx as f32 * cell;
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

    // ── Log panel (right side) ──
    let log_left = sw - log_width;
    draw_rectangle(log_left, mid_top, log_width, mid_height, Color::new(0.04, 0.04, 0.04, 1.0));
    draw_line(log_left, mid_top, log_left, mid_top + mid_height, 1.0, Color::new(0.15, 0.15, 0.15, 1.0));

    let log_font_size = 13u16;
    let line_h = 18.0;
    let log_pad = 12.0;
    let log_text_top = mid_top + log_pad + log_font_size as f32;
    let log_max_w = log_width - log_pad * 2.0;
    // Word-wrap log entries into visual lines
    let entry_gap = 4.0_f32; // small gap between log entries
    let mut wrapped: Vec<(String, String, bool)> = Vec::new(); // (text, color, is_last_line_of_entry)
    for entry in &state.log {
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
    let mut y_cursor = mid_top + mid_height - 8.0;
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

    // ── BOTTOM ROW: Stats + keymap ──
    let bot_y = sh - bottom_height;
    draw_rectangle(0.0, bot_y, sw, bottom_height, Color::new(0.05, 0.05, 0.05, 1.0));
    draw_line(0.0, bot_y, sw, bot_y, 1.0, Color::new(0.13, 0.13, 0.13, 1.0));

    let hp_pct = if state.player.max_hp > 0 {
        state.player.hp as f32 / state.player.max_hp as f32
    } else {
        0.0
    };
    let stats = format!(
        "HP {}/{}  LVL {}  ATK {}  DEF {}  {} / {}  ${}  P{}",
        state.player.hp, state.player.max_hp, state.player.level,
        state.player.attack + state.player.weapon_damage,
        state.player.defense + state.player.armor_defense,
        state.player.weapon, state.player.armor,
        state.player.gold, state.player.potions,
    );
    draw_text_ex(&stats, 16.0, bot_y + 19.0, TextParams {
        font: Some(ui_font), font_size: 14, color: hp_bar_color(hp_pct), ..Default::default()
    });

    let keys = "WASD: move  Bump: attack  P: potion";
    let kw = measure_text(keys, Some(ui_font), 13, 1.0).width;
    draw_text_ex(keys, sw - kw - 16.0, bot_y + 19.0, TextParams {
        font: Some(ui_font), font_size: 13, color: WHITE, ..Default::default()
    });
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
