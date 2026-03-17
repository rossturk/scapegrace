use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ── Types ──

#[derive(Clone, Serialize, Deserialize)]
pub struct TileDef {
    pub name: String,
    pub color: String,
    pub walkable: bool,
    pub char_display: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Monster {
    pub id: String,
    pub name: String,
    pub sprite: String,
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub xp_value: i32,
    pub description: String,
    #[serde(default)]
    pub is_boss: bool,
}

impl Monster {
    pub fn is_alive(&self) -> bool {
        self.hp > 0
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub sprite: String,
    pub x: i32,
    pub y: i32,
    pub item_type: String, // weapon, armor, potion, gold
    pub value: i32,
    pub description: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Player {
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub xp: i32,
    pub level: i32,
    pub xp_to_next: i32,
    pub gold: i32,
    pub weapon: String,
    pub weapon_damage: i32,
    pub armor: String,
    pub armor_defense: i32,
    pub potions: i32,
    pub floor: i32,
    pub facing: f32, // radians, 0 = right, PI/2 = down
}

impl Default for Player {
    fn default() -> Self {
        Self {
            x: 0, y: 0,
            hp: 30, max_hp: 30,
            attack: 5, defense: 2,
            xp: 0, level: 1, xp_to_next: 20,
            gold: 0,
            weapon: "Fists".into(), weapon_damage: 0,
            armor: "None".into(), armor_defense: 0,
            potions: 1, floor: 1, facing: -std::f32::consts::FRAC_PI_2,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub text: String,
    pub color: String,
}

#[derive(Clone, Serialize)]
pub struct Level {
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<Vec<String>>,
    pub tile_defs: std::collections::HashMap<String, TileDef>,
    pub monsters: Vec<Monster>,
    pub items: Vec<Item>,
    pub traps: Vec<Trap>,
    pub title: String,
    pub description: String,
    pub font: String,
    pub scale: Vec<f32>,  // frequencies for footstep notes
    #[serde(skip)]
    pub revealed: HashSet<(i32, i32)>,
    #[serde(skip)]
    pub visible: HashSet<(i32, i32)>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Trap {
    pub x: i32,
    pub y: i32,
    pub damage: i32,
    pub name: String,
    pub triggered: bool,
}

#[derive(Clone)]
pub struct Overworld {
    pub name: String,
    pub font: String,
    pub description: String,
    pub nodes: Vec<OverworldNode>,
    pub connections: Vec<(usize, usize)>,
    pub current_node: usize,
}

#[derive(Clone)]
pub struct OverworldNode {
    pub name: String,
    pub font: String,
    pub description: String,
    pub theme: String,
    pub palette: Vec<String>,
    pub budget: i32,
    pub x: f32,
    pub y: f32,
    pub completed: bool,
    pub unlocked: bool,
    pub is_final: bool,
}

pub struct GameState {
    pub player: Player,
    pub level: Level,
    pub log: Vec<LogEntry>,
    pub game_over: bool,
    pub victory: bool,
    pub vision_radius: i32,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            player: Player::default(),
            level: Level {
                width: 40, height: 24,
                tiles: vec![], tile_defs: Default::default(),
                monsters: vec![], items: vec![], traps: vec![],
                title: String::new(), description: String::new(), font: String::new(),
                scale: vec![], revealed: HashSet::new(), visible: HashSet::new(),
            },
            log: vec![],
            game_over: false,
            victory: false,
            vision_radius: 5,
        }
    }

    pub fn log(&mut self, text: &str, color: &str) {
        self.log.push(LogEntry { text: text.into(), color: color.into() });
        if self.log.len() > 50 {
            self.log.drain(0..self.log.len() - 50);
        }
    }
}

// ── Combat ──

fn attack_roll(atk: i32, def: i32) -> (i32, bool) {
    let mut rng = rand::thread_rng();
    let base = (atk - def / 2).max(1);
    let roll = rng.gen_range(1..=20);
    if roll == 20 {
        (base * 2, true)
    } else if roll == 1 {
        (0, false)
    } else {
        let damage = (base + rng.gen_range(-2..=2)).max(1);
        (damage, false)
    }
}

pub fn player_attack(state: &mut GameState, monster_idx: usize) -> bool {
    let total_atk = state.player.attack + state.player.weapon_damage;
    let (damage, crit) = attack_roll(total_atk, state.level.monsters[monster_idx].defense);

    let mon_name = state.level.monsters[monster_idx].name.clone();

    if damage == 0 {
        state.log(&format!("You miss the {}!", mon_name), "#888");
        return false;
    }

    state.level.monsters[monster_idx].hp -= damage;

    if crit {
        state.log(&format!("CRITICAL! You hit {} for {} damage!", mon_name, damage), "#ff4444");
    } else {
        state.log(&format!("You hit {} for {} damage.", mon_name, damage), "#ffaa44");
    }

    if state.level.monsters[monster_idx].hp <= 0 {
        let xp = state.level.monsters[monster_idx].xp_value;
        let is_boss = state.level.monsters[monster_idx].is_boss;
        state.log(&format!("You defeated the {}! (+{} XP)", mon_name, xp), "#44ff44");
        state.player.xp += xp;
        check_level_up(state);
        maybe_drop_loot(state, monster_idx);
        if is_boss {
            state.log("THE BOSS IS SLAIN!", "#ffd700");
            state.victory = true;
        }
        return true;
    } else {
        let hp = state.level.monsters[monster_idx].hp;
        let max_hp = state.level.monsters[monster_idx].max_hp;
        state.log(&format!("  {}: {}/{} HP", mon_name, hp, max_hp), "#888");
    }
    false
}

fn monster_attack(state: &mut GameState, monster_idx: usize) {
    let mon = &state.level.monsters[monster_idx];
    if !mon.is_alive() { return; }
    let total_def = state.player.defense + state.player.armor_defense;
    let (damage, crit) = attack_roll(mon.attack, total_def);
    let mon_name = mon.name.clone();

    if damage == 0 {
        state.log(&format!("The {} misses!", mon_name), "#888");
        return;
    }
    state.player.hp -= damage;
    if crit {
        state.log(&format!("The {} CRITS you for {}!", mon_name, damage), "#ff2222");
    } else {
        state.log(&format!("The {} hits you for {}.", mon_name, damage), "#ff8844");
    }
    if state.player.hp <= 0 {
        state.log("You have died.", "#ff0000");
        state.game_over = true;
    }
}

fn check_level_up(state: &mut GameState) {
    while state.player.xp >= state.player.xp_to_next {
        state.player.xp -= state.player.xp_to_next;
        state.player.level += 1;
        state.player.max_hp += 5;
        state.player.hp = state.player.max_hp;
        state.player.attack += 1;
        state.player.defense += 1;
        state.player.xp_to_next = (state.player.xp_to_next as f64 * 1.5) as i32;
        state.log(&format!("LEVEL UP! You are now level {}!", state.player.level), "#ffff44");
        state.log("  HP +5, ATK +1, DEF +1", "#ffff44");
    }
}

fn maybe_drop_loot(state: &mut GameState, monster_idx: usize) {
    let mut rng = rand::thread_rng();
    if rng.gen::<f64>() > 0.4 { return; }

    let mon = &state.level.monsters[monster_idx];
    let (mx, my) = (mon.x, mon.y);
    let mon_id = mon.id.clone();
    let mon_name = mon.name.clone();
    let xp_val = mon.xp_value;

    let roll: f64 = rng.gen();
    let item = if roll < 0.5 {
        let gold = rng.gen_range(1..=xp_val.max(1));
        Item {
            id: format!("drop_{}", mon_id),
            name: format!("{} Gold", gold),
            sprite: "💰".into(), x: mx, y: my,
            item_type: "gold".into(), value: gold,
            description: String::new(),
        }
    } else {
        Item {
            id: format!("drop_{}", mon_id),
            name: "Health Potion".into(),
            sprite: "🧪".into(), x: mx, y: my,
            item_type: "potion".into(), value: 0,
            description: String::new(),
        }
    };

    state.log(&format!("The {} dropped {}!", mon_name, item.name), "#ab47bc");
    state.level.items.push(item);
}

// ── Movement ──

pub fn try_move(state: &mut GameState, dx: i32, dy: i32) -> serde_json::Value {
    let nx = state.player.x + dx;
    let ny = state.player.y + dy;

    if nx < 0 || ny < 0 || nx >= state.level.width || ny >= state.level.height {
        return serde_json::json!({"moved": false});
    }

    let tile = &state.level.tiles[ny as usize][nx as usize];
    if let Some(td) = state.level.tile_defs.get(tile) {
        if !td.walkable {
            return serde_json::json!({"moved": false});
        }
    }

    // Check monster (bosses occupy 2x2: [x,y], [x+1,y], [x,y+1], [x+1,y+1])
    let monster_idx = state.level.monsters.iter().position(|m| {
        if !m.is_alive() { return false; }
        if m.is_boss {
            nx >= m.x && nx <= m.x + 1 && ny >= m.y && ny <= m.y + 1
        } else {
            m.x == nx && m.y == ny
        }
    });
    if let Some(idx) = monster_idx {
        let killed = player_attack(state, idx);
        if !killed {
            monster_attack(state, idx);
        }
        return serde_json::json!({"moved": false, "combat": true});
    }

    state.player.x = nx;
    state.player.y = ny;
    let newly = reveal_around(&mut state.level, nx, ny, state.vision_radius);

    // Pick up items
    let items_here: Vec<usize> = state.level.items.iter().enumerate()
        .filter(|(_, it)| it.x == nx && it.y == ny)
        .map(|(i, _)| i)
        .collect();

    for &idx in items_here.iter().rev() {
        let item = state.level.items.remove(idx);
        pickup_item(state, &item);
    }

    // Check traps
    let mut trap_damage = 0;
    let mut trap_name = String::new();
    for trap in &mut state.level.traps {
        if trap.x == nx && trap.y == ny && !trap.triggered {
            trap.triggered = true;
            let mut rng = rand::thread_rng();
            trap_damage = (trap.damage + rng.gen_range(-2..=2)).max(1);
            trap_name = trap.name.clone();
        }
    }
    if trap_damage > 0 {
        state.player.hp -= trap_damage;
        state.log(&format!("TRAP! {} deals {} damage!", trap_name, trap_damage), "#ff4444");
        if state.player.hp <= 0 {
            state.log("You have died.", "#ff0000");
            state.game_over = true;
        }
    }

    serde_json::json!({
        "moved": true,
        "revealed": newly,
    })
}

fn pickup_item(state: &mut GameState, item: &Item) {
    match item.item_type.as_str() {
        "gold" => {
            state.player.gold += item.value;
            state.log(&format!("Picked up {} gold.", item.value), "#ffd700");
        }
        "potion" => {
            state.player.potions += 1;
            state.log(&format!("Picked up {}.", item.name), "#44ff44");
        }
        "weapon" => {
            if item.value > state.player.weapon_damage {
                state.log(&format!("Equipped {}! (ATK +{})", item.name, item.value), "#ff8844");
                state.player.weapon = item.name.clone();
                state.player.weapon_damage = item.value;
            } else {
                let sell = item.value * 2;
                state.player.gold += sell;
                state.log(&format!("Sold {} for {} gold.", item.name, sell), "#888");
            }
        }
        "armor" => {
            if item.value > state.player.armor_defense {
                state.log(&format!("Equipped {}! (DEF +{})", item.name, item.value), "#4488ff");
                state.player.armor = item.name.clone();
                state.player.armor_defense = item.value;
            } else {
                let sell = item.value * 2;
                state.player.gold += sell;
                state.log(&format!("Sold {} for {} gold.", item.name, sell), "#888");
            }
        }
        _ => {}
    }
}

pub fn use_potion(state: &mut GameState) -> bool {
    if state.player.potions <= 0 {
        state.log("No potions!", "#888");
        return false;
    }
    let mut rng = rand::thread_rng();
    let heal = rng.gen_range(8..=15);
    state.player.potions -= 1;
    state.player.hp = (state.player.hp + heal).min(state.player.max_hp);
    state.log(&format!("You drink a potion and heal {} HP. ({} left)", heal, state.player.potions), "#44ff44");
    true
}

// ── Monster AI ──

pub fn monster_turns(state: &mut GameState) -> Vec<serde_json::Value> {
    let mut events = vec![];
    let px = state.player.x;
    let py = state.player.y;

    for i in 0..state.level.monsters.len() {
        if !state.level.monsters[i].is_alive() { continue; }
        let mon = &state.level.monsters[i];
        let dist = (mon.x - px).abs() + (mon.y - py).abs();
        if dist > 8 { continue; }

        // Adjacent? Attack.
        let adjacent = if mon.is_boss {
            // Boss is 2x2: check if player is adjacent to any of the 4 tiles
            let bx = mon.x;
            let by = mon.y;
            (px >= bx - 1 && px <= bx + 2 && py >= by - 1 && py <= by + 2)
                && !(px >= bx && px <= bx + 1 && py >= by && py <= by + 1) // not inside the boss
        } else {
            (mon.x - px).abs() <= 1 && (mon.y - py).abs() <= 1 && dist == 1
        };
        if adjacent {
            monster_attack(state, i);
            if state.game_over { return events; }
            continue;
        }


        // Move toward player
        let dx = if px > mon.x { 1 } else if px < mon.x { -1 } else { 0 };
        let dy = if py > mon.y { 1 } else if py < mon.y { -1 } else { 0 };

        let (nx, ny) = if (px - mon.x).abs() >= (py - mon.y).abs() {
            (mon.x + dx, mon.y)
        } else {
            (mon.x, mon.y + dy)
        };

        if nx >= 0 && ny >= 0 && nx < state.level.width && ny < state.level.height {
            let tile = &state.level.tiles[ny as usize][nx as usize];
            let walkable = state.level.tile_defs.get(tile).map_or(false, |t| t.walkable);
            if walkable {
                let blocked = state.level.monsters.iter().enumerate()
                    .any(|(j, m)| j != i && m.x == nx && m.y == ny && m.is_alive());
                if !blocked && !(nx == px && ny == py) {
                    state.level.monsters[i].x = nx;
                    state.level.monsters[i].y = ny;
                    events.push(serde_json::json!({
                        "id": state.level.monsters[i].id,
                        "x": nx, "y": ny,
                    }));
                }
            }
        }
    }
    events
}

// ── Fog ──

pub fn reveal_around(level: &mut Level, px: i32, py: i32, radius: i32) -> Vec<[i32; 2]> {
    let mut newly = vec![];

    // Clear current visibility
    level.visible.clear();

    // Cast rays to perimeter of vision circle
    let r2 = radius * radius;
    let steps = (radius * 8).max(32); // number of rays around the circle
    for i in 0..steps {
        let angle = (i as f32 / steps as f32) * std::f32::consts::TAU;
        let dx = angle.cos();
        let dy = angle.sin();

        // March along the ray
        let mut x = px as f32 + 0.5;
        let mut y = py as f32 + 0.5;
        for _ in 0..=(radius + 1) {
            let tx = x as i32;
            let ty = y as i32;

            if tx < 0 || ty < 0 || tx >= level.width || ty >= level.height { break; }

            let dist2 = (tx - px) * (tx - px) + (ty - py) * (ty - py);
            if dist2 > r2 { break; }

            // Mark visible and revealed
            level.visible.insert((tx, ty));
            if level.revealed.insert((tx, ty)) {
                newly.push([tx, ty]);
            }

            // Stop ray after hitting a wall (but the wall tile itself is visible)
            let tile = &level.tiles[ty as usize][tx as usize];
            if !level.tile_defs.get(tile).map_or(false, |t| t.walkable) {
                break;
            }

            x += dx;
            y += dy;
        }
    }

    // Always see own tile
    level.visible.insert((px, py));
    level.revealed.insert((px, py));

    newly
}
