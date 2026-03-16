mod game;
mod gen;
mod maps;

use game::*;
use macroquad::prelude::*;
use ::rand::Rng;
use std::sync::mpsc;

const TILE: f32 = 24.0;

enum Screen {
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
        ..Default::default()
    }
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

    let mut state = GameState::new();
    let mut screen = Screen::Start;
    let mut gen_rx: Option<mpsc::Receiver<GenMsg>> = None;
    let mut phase_text = String::new();
    let mut phase_detail = String::new();
    let mut confetti: Vec<Confetti> = vec![];
    let mut title_font: Option<Font> = None;
    let mut overworld_font: Option<Font> = None;

    // Overworld state
    let mut overworld: Option<Overworld> = None;
    let mut player_snapshot: Option<Player> = None;
    let mut level_snapshot: Option<(usize, Level, [i32; 2])> = None; // (node_index, level, start) for retry

    // Key repeat for overworld navigation
    let mut nav_hold_time: f64 = 0.0;
    let mut nav_last_fire: f64 = 0.0;
    const NAV_INITIAL_DELAY: f64 = 0.3;
    const NAV_REPEAT_RATE: f64 = 0.15;

    loop {
        clear_background(Color::new(0.04, 0.04, 0.04, 1.0));

        match screen {
            Screen::Start => {
                draw_start_screen(&ui_font, &ui_font_bold);
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                    start_overworld_generation(&mut gen_rx);
                    screen = Screen::GenOverworld;
                    phase_text = "designing overworld".into();
                    phase_detail.clear();
                }
            }

            Screen::GenOverworld => {
                draw_loading_screen(&ui_font, &phase_text, &phase_detail);

                if let Some(rx) = &gen_rx {
                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            GenMsg::Phase(p, d) => {
                                phase_text = p;
                                phase_detail = d;
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
                    phase_text = "designing overworld".into();
                    phase_detail.clear();
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
                        let mut best: Option<usize> = None;
                        let mut best_cosine = -1.0_f32;
                        let cur_x = ow.nodes[cur].x;
                        let cur_y = ow.nodes[cur].y;
                        let dir_len = (dx * dx + dy * dy).sqrt();
                        for &(a, b) in &ow.connections {
                            let neighbor = if a == cur { b } else if b == cur { a } else { continue };
                            if !ow.nodes[neighbor].unlocked { continue; }
                            let nx = ow.nodes[neighbor].x - cur_x;
                            let ny = ow.nodes[neighbor].y - cur_y;
                            let node_len = (nx * nx + ny * ny).sqrt();
                            if node_len < 0.001 { continue; }
                            // Cosine similarity: how aligned is this node with the pressed direction
                            let cosine = (nx * dx + ny * dy) / (node_len * dir_len);
                            if cosine > 0.0 && cosine > best_cosine {
                                best_cosine = cosine;
                                best = Some(neighbor);
                            }
                        }
                        if let Some(next) = best {
                            ow.current_node = next;
                        }
                    }

                    if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                        let node = &ow.nodes[ow.current_node];
                        if node.unlocked && !node.completed {
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
                                state.log("Welcome. Your task: find and defeat the boss.", "#666");
                                screen = Screen::Playing;
                            } else {
                                start_level_generation(&state, ow, &mut gen_rx);
                                screen = Screen::GenLevel;
                                phase_text = "designing level".into();
                                phase_detail.clear();
                            }
                        }
                    }
                }
            }

            Screen::GenLevel => {
                draw_loading_screen(&ui_font, &phase_text, &phase_detail);

                if let Some(rx) = &gen_rx {
                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            GenMsg::Phase(p, d) => {
                                phase_text = p;
                                phase_detail = d;
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
                                state.log("Welcome. Your task: find and defeat the boss.", "#666");
                                if let Some(bytes) = font_bytes {
                                    match load_ttf_font_from_bytes(&bytes) {
                                        Ok(f) => title_font = Some(f),
                                        Err(e) => eprintln!("Font load error: {}", e),
                                    }
                                } else {
                                    title_font = None;
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
                        start_level_generation(&state, ow, &mut gen_rx);
                        phase_text = "creating universe".into();
                        phase_detail.clear();
                    }
                }
            }

            Screen::Playing => {
                handle_playing_input(&mut state, &mut screen, &mut confetti);
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
) {
    if is_key_pressed(KeyCode::P) {
        use_potion(state);
    }

    let mut dx = 0i32;
    let mut dy = 0i32;
    if is_key_pressed(KeyCode::W) || is_key_pressed(KeyCode::Up) {
        dy = -1;
    }
    if is_key_pressed(KeyCode::S) || is_key_pressed(KeyCode::Down) {
        dy = 1;
    }
    if is_key_pressed(KeyCode::A) || is_key_pressed(KeyCode::Left) {
        dx = -1;
    }
    if is_key_pressed(KeyCode::D) || is_key_pressed(KeyCode::Right) {
        dx = 1;
    }

    if dx != 0 || dy != 0 {
        let result = try_move(state, dx, dy);
        let moved = result["moved"].as_bool().unwrap_or(false);
        let combat = result["combat"].as_bool().unwrap_or(false);
        if moved || combat {
            monster_turns(state);
        }

        if state.victory {
            spawn_confetti(confetti);
            *screen = Screen::Victory;
        } else if state.game_over {
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
        match gen::generate_overworld(|phase| {
            let _ = tx.send(GenMsg::Phase(phase.phase, phase.detail));
        }) {
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
        match gen::generate_level(&config, &player, |phase| {
            let _ = tx.send(GenMsg::Phase(phase.phase, phase.detail));
        }) {
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

// ── Screens ──

fn draw_start_screen(font: &Font, bold: &Font) {
    let sw = screen_width();
    let sh = screen_height();

    let title = "SCAPEGRACE";
    let title_size = 56u16;
    let tw = measure_text(title, Some(bold), title_size, 1.0).width;
    draw_text_ex(title, (sw - tw) / 2.0, sh / 2.0 - 30.0, TextParams {
        font: Some(bold), font_size: title_size, color: hex_to_color("#e94560"), ..Default::default()
    });

    let prompt = "Press ENTER to start";
    let ps = 18u16;
    let pw = measure_text(prompt, Some(font), ps, 1.0).width;
    draw_text_ex(prompt, (sw - pw) / 2.0, sh / 2.0 + 30.0, TextParams {
        font: Some(font), font_size: ps, color: GRAY, ..Default::default()
    });
}

fn draw_loading_screen(font: &Font, phase_text: &str, phase_detail: &str) {
    let sw = screen_width();
    let sh = screen_height();

    // Spinner
    let time = get_time() as f32;
    let cx = sw / 2.0;
    let cy = sh / 2.0 - 30.0;
    let angle = time * 5.0;
    let r = 16.0;
    for i in 0..8 {
        let a = angle + i as f32 * std::f32::consts::TAU / 8.0;
        let alpha = 1.0 - i as f32 * 0.12;
        draw_circle(
            cx + a.cos() * r,
            cy + a.sin() * r,
            3.5,
            Color::new(0.91, 0.27, 0.37, alpha),
        );
    }

    let ps = 20u16;
    let ptw = measure_text(phase_text, Some(font), ps, 1.0).width;
    draw_text_ex(phase_text, (sw - ptw) / 2.0, sh / 2.0 + 20.0, TextParams {
        font: Some(font), font_size: ps, color: GRAY, ..Default::default()
    });

    if !phase_detail.is_empty() {
        let ds = 16u16;
        let pdw = measure_text(phase_detail, Some(font), ds, 1.0).width;
        draw_text_ex(phase_detail, (sw - pdw) / 2.0, sh / 2.0 + 48.0, TextParams {
            font: Some(font), font_size: ds, color: DARKGRAY, ..Default::default()
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
    let map_top = top_bar + 20.0;
    let map_bottom = sh - bottom_bar;
    let map_w = map_right - map_left;
    let map_h = map_bottom - map_top;

    // Title
    let tfont = ow_font.unwrap_or(ui_bold);
    let ts = 36u16;
    let tw = measure_text(&ow.name, Some(tfont), ts, 1.0).width;
    draw_text_ex(&ow.name, (sw - tw) / 2.0, 50.0, TextParams {
        font: Some(tfont), font_size: ts, color: hex_to_color("#e0d5c0"), ..Default::default()
    });

    // Description
    let ds = 16u16;
    let dw = measure_text(&ow.description, Some(ui_font), ds, 1.0).width;
    draw_text_ex(&ow.description, (sw - dw) / 2.0, 78.0, TextParams {
        font: Some(ui_font), font_size: ds, color: DARKGRAY, ..Default::default()
    });

    // Helper to convert node coords to screen coords
    let node_screen = |n: &OverworldNode| -> (f32, f32) {
        (map_left + n.x * map_w, map_top + n.y * map_h)
    };

    // Draw connections (thick lines)
    for &(a, b) in &ow.connections {
        if a >= ow.nodes.len() || b >= ow.nodes.len() { continue; }
        let (ax, ay) = node_screen(&ow.nodes[a]);
        let (bx, by) = node_screen(&ow.nodes[b]);
        let line_color = if ow.nodes[a].unlocked && ow.nodes[b].unlocked {
            Color::new(0.5, 0.5, 0.5, 0.8)
        } else {
            Color::new(0.25, 0.25, 0.25, 0.5)
        };
        draw_line(ax, ay, bx, by, 3.0, line_color);
    }

    // Draw nodes
    let time = get_time() as f32;
    for (i, node) in ow.nodes.iter().enumerate() {
        let (nx, ny) = node_screen(node);
        let r = 18.0;

        if node.completed {
            // Green filled
            draw_circle(nx, ny, r, hex_to_color("#44ff44"));
            draw_circle_lines(nx, ny, r, 2.5, hex_to_color("#228822"));
        } else if node.unlocked {
            // White outline, dark fill (red outline for final level)
            draw_circle(nx, ny, r, Color::new(0.15, 0.15, 0.15, 1.0));
            if node.is_final {
                draw_circle_lines(nx, ny, r, 3.0, hex_to_color("#e94560"));
            } else {
                draw_circle_lines(nx, ny, r, 2.5, WHITE);
            }
        } else {
            // Gray (locked, red outline for final level)
            draw_circle(nx, ny, r, Color::new(0.2, 0.2, 0.2, 1.0));
            if node.is_final {
                draw_circle_lines(nx, ny, r, 3.0, hex_to_color("#e94560"));
            } else {
                draw_circle_lines(nx, ny, r, 2.0, Color::new(0.35, 0.35, 0.35, 1.0));
            }
        }

        // Current node: pulsing player circle
        if i == ow.current_node {
            let pulse = (time * 3.0).sin() * 0.3 + 0.7;
            let pulse_color = Color::new(0.91, 0.27, 0.37, pulse);
            draw_circle(nx, ny, r * 0.55, pulse_color);
            draw_circle_lines(nx, ny, r + 4.0, 2.0, Color::new(0.91, 0.27, 0.37, pulse * 0.5));
        }

        // Level name below unlocked nodes
        if node.unlocked {
            let ns = 14u16;
            let nw = measure_text(&node.name, Some(ui_font), ns, 1.0).width;
            draw_text_ex(&node.name, nx - nw / 2.0, ny + r + 18.0, TextParams {
                font: Some(ui_font), font_size: ns, color: WHITE, ..Default::default()
            });
        }
    }

    // Bottom info
    let current = &ow.nodes[ow.current_node];
    let info = if current.completed {
        format!("{} — COMPLETED", current.name)
    } else {
        format!("{} — {}", current.name, current.description)
    };
    let is = 16u16;
    let iw = measure_text(&info, Some(ui_font), is, 1.0).width;
    draw_text_ex(&info, (sw - iw) / 2.0, sh - 38.0, TextParams {
        font: Some(ui_font), font_size: is, color: hex_to_color("#e0d5c0"), ..Default::default()
    });

    let hint = if current.completed {
        "Arrows: navigate"
    } else {
        "Arrows: navigate  |  ENTER: play level"
    };
    let hs = 14u16;
    let hw = measure_text(hint, Some(ui_font), hs, 1.0).width;
    draw_text_ex(hint, (sw - hw) / 2.0, sh - 16.0, TextParams {
        font: Some(ui_font), font_size: hs, color: DARKGRAY, ..Default::default()
    });
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
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.7));

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
    let top_height = 128.0;
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
        let ts = 38u16;
        let tw = measure_text(&state.level.title, Some(tfont), ts, 1.0).width;
        draw_text_ex(&state.level.title, (sw - tw) / 2.0, 58.0, TextParams {
            font: Some(tfont), font_size: ts, color: hex_to_color("#e0d5c0"), ..Default::default()
        });

        if !state.level.description.is_empty() {
            let ds = 19u16;
            let dw = measure_text(&state.level.description, Some(tfont), ds, 1.0).width;
            draw_text_ex(&state.level.description, (sw - dw) / 2.0, 95.0, TextParams {
                font: Some(tfont), font_size: ds, color: Color::new(0.45, 0.45, 0.45, 1.0), ..Default::default()
            });
        }
    }

    // ── MIDDLE ROW: Map (left) + Log (right) ──

    let map_left = 0.0;
    let tiles_x = (map_width / TILE) as i32;
    let tiles_y = (mid_height / TILE) as i32;
    let camera_x = state.player.x - tiles_x / 2;
    let camera_y = state.player.y - tiles_y / 2;
    let vision_r = state.vision_radius as f32;

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

            let dist = (((tx - state.player.x) as f32).powi(2)
                + ((ty - state.player.y) as f32).powi(2))
            .sqrt();
            let in_vision = dist <= vision_r;

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

            draw_rectangle_lines(screen_x, screen_y, TILE, TILE, 1.0, Color::new(1.0, 1.0, 1.0, 0.024));
        }
    }

    // Items
    for item in &state.level.items {
        if !state.level.revealed.contains(&(item.x, item.y)) { continue; }
        let dist = (((item.x - state.player.x) as f32).powi(2)
            + ((item.y - state.player.y) as f32).powi(2)).sqrt();
        if dist > vision_r { continue; }
        let sx = map_left + (item.x - camera_x) as f32 * TILE;
        let sy = mid_top + (item.y - camera_y) as f32 * TILE;
        if sy + TILE < mid_top || sy > mid_top + mid_height || sx + TILE > map_width { continue; }

        let cx = sx + TILE / 2.0;
        let cy = sy + TILE / 2.0;
        let r = TILE * 0.38;
        let color = item_color(&item.item_type);

        match item.item_type.as_str() {
            "weapon" => {
                draw_poly(cx, cy, 3, r, 0.0, color);
            }
            "armor" => {
                draw_circle(cx, cy, r, color);
            }
            _ => {
                let half = r * 0.85;
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
        if !state.level.revealed.contains(&(mon.x, mon.y)) { continue; }
        let dist = (((mon.x - state.player.x) as f32).powi(2)
            + ((mon.y - state.player.y) as f32).powi(2)).sqrt();
        let extra = if mon.is_boss { 1.0 } else { 0.0 };
        if dist > vision_r + extra { continue; }

        let sx = map_left + (mon.x - camera_x) as f32 * TILE;
        let sy = mid_top + (mon.y - camera_y) as f32 * TILE;
        if sy + TILE < mid_top || sy > mid_top + mid_height || sx + TILE > map_width { continue; }

        if mon.is_boss {
            let cx = sx + TILE;
            let cy = sy + TILE;
            let r = TILE * 0.85;
            let pct = mon.hp as f32 / mon.max_hp as f32;
            let base_color = hex_to_color("#ffd700");
            // Glow
            draw_circle(cx, cy, r + 6.0, Color::new(1.0, 0.84, 0.0, 0.06));
            draw_circle(cx, cy, r + 3.0, Color::new(1.0, 0.84, 0.0, 0.12));
            // Pie chart
            draw_circle(cx, cy, r, Color::new(0.15, 0.15, 0.15, 1.0));
            if pct > 0.0 {
                draw_pie(cx, cy, r, pct, base_color);
            }
        } else {
            let cx = sx + TILE / 2.0;
            let cy = sy + TILE / 2.0;
            let r = TILE * 0.4;
            let pct = mon.hp as f32 / mon.max_hp as f32;
            let base_color = Color::new(0.9, 0.25, 0.25, 1.0);
            // Pie chart
            draw_circle(cx, cy, r, Color::new(0.15, 0.15, 0.15, 1.0));
            if pct > 0.0 {
                draw_pie(cx, cy, r, pct, base_color);
            }
        }
    }

    // Player — HP pie chart
    let px = map_left + (state.player.x - camera_x) as f32 * TILE + TILE / 2.0;
    let py = mid_top + (state.player.y - camera_y) as f32 * TILE + TILE / 2.0;
    let r = TILE * 0.35;
    let hp_pct = if state.player.max_hp > 0 {
        state.player.hp as f32 / state.player.max_hp as f32
    } else {
        0.0
    };
    draw_circle(px, py, r, Color::new(0.15, 0.15, 0.15, 1.0));
    if hp_pct > 0.0 {
        draw_pie(px, py, r, hp_pct, hp_bar_color(hp_pct));
    }

    // ── Log panel (right side) ──
    let log_left = sw - log_width;
    draw_rectangle(log_left, mid_top, log_width, mid_height, Color::new(0.04, 0.04, 0.04, 1.0));
    draw_line(log_left, mid_top, log_left, mid_top + mid_height, 1.0, Color::new(0.15, 0.15, 0.15, 1.0));

    let log_font_size = 13u16;
    let line_h = 18.0;
    let log_text_top = mid_top + 14.0;
    let max_log_lines = ((mid_height - 20.0) / line_h) as usize;
    let log_start = state.log.len().saturating_sub(max_log_lines);
    for (i, entry) in state.log[log_start..].iter().enumerate() {
        draw_text_ex(&entry.text, log_left + 12.0, log_text_top + i as f32 * line_h, TextParams {
            font: Some(ui_font), font_size: log_font_size, color: hex_to_color(&entry.color), ..Default::default()
        });
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

fn draw_pie(cx: f32, cy: f32, r: f32, pct: f32, color: Color) {
    let segments = 32;
    let angle_span = pct.clamp(0.0, 1.0) * std::f32::consts::TAU;
    let start_angle = -std::f32::consts::FRAC_PI_2; // 12 o'clock
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
