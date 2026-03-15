mod game;
mod gen;
mod maps;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, Json},
    routing::get,
    Router,
};
use game::*;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::services::ServeDir;

struct AppState {
    debug: Mutex<Option<DebugSnapshot>>,
    api_game: Mutex<Option<GameState>>,
}
type SharedState = Arc<AppState>;

#[derive(Clone, serde::Serialize)]
struct DebugSnapshot {
    player: Player,
    tiles: Vec<Vec<String>>,
    tile_defs: std::collections::HashMap<String, TileDef>,
    width: i32,
    height: i32,
    monsters: Vec<Monster>,
    items: Vec<Item>,
    stairs: [i32; 2],
    title: String,
    description: String,
    font: String,
    revealed_count: usize,
    reachable_from_player: usize,
    // ASCII rendering of the full map
    map_ascii: String,
}

fn text_msg(v: serde_json::Value) -> Message {
    Message::Text(v.to_string().into())
}

fn snapshot(state: &GameState) -> DebugSnapshot {
    // Build ASCII map
    let mut ascii = String::new();
    // Reverse lookup: tile name → first char key
    let name_to_char: std::collections::HashMap<&str, char> = state.level.tile_defs.iter()
        .map(|(name, def)| {
            if !def.walkable { (name.as_str(), '#') }
            else if !def.char_display.is_empty() { (name.as_str(), def.char_display.chars().next().unwrap_or('.')) }
            else { (name.as_str(), '.') }
        })
        .collect();

    for y in 0..state.level.height {
        for x in 0..state.level.width {
            if x == state.player.x && y == state.player.y {
                ascii.push('@');
            } else if state.level.monsters.iter().any(|m| m.x == x && m.y == y && m.is_alive() && m.is_boss) {
                ascii.push('B');
            } else if state.level.monsters.iter().any(|m| m.x == x && m.y == y && m.is_alive()) {
                ascii.push('M');
            } else if state.level.items.iter().any(|it| it.x == x && it.y == y) {
                ascii.push('!');
            } else {
                let tile = &state.level.tiles[y as usize][x as usize];
                let ch = name_to_char.get(tile.as_str()).copied().unwrap_or('?');
                ascii.push(ch);
            }
        }
        ascii.push('\n');
    }

    // Count reachable tiles from player
    let reachable = gen::reachable_from(
        &state.level.tiles, &state.level.tile_defs,
        state.player.x, state.player.y,
        state.level.width, state.level.height,
    );

    DebugSnapshot {
        player: state.player.clone(),
        tiles: state.level.tiles.clone(),
        tile_defs: state.level.tile_defs.clone(),
        width: state.level.width,
        height: state.level.height,
        monsters: state.level.monsters.clone(),
        items: state.level.items.clone(),
        stairs: [0, 0], // deprecated
        title: state.level.title.clone(),
        description: state.level.description.clone(),
        font: state.level.font.clone(),
        revealed_count: state.level.revealed.len(),
        reachable_from_player: reachable,
        map_ascii: ascii,
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let static_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static");
    let shared: SharedState = Arc::new(AppState {
        debug: Mutex::new(None),
        api_game: Mutex::new(None),
    });

    let app = Router::new()
        .route("/", get(index))
        .route("/ws", get(ws_handler))
        .route("/debug", get(debug_handler))
        .route("/api/start", axum::routing::post(api_start))
        .route("/api/move", axum::routing::post(api_move))
        .route("/api/potion", axum::routing::post(api_potion))
        .route("/api/state", get(api_state))
        .nest_service("/static", ServeDir::new(static_dir))
        .with_state(shared);

    let addr = "0.0.0.0:3000";
    println!("Scapegrace running at http://localhost:3000");
    println!("Debug: http://localhost:3000/debug");
    println!("API: POST /api/start, POST /api/move, POST /api/potion, GET /api/state");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn index() -> Html<String> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static/index.html");
    Html(std::fs::read_to_string(path).unwrap_or_else(|_| "<h1>Scapegrace</h1>".into()))
}

async fn debug_handler(State(shared): State<SharedState>) -> Json<serde_json::Value> {
    let snap = shared.debug.lock().await;
    match &*snap {
        Some(s) => Json(serde_json::to_value(s).unwrap_or(serde_json::json!({"error": "serialize failed"}))),
        None => Json(serde_json::json!({"error": "no active game"})),
    }
}

async fn ws_handler(ws: WebSocketUpgrade, State(shared): State<SharedState>) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_socket(socket, shared))
}

async fn handle_socket(mut socket: WebSocket, shared: SharedState) {
    let mut state = GameState::new();

    // Wait for messages
    while let Some(Ok(msg)) = socket.recv().await {
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => break,
            _ => continue,
        };

        let cmd: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        match cmd["type"].as_str().unwrap_or("") {
            "start" | "restart" => {
                if cmd["type"].as_str() == Some("restart") {
                    state = GameState::new();
                }
                let (tx, mut rx) = tokio::sync::mpsc::channel::<gen::PhaseUpdate>(10);
                let floor = 1;
                let player_clone = state.player.clone();
                let budget = state.budget;

                let mut gen_handle = tokio::spawn(async move {
                    gen::generate_level(floor, &player_clone, budget, move |phase| {
                        let _ = tx.try_send(phase);
                    }).await
                });

                // Forward phase updates to client while generation runs
                loop {
                    tokio::select! {
                        Some(phase) = rx.recv() => {
                            let _ = socket.send(text_msg(serde_json::json!({
                                "type": "phase",
                                "phase": phase.phase,
                                "detail": phase.detail,
                            }))).await;
                        }
                        result = &mut gen_handle => {
                            // Drain remaining phases
                            while let Ok(phase) = rx.try_recv() {
                                let _ = socket.send(text_msg(serde_json::json!({
                                    "type": "phase",
                                    "phase": phase.phase,
                                    "detail": phase.detail,
                                }))).await;
                            }
                            match result {
                                Ok(Ok((level, start, remaining))) => {
                                    state.level = level;
                                    state.player.x = start[0];
                                    state.player.y = start[1];
                                    state.player.floor = floor;
                                    state.budget = remaining + 40;
                                    let newly = reveal_around(&mut state.level, state.player.x, state.player.y, state.vision_radius);
                                    state.log("Find and defeat the boss.", "#666");
                                    let _ = socket.send(text_msg(build_full_state(&state, &newly))).await;
                                    *shared.debug.lock().await = Some(snapshot(&state));
                                }
                                Ok(Err(e)) => {
                                    let _ = socket.send(text_msg(serde_json::json!({"type": "error", "message": e}))).await;
                                }
                                Err(e) => {
                                    let _ = socket.send(text_msg(serde_json::json!({"type": "error", "message": e.to_string()}))).await;
                                }
                            }
                            break;
                        }
                    }
                }
            }
            "move" => {
                if state.game_over { continue; }
                let dx = cmd["dx"].as_i64().unwrap_or(0) as i32;
                let dy = cmd["dy"].as_i64().unwrap_or(0) as i32;

                let result = try_move(&mut state, dx, dy);
                let moved = result["moved"].as_bool().unwrap_or(false);
                let combat = result["combat"].as_bool().unwrap_or(false);

                if moved || combat {
                    let mon_moves = monster_turns(&mut state);
                    let _ = socket.send(text_msg(build_update(&state, &result, &mon_moves))).await;
                } else {
                    let _ = socket.send(text_msg(build_update(&state, &result, &[]))).await;
                }
                *shared.debug.lock().await = Some(snapshot(&state));
            }
            "potion" => {
                if state.game_over { continue; }
                use_potion(&mut state);
                let _ = socket.send(text_msg(serde_json::json!({
                    "type": "update",
                    "player": &state.player,
                    "log": &state.log,
                    "game_over": state.game_over,
                }))).await;
            }
            // "restart" is handled by "start" | "restart" above
            _ => {}
        }
    }
}

fn build_full_state(state: &GameState, revealed: &[[i32; 2]]) -> serde_json::Value {
    serde_json::json!({
        "type": "full_state",
        "tiles": state.level.tiles,
        "tile_defs": state.level.tile_defs,
        "width": state.level.width,
        "height": state.level.height,
        "player": state.player,
        "monsters": state.level.monsters.iter().filter(|m| m.is_alive()).collect::<Vec<_>>(),
        "items": state.level.items,
        "traps": state.level.traps.iter().filter(|t| t.triggered).collect::<Vec<_>>(),
        "revealed": revealed,
        "title": state.level.title,
        "description": state.level.description,
        "font": state.level.font,
        "log": state.log,
        "game_over": state.game_over,
        "victory": state.victory,
    })
}

fn build_update(state: &GameState, move_result: &serde_json::Value, monster_moves: &[serde_json::Value]) -> serde_json::Value {
    serde_json::json!({
        "type": "update",
        "move_result": move_result,
        "player": state.player,
        "monsters": state.level.monsters.iter().filter(|m| m.is_alive()).collect::<Vec<_>>(),
        "items": state.level.items,
        "traps": state.level.traps.iter().filter(|t| t.triggered).collect::<Vec<_>>(),
        "monster_moves": monster_moves,
        "log": state.log,
        "game_over": state.game_over,
        "victory": state.victory,
    })
}

// ── HTTP API for MCP testing ──

async fn api_start(State(shared): State<SharedState>) -> Json<serde_json::Value> {
    let mut state = GameState::new();
    match gen::generate_level(1, &state.player, state.budget, |_| {}).await {
        Ok((level, start, remaining)) => {
            state.level = level;
            state.player.x = start[0];
            state.player.y = start[1];
            state.player.floor = 1;
            state.budget = remaining + 40;
            reveal_around(&mut state.level, state.player.x, state.player.y, state.vision_radius);
            let result = api_snapshot(&state);
            *shared.api_game.lock().await = Some(state);
            Json(result)
        }
        Err(e) => Json(serde_json::json!({"error": e})),
    }
}

async fn api_move(
    State(shared): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let mut guard = shared.api_game.lock().await;
    let state = match guard.as_mut() {
        Some(s) => s,
        None => return Json(serde_json::json!({"error": "no game started, POST /api/start first"})),
    };
    let dx = body["dx"].as_i64().unwrap_or(0) as i32;
    let dy = body["dy"].as_i64().unwrap_or(0) as i32;
    try_move(state, dx, dy);
    if !state.game_over {
        monster_turns(state);
    }
    Json(api_snapshot(state))
}

async fn api_potion(State(shared): State<SharedState>) -> Json<serde_json::Value> {
    let mut guard = shared.api_game.lock().await;
    let state = match guard.as_mut() {
        Some(s) => s,
        None => return Json(serde_json::json!({"error": "no game started"})),
    };
    use_potion(state);
    Json(api_snapshot(state))
}

async fn api_state(State(shared): State<SharedState>) -> Json<serde_json::Value> {
    let guard = shared.api_game.lock().await;
    match guard.as_ref() {
        Some(state) => Json(api_snapshot(state)),
        None => Json(serde_json::json!({"error": "no game started"})),
    }
}

fn api_snapshot(state: &GameState) -> serde_json::Value {
    let mut ascii = String::new();
    let name_to_char: std::collections::HashMap<&str, char> = state.level.tile_defs.iter()
        .map(|(name, def)| {
            if !def.walkable { (name.as_str(), '#') }
            else if !def.char_display.is_empty() { (name.as_str(), def.char_display.chars().next().unwrap_or('.')) }
            else { (name.as_str(), '.') }
        })
        .collect();

    for y in 0..state.level.height {
        for x in 0..state.level.width {
            if x == state.player.x && y == state.player.y {
                ascii.push('@');
            } else if state.level.monsters.iter().any(|m| m.x == x && m.y == y && m.is_alive() && m.is_boss) {
                ascii.push('B');
            } else if state.level.monsters.iter().any(|m| m.x == x && m.y == y && m.is_alive()) {
                ascii.push('M');
            } else if state.level.items.iter().any(|it| it.x == x && it.y == y) {
                ascii.push('!');
            } else {
                let tile = &state.level.tiles[y as usize][x as usize];
                ascii.push(name_to_char.get(tile.as_str()).copied().unwrap_or('?'));
            }
        }
        ascii.push('\n');
    }

    serde_json::json!({
        "map": ascii,
        "title": state.level.title,
        "player": {
            "x": state.player.x, "y": state.player.y,
            "hp": state.player.hp, "max_hp": state.player.max_hp,
            "attack": state.player.attack + state.player.weapon_damage,
            "defense": state.player.defense + state.player.armor_defense,
            "level": state.player.level,
            "weapon": state.player.weapon,
            "armor": state.player.armor,
            "potions": state.player.potions,
            "gold": state.player.gold,
        },
        "monsters": state.level.monsters.iter()
            .filter(|m| m.is_alive())
            .map(|m| serde_json::json!({
                "name": m.name, "x": m.x, "y": m.y,
                "hp": m.hp, "max_hp": m.max_hp,
                "is_boss": m.is_boss,
            }))
            .collect::<Vec<_>>(),
        "items": state.level.items.iter()
            .map(|it| serde_json::json!({
                "name": it.name, "x": it.x, "y": it.y, "type": it.item_type,
            }))
            .collect::<Vec<_>>(),
        "log": state.log.iter().map(|l| &l.text).collect::<Vec<_>>(),
        "game_over": state.game_over,
        "victory": state.victory,
    })
}
