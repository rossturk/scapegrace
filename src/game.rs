use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ── Types ──

#[derive(Clone, PartialEq)]
pub enum NodeType {
    Start,
    Level,
    Store,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TileDef {
    pub name: String,
    pub color: String,
    pub walkable: bool,
    pub char_display: String,
    #[serde(default)]
    pub damage: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Monster {
    pub id: String,
    pub name: String,
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
    #[serde(default)]
    pub boss_enraged_turns: i32,     // turns of 2x speed remaining (set on first sight)
    #[serde(default)]
    pub boss_has_seen_player: bool,   // whether boss has ever spotted the player
    #[serde(default)]
    pub boss_attacked_this_turn: bool, // set each turn to track regen eligibility
    #[serde(default)]
    pub boss_body: Vec<(i32, i32)>,  // dynamic body tiles (4 tiles, can reshape to squeeze)
    #[serde(default)]
    pub boss_flee_budget: i32,       // flee turns remaining (resets after cooldown)
    #[serde(default)]
    pub boss_flee_cooldown: i32,     // when >0, boss can't flee and must fight
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
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
    pub x: i32,
    pub y: i32,
    pub item_type: String, // weapon, armor, potion, gold
    pub value: i32,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
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
    pub keys: i32,
    pub floor: i32,
    pub facing: f32, // radians, 0 = right, PI/2 = down
    pub bombs: i32,
    pub speed_potions: i32,
    pub speed_turns: i32,
    #[serde(default = "default_potion_cap")]
    pub potion_cap: i32,
    #[serde(default)]
    pub antidotes: i32,        // consumable: negates damage tile damage for 20 steps
    #[serde(default)]
    pub antidote_steps: i32,   // remaining immune steps (active effect)
    #[serde(default)]
    pub scout_maps: i32,       // consumable: reveals entire map on level entry
}

fn default_potion_cap() -> i32 { 10 }

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
            potions: 1, keys: 0, floor: 1, facing: -std::f32::consts::FRAC_PI_2,
            bombs: 0, speed_potions: 0, speed_turns: 0,
            potion_cap: 10,
            antidotes: 0, antidote_steps: 0, scout_maps: 0,
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
    pub victory_message: String,
    pub defeat_message: String,
    #[serde(skip)]
    pub revealed: HashSet<(i32, i32)>,
    #[serde(skip)]
    pub visible: HashSet<(i32, i32)>,
    #[serde(skip)]
    pub char_marks: std::collections::HashMap<(i32, i32), f32>, // bomb scorch: pos → intensity 0.0-1.0
    /// Per-region music scales for unified overworld (ox, oy, w, h, scale_freqs)
    #[serde(skip)]
    pub region_scales: Vec<(i32, i32, i32, i32, Vec<f32>)>,
}

impl Level {
    /// Get the music scale for a position. Returns the region's scale if inside one, or the default.
    pub fn scale_at(&self, x: i32, y: i32) -> &[f32] {
        for (ox, oy, w, h, scale) in &self.region_scales {
            if x >= *ox && x < ox + w && y >= *oy && y < oy + h {
                return scale;
            }
        }
        &self.scale // default (C major for overworld)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Trap {
    pub x: i32,
    pub y: i32,
    pub damage: i32,
    pub name: String,
    pub triggered: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

#[derive(Clone)]
pub struct Overworld {
    pub name: String,
    pub font: String,
    pub description_font: String,
    pub label_font: String,
    pub description: String,
    pub bg_color: String,
    pub text_color: String,
    pub nodes: Vec<OverworldNode>,
    pub connections: Vec<(usize, usize)>,
    pub current_node: usize,
    pub store_stock: Vec<StoreSlot>,
    pub bg_image: Option<String>,
    pub bg_gradient: Option<String>,
    pub bg_terrain: bool,
    pub terrain_seed: u32,
    pub title_x: f32,
    pub title_y: f32,
    pub title_font_size: f32,
    pub desc_x: f32,
    pub desc_y: f32,
    pub desc_font_size: f32,
}

impl Overworld {
    /// Scale store prices based on campaign tier so the economy stays proportional.
    /// Permanent upgrades (max_hp, potion_cap) scale slower so they stay affordable.
    pub fn scale_store_prices(&mut self, campaign_tier: i32) {
        for slot in &mut self.store_stock {
            let rate = match slot.item_type.as_str() {
                "max_hp" | "potion_cap" => 0.2,
                _ => 0.3,
            };
            let multiplier = 1.0 + campaign_tier as f32 * rate;
            slot.price = (slot.price as f32 * multiplier).ceil() as i32;
        }
    }
}

/// Unified overworld tile grid — all levels stitched together with hallways
#[derive(Clone)]
pub struct OverworldMap {
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<Vec<String>>,
    pub tile_defs: std::collections::HashMap<String, TileDef>,
    pub level_regions: Vec<LevelRegion>,
    pub hallways: Vec<HallwaySegment>,
    pub player_pos: [i32; 2], // current player position on the overworld
}

#[derive(Clone)]
pub struct LevelRegion {
    pub node_idx: usize,
    pub ox: i32, pub oy: i32, // offset in the overworld grid
    pub w: i32, pub h: i32,
    pub entry_pos: Option<[i32; 2]>, // world coords of entry door
    pub exit_pos: Option<[i32; 2]>,  // world coords of exit door
}

#[derive(Clone)]
pub struct HallwaySegment {
    pub from_level: usize,
    pub to_level: usize,
    pub tiles: Vec<(i32, i32)>, // ordered floor tile positions for stats scroll
}

#[derive(Clone)]
pub struct StoreSlot {
    pub name: String,
    pub description: String,
    pub item_type: String,
    pub price: i32,
    pub stock: i32,
    pub value: i32, // effect magnitude (HP amount, cap increase, etc.)
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
    pub node_type: NodeType,
    pub exit_direction: String, // "n", "s", "e", "w" — default "e"
}

pub struct GameState {
    pub player: Player,
    pub level: Level,
    pub log: Vec<LogEntry>,
    pub game_over: bool,
    pub victory: bool,
    pub vision_radius: i32,
    pub item_sprites: std::collections::HashMap<String, String>,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            player: Player::default(),
            level: Level {
                width: 60, height: 36,
                tiles: vec![], tile_defs: Default::default(),
                monsters: vec![], items: vec![], traps: vec![],
                title: String::new(), description: String::new(), font: String::new(),
                scale: vec![], victory_message: String::new(), defeat_message: String::new(),
                revealed: HashSet::new(), visible: HashSet::new(),
                char_marks: Default::default(),
                region_scales: vec![],
            },
            log: vec![],
            game_over: false,
            victory: false,
            item_sprites: Default::default(),
            vision_radius: 5,
        }
    }

    pub fn log(&mut self, text: &str, color: &str) {
        self.log.push(LogEntry { text: text.into(), color: color.into() });
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
            let boss_x = state.level.monsters.get(monster_idx).map(|m| m.x).unwrap_or(0);
            let boss_y = state.level.monsters.get(monster_idx).map(|m| m.y).unwrap_or(0);

            // Find which region the boss is in, only unlock that region's exit doors
            let mut region_bounds: Option<(i32, i32, i32, i32)> = None;
            for &(ox, oy, w, h, _) in &state.level.region_scales {
                if boss_x >= ox && boss_x < ox + w && boss_y >= oy && boss_y < oy + h {
                    region_bounds = Some((ox, oy, w, h));
                    break;
                }
            }

            let mut found_exit = false;
            if let Some((rx, ry, rw, rh)) = region_bounds {
                // Unified map: only unlock doors in this region
                for y in ry..(ry + rh) {
                    for x in rx..(rx + rw) {
                        if y >= 0 && x >= 0 && (y as usize) < state.level.tiles.len() && (x as usize) < state.level.tiles[y as usize].len() {
                            if state.level.tiles[y as usize][x as usize] == "exit_door_locked" {
                                state.level.tiles[y as usize][x as usize] = "exit_door".to_string();
                                found_exit = true;
                            }
                        }
                    }
                }
            } else {
                // Fallback: single-level mode, unlock all exit doors
                for row in &mut state.level.tiles {
                    for tile in row.iter_mut() {
                        if tile == "exit_door_locked" {
                            *tile = "exit_door".to_string();
                            found_exit = true;
                        }
                    }
                }
            }

            if found_exit {
                state.log("An exit door has opened!", "#44ccff");
            } else if state.level.region_scales.is_empty() {
                // No exit door and not unified map — fall back to immediate victory
                state.victory = true;
            }
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
    while state.player.level < 50 && state.player.xp >= state.player.xp_to_next {
        state.player.xp -= state.player.xp_to_next;
        state.player.level += 1;
        state.player.max_hp += 5;
        state.player.hp = state.player.max_hp;
        state.player.attack += 1;
        state.player.defense += 1;
        state.player.xp_to_next = (state.player.xp_to_next as f64 * 1.25) as i32;
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
    let item = if roll < 0.90 {
        let gold = rng.gen_range(1..=xp_val.max(1));
        Item {
            id: format!("drop_{}", mon_id),
            name: format!("{} Gold", gold),
            x: mx, y: my,
            item_type: "gold".into(), value: gold,
            description: String::new(), image: state.item_sprites.get("gold").cloned(),
        }
    } else {
        Item {
            id: format!("drop_{}", mon_id),
            name: "Health Potion".into(),
            x: mx, y: my,
            item_type: "potion".into(), value: 0,
            description: String::new(), image: state.item_sprites.get("potion").cloned(),
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
    if tile == "locked_door" {
        if state.player.keys > 0 {
            state.player.keys -= 1;
            // Find the floor tile name to replace with
            let floor_name = state.level.tile_defs.values()
                .find(|t| t.walkable)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| "floor".into());
            state.level.tiles[ny as usize][nx as usize] = floor_name;
            state.log("You unlock the door! 🔓", "#ffd700");
            // Don't move into the tile this turn — just unlock
            return serde_json::json!({"moved": false, "unlocked": true});
        } else {
            state.log("The door is locked. Find a key.", "#888");
            return serde_json::json!({"moved": false});
        }
    }
    if tile == "store_merchant" {
        state.log("You approach the shopkeeper.", "#ffd700");
        return serde_json::json!({"moved": false, "store": true});
    }
    if let Some(td) = state.level.tile_defs.get(tile) {
        if !td.walkable {
            if tile == "exit_door_locked" {
                state.log("The gate is sealed. Defeat the boss to open it.", "#aa6622");
                return serde_json::json!({"moved": false, "gate_blocked": true});
            }
            return serde_json::json!({"moved": false});
        }
    }

    // Check monster collision (bosses use dynamic body tiles)
    let monster_idx = state.level.monsters.iter().position(|m| {
        if !m.is_alive() { return false; }
        if m.is_boss {
            m.boss_body.iter().any(|&(bx, by)| bx == nx && by == ny)
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

    // Check if player stepped on exit door
    if state.level.tiles[ny as usize][nx as usize] == "exit_door" {
        // In unified map (region_scales present), just log and continue — no victory screen
        if !state.level.region_scales.is_empty() {
            state.log("You pass through the gate...", "#44ccff");
            // Make the door walkable-through (already is, since exit_door is walkable)
        } else {
            state.log("You exit the level!", "#ffd700");
            state.victory = true;
            return serde_json::json!({"moved": true, "victory": true});
        }
    }


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

    // Check damage tiles (lava, acid, etc.) — antidote negates damage
    if let Some(td) = state.level.tile_defs.get(&state.level.tiles[ny as usize][nx as usize]) {
        if td.damage > 0 {
            if state.player.antidote_steps > 0 {
                state.player.antidote_steps -= 1;
                state.log(&format!("Antidote protects you from {}! ({} steps left)", td.name, state.player.antidote_steps), "#44ddaa");
            } else {
                state.player.hp -= td.damage;
                state.log(&format!("{} deals {} damage!", td.name, td.damage), "#ff6644");
                if state.player.hp <= 0 {
                    state.log("You have died.", "#ff0000");
                    state.game_over = true;
                }
            }
        }
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
            let cap = state.player.potion_cap;
            if state.player.potions >= cap {
                let gold = 3;
                state.player.gold += gold;
                state.log(&format!("Potions full! Sold {} for {}g.", item.name, gold), "#ffd700");
            } else {
                state.player.potions += 1;
                state.log(&format!("Picked up {}. ({}/{})", item.name, state.player.potions, cap), "#44ff44");
            }
        }
        "key" => {
            state.player.keys += 1;
            state.log("Picked up a key! 🔑", "#ffd700");
        }
        "weapon" => {
            if item.value > state.player.weapon_damage {
                state.log(&format!("Equipped {}! (ATK +{})", item.name, item.value), "#ff8844");
                state.player.weapon = item.name.clone();
                state.player.weapon_damage = item.value;
            } else {
                state.log(&format!("Found {} (ATK +{}) — kept {}.", item.name, item.value, state.player.weapon), "#888");
            }
        }
        "armor" => {
            if item.value > state.player.armor_defense {
                state.log(&format!("Equipped {}! (DEF +{})", item.name, item.value), "#4488ff");
                state.player.armor = item.name.clone();
                state.player.armor_defense = item.value;
            } else {
                state.log(&format!("Found {} (DEF +{}) — kept {}.", item.name, item.value, state.player.armor), "#888");
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
    let pct = rng.gen_range(0.15..=0.25);
    let heal = (state.player.max_hp as f32 * pct).round() as i32;
    state.player.potions -= 1;
    state.player.hp = (state.player.hp + heal).min(state.player.max_hp);
    state.log(&format!("You drink a potion and heal {} HP. ({} left)", heal, state.player.potions), "#44ff44");
    true
}

pub fn use_bomb(state: &mut GameState) -> bool {
    if state.player.bombs <= 0 {
        state.log("No bombs!", "#888");
        return false;
    }
    state.player.bombs -= 1;
    let mut rng = rand::thread_rng();
    let px = state.player.x;
    let py = state.player.y;
    let radius = 3;
    let mut hit_count = 0;

    state.log("You throw a bomb!", "#ff6600");

    // Char tiles in radius — intensity falls off with distance
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let tx = px + dx;
            let ty = py + dy;
            if tx < 0 || ty < 0 || tx >= state.level.width || ty >= state.level.height { continue; }
            let dist = ((dx * dx + dy * dy) as f32).sqrt();
            if dist > radius as f32 { continue; }
            let intensity = 1.0 - (dist / (radius as f32 + 1.0));
            let tile = &state.level.tiles[ty as usize][tx as usize];
            if state.level.tile_defs.get(tile).map_or(false, |t| t.walkable) {
                let existing = state.level.char_marks.get(&(tx, ty)).copied().unwrap_or(0.0);
                state.level.char_marks.insert((tx, ty), existing.max(intensity));
            }
        }
    }

    for i in 0..state.level.monsters.len() {
        if !state.level.monsters[i].is_alive() { continue; }
        // For bosses, use distance to closest body tile
        let dist = if state.level.monsters[i].is_boss {
            state.level.monsters[i].boss_body.iter()
                .map(|&(bx, by)| {
                    let dx = bx - px;
                    let dy = by - py;
                    ((dx * dx + dy * dy) as f32).sqrt()
                })
                .fold(f32::MAX, f32::min)
        } else {
            let dx = state.level.monsters[i].x - px;
            let dy = state.level.monsters[i].y - py;
            ((dx * dx + dy * dy) as f32).sqrt()
        };
        if dist > radius as f32 { continue; }

        let mon_name = state.level.monsters[i].name.clone();
        let is_boss = state.level.monsters[i].is_boss;
        hit_count += 1;

        if is_boss {
            // Bosses take heavy damage but survive
            let falloff = 1.0 - (dist / (radius as f32 + 1.0));
            let base = rng.gen_range(8..=15);
            let damage = ((base as f32 * falloff).round() as i32 * 2).max(1);
            state.level.monsters[i].hp -= damage;
            state.log(&format!("  Bomb hits {} for {} damage!", mon_name, damage), "#ff6600");

            if state.level.monsters[i].hp <= 0 {
                let xp = state.level.monsters[i].xp_value;
                state.log(&format!("  {} destroyed! (+{} XP)", mon_name, xp), "#44ff44");
                state.player.xp += xp;
                check_level_up(state);
                maybe_drop_loot(state, i);
                state.log("THE BOSS IS SLAIN!", "#ffd700");
                let mut found_exit = false;
                for row in &mut state.level.tiles {
                    for tile in row.iter_mut() {
                        if tile == "exit_door_locked" {
                            *tile = "exit_door".to_string();
                            found_exit = true;
                        }
                    }
                }
                if found_exit {
                    state.log("An exit door has opened!", "#44ccff");
                } else {
                    state.victory = true;
                }
            }
        } else {
            // Non-bosses are instantly killed
            let xp = state.level.monsters[i].xp_value;
            state.level.monsters[i].hp = 0;
            state.log(&format!("  Bomb obliterates {}! (+{} XP)", mon_name, xp), "#ff6600");
            state.player.xp += xp;
            check_level_up(state);
            maybe_drop_loot(state, i);
        }
    }

    if hit_count == 0 {
        state.log("  ...but nothing was in range.", "#888");
    }
    state.log(&format!("({} bombs left)", state.player.bombs), "#888");
    true
}

pub fn use_antidote(state: &mut GameState) -> bool {
    if state.player.antidotes <= 0 {
        state.log("No antidotes!", "#888");
        return false;
    }
    state.player.antidotes -= 1;
    state.player.antidote_steps = i32::MAX / 2; // lasts the whole level
    state.log(&format!("You drink an antidote! Immune to hazards this level. ({} left)", state.player.antidotes), "#44ddaa");
    true
}

/// Consume a scout map: reveal the entire level.
pub fn use_scout_map(state: &mut GameState) -> bool {
    if state.player.scout_maps <= 0 { return false; }
    state.player.scout_maps -= 1;
    for y in 0..state.level.height {
        for x in 0..state.level.width {
            state.level.revealed.insert((x, y));
        }
    }
    state.log(&format!("Scout map reveals the entire level! ({} left)", state.player.scout_maps), "#ffd700");
    true
}

pub fn use_speed_potion(state: &mut GameState) -> bool {
    if state.player.speed_potions <= 0 {
        state.log("No speed potions!", "#888");
        return false;
    }
    state.player.speed_potions -= 1;
    state.player.speed_turns = 5;
    state.log(&format!("You drink a speed potion! Monsters frozen for 5 turns. ({} left)", state.player.speed_potions), "#44ddff");
    true
}

// ── Monster AI ──

fn has_line_of_sight(level: &Level, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    let dx = (x1 - x0) as f32;
    let dy = (y1 - y0) as f32;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 1.0 { return true; }
    let steps = (dist * 2.0) as i32 + 1;
    let sx = dx / steps as f32;
    let sy = dy / steps as f32;
    let mut x = x0 as f32 + 0.5;
    let mut y = y0 as f32 + 0.5;
    for _ in 0..steps {
        x += sx;
        y += sy;
        let tx = x as i32;
        let ty = y as i32;
        if tx < 0 || ty < 0 || tx >= level.width || ty >= level.height { return false; }
        if tx == x1 && ty == y1 { return true; }
        let tile = &level.tiles[ty as usize][tx as usize];
        if !level.tile_defs.get(tile).map_or(false, |t| t.walkable) {
            return false;
        }
    }
    true
}

/// Check if player is adjacent to any boss body tile (not on one)
fn boss_body_adjacent(body: &[(i32, i32)], px: i32, py: i32) -> bool {
    let on_body = body.iter().any(|&(bx, by)| px == bx && py == by);
    if on_body { return false; }
    body.iter().any(|&(bx, by)| (px - bx).abs() <= 1 && (py - by).abs() <= 1)
}

/// Check if a tile is walkable and in bounds
fn tile_ok(state: &GameState, tx: i32, ty: i32) -> bool {
    tx >= 0 && ty >= 0 && tx < state.level.width && ty < state.level.height
        && state.level.tile_defs
            .get(&state.level.tiles[ty as usize][tx as usize])
            .map_or(false, |t| t.walkable)
}

/// Check if a tile is free of the player and other monsters (not counting boss_idx)
fn tile_free(state: &GameState, boss_idx: usize, tx: i32, ty: i32) -> bool {
    if tx == state.player.x && ty == state.player.y { return false; }
    for (j, m) in state.level.monsters.iter().enumerate() {
        if j == boss_idx || !m.is_alive() { continue; }
        if m.is_boss {
            if m.boss_body.iter().any(|&(bx, by)| bx == tx && by == ty) { return false; }
        } else if m.x == tx && m.y == ty {
            return false;
        }
    }
    true
}

/// Try to reform boss body into a 2x2 block around the head.
/// Picks the arrangement that puts the block closest to (target_x, target_y).
fn try_reform_2x2(state: &GameState, boss_idx: usize, head: (i32, i32), tx: i32, ty: i32) -> Option<Vec<(i32, i32)>> {
    // 4 possible 2x2 arrangements: head in each corner
    let arrangements: [[(i32, i32); 4]; 4] = [
        [(0,0), (1,0), (0,1), (1,1)],     // head top-left
        [(-1,0), (0,0), (-1,1), (0,1)],   // head top-right
        [(0,-1), (1,-1), (0,0), (1,0)],   // head bottom-left
        [(-1,-1), (0,-1), (-1,0), (0,0)], // head bottom-right
    ];
    let mut best: Option<(Vec<(i32, i32)>, i32)> = None;
    for arr in &arrangements {
        let tiles: Vec<(i32, i32)> = arr.iter()
            .map(|&(ox, oy)| (head.0 + ox, head.1 + oy))
            .collect();
        let all_ok = tiles.iter().all(|&(x, y)| tile_ok(state, x, y) && tile_free(state, boss_idx, x, y));
        if !all_ok { continue; }
        // Score: minimize distance from block center to target
        let cx = tiles.iter().map(|t| t.0).sum::<i32>();
        let cy = tiles.iter().map(|t| t.1).sum::<i32>();
        let dist = (cx - tx * 4).abs() + (cy - ty * 4).abs();
        if best.as_ref().map_or(true, |b| dist < b.1) {
            // Put head first
            let mut ordered = vec![head];
            for &t in &tiles { if t != head { ordered.push(t); } }
            best = Some((ordered, dist));
        }
    }
    best.map(|b| b.0)
}

/// Check if tiles form a connected group via cardinal adjacency.
fn is_body_connected(tiles: &[(i32, i32)]) -> bool {
    if tiles.len() <= 1 { return true; }
    let set: std::collections::HashSet<(i32,i32)> = tiles.iter().copied().collect();
    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![tiles[0]];
    visited.insert(tiles[0]);
    while let Some((x, y)) = stack.pop() {
        for &(nx, ny) in &[(x-1,y),(x+1,y),(x,y-1),(x,y+1)] {
            if set.contains(&(nx, ny)) && visited.insert((nx, ny)) {
                stack.push((nx, ny));
            }
        }
    }
    visited.len() == tiles.len()
}

/// Check if body forms a 2x2 block.
fn body_is_2x2(body: &[(i32, i32)]) -> bool {
    if body.len() != 4 { return false; }
    let min_x = body.iter().map(|t| t.0).min().unwrap();
    let min_y = body.iter().map(|t| t.1).min().unwrap();
    let max_x = body.iter().map(|t| t.0).max().unwrap();
    let max_y = body.iter().map(|t| t.1).max().unwrap();
    max_x - min_x == 1 && max_y - min_y == 1
}

/// Try to slide the boss as a 2x2 block. Returns true if successful.
fn try_2x2_slide(state: &mut GameState, i: usize, dx: i32, dy: i32, tx: i32, ty: i32) -> bool {
    let body = &state.level.monsters[i].boss_body;
    if !body_is_2x2(body) { return false; }
    let min_x = body.iter().map(|t| t.0).min().unwrap();
    let min_y = body.iter().map(|t| t.1).min().unwrap();

    // Try dominant axis first, then fallback
    let steps: [(i32,i32); 2] = if dx.abs() >= dy.abs() {
        [(dx.signum(), 0), (0, dy.signum())]
    } else {
        [(0, dy.signum()), (dx.signum(), 0)]
    };

    for (sx, sy) in steps {
        if sx == 0 && sy == 0 { continue; }
        let na = (min_x + sx, min_y + sy);
        let tiles = [(na.0,na.1),(na.0+1,na.1),(na.0,na.1+1),(na.0+1,na.1+1)];
        let ok = tiles.iter().all(|&(x,y)| tile_ok(state, x, y) && tile_free(state, i, x, y));
        if ok {
            // Order with closest to target first
            let mut new_body = tiles.to_vec();
            new_body.sort_by_key(|&(bx,by)| (bx-tx).abs() + (by-ty).abs());
            state.level.monsters[i].boss_body = new_body;
            state.level.monsters[i].x = state.level.monsters[i].boss_body[0].0;
            state.level.monsters[i].y = state.level.monsters[i].boss_body[0].1;
            return true;
        }
    }
    false
}

/// Amoeba movement: relocate the farthest removable body tile to a position
/// adjacent to the remaining body that's closest to the target.
fn amoeba_step(state: &GameState, boss_idx: usize, body: &[(i32, i32)], tx: i32, ty: i32) -> Option<Vec<(i32, i32)>> {
    // Find tiles that can be removed without disconnecting the body ("leaves")
    let removable: Vec<usize> = (0..body.len()).filter(|&i| {
        let remaining: Vec<_> = body.iter().enumerate()
            .filter(|(j, _)| *j != i).map(|(_, &t)| t).collect();
        is_body_connected(&remaining)
    }).collect();
    if removable.is_empty() { return None; }

    // Pick the removable tile farthest from target
    let &far_idx = removable.iter()
        .max_by_key(|&&i| {
            let (bx, by) = body[i];
            (bx - tx).abs() + (by - ty).abs()
        })?;

    let remaining: Vec<(i32,i32)> = body.iter().enumerate()
        .filter(|(j, _)| *j != far_idx).map(|(_, &t)| t).collect();
    let body_set: std::collections::HashSet<_> = body.iter().copied().collect();

    // Collect candidate positions: adjacent to remaining, walkable, free, not in body
    let mut candidates = std::collections::HashSet::new();
    for &(rx, ry) in &remaining {
        for &(nx, ny) in &[(rx-1,ry),(rx+1,ry),(rx,ry-1),(rx,ry+1)] {
            if body_set.contains(&(nx, ny)) { continue; }
            if !tile_ok(state, nx, ny) { continue; }
            if !tile_free(state, boss_idx, nx, ny) { continue; }
            candidates.insert((nx, ny));
        }
    }

    // Pick candidate closest to target that keeps body connected
    let best = candidates.iter()
        .filter(|&&(cx, cy)| {
            let mut new_body = remaining.clone();
            new_body.push((cx, cy));
            is_body_connected(&new_body)
        })
        .min_by_key(|&&(cx, cy)| (cx - tx).abs() + (cy - ty).abs())?;

    // Check that the overall body gets closer (sum of distances decreases)
    let old_sum: i32 = body.iter().map(|&(bx,by)| (bx-tx).abs() + (by-ty).abs()).sum();
    let mut new_body = remaining;
    new_body.push(*best);
    let new_sum: i32 = new_body.iter().map(|&(bx,by)| (bx-tx).abs() + (by-ty).abs()).sum();
    if new_sum >= old_sum { return None; }

    Some(new_body)
}

/// Update boss body[0] and monster x,y to be the tile closest to target.
fn update_boss_head(state: &mut GameState, i: usize, tx: i32, ty: i32) {
    let body = &state.level.monsters[i].boss_body;
    if body.is_empty() { return; }
    let best = body.iter().enumerate()
        .min_by_key(|(_, &(bx, by))| (bx - tx).abs() + (by - ty).abs())
        .map(|(idx, _)| idx).unwrap();
    if best != 0 { state.level.monsters[i].boss_body.swap(0, best); }
    let head = state.level.monsters[i].boss_body[0];
    state.level.monsters[i].x = head.0;
    state.level.monsters[i].y = head.1;
}

/// Move boss toward or away from a target. Returns true if moved.
fn try_boss_move(state: &mut GameState, i: usize, dx: i32, dy: i32, events: &mut Vec<serde_json::Value>) -> bool {
    // Compute target point (player for charging, away-from-player for fleeing)
    let body = &state.level.monsters[i].boss_body;
    let cx = body.iter().map(|t| t.0).sum::<i32>() / body.len().max(1) as i32;
    let cy = body.iter().map(|t| t.1).sum::<i32>() / body.len().max(1) as i32;
    let tx = cx + dx;
    let ty = cy + dy;

    // Try 2x2 block slide first (normal movement in open space)
    if try_2x2_slide(state, i, dx, dy, tx, ty) {
        events.push(serde_json::json!({"id": state.level.monsters[i].id,
            "x": state.level.monsters[i].x, "y": state.level.monsters[i].y}));
        return true;
    }

    // Amoeba squeeze: relocate farthest tile closer to target
    let body = state.level.monsters[i].boss_body.clone();
    if let Some(new_body) = amoeba_step(state, i, &body, tx, ty) {
        state.level.monsters[i].boss_body = new_body;
        update_boss_head(state, i, tx, ty);

        // Try to reform 2x2 if we're no longer squeezed
        if !body_is_2x2(&state.level.monsters[i].boss_body) {
            let head = state.level.monsters[i].boss_body[0];
            if let Some(reformed) = try_reform_2x2(state, i, head, tx, ty) {
                state.level.monsters[i].boss_body = reformed;
                update_boss_head(state, i, tx, ty);
            }
        }

        events.push(serde_json::json!({"id": state.level.monsters[i].id,
            "x": state.level.monsters[i].x, "y": state.level.monsters[i].y}));
        return true;
    }

    false
}

pub fn monster_turns(state: &mut GameState) -> Vec<serde_json::Value> {
    let mut events = vec![];
    let px = state.player.x;
    let py = state.player.y;

    for i in 0..state.level.monsters.len() {
        if !state.level.monsters[i].is_alive() { continue; }
        state.level.monsters[i].boss_attacked_this_turn = false;

        let mon = &state.level.monsters[i];

        if mon.is_boss {
            // Use closest body tile for distance and LOS checks
            let body = mon.boss_body.clone();
            let closest = body.iter()
                .min_by_key(|&&(bx, by)| (bx - px).abs() + (by - py).abs())
                .copied().unwrap_or((mon.x, mon.y));
            let dist = (closest.0 - px).abs() + (closest.1 - py).abs();
            if dist > 8 { continue; }
            let has_los = body.iter()
                .any(|&(bx, by)| has_line_of_sight(&state.level, bx, by, px, py));
            if !has_los { continue; }

            // ── Boss AI ──
            let hp_pct = mon.hp as f32 / mon.max_hp as f32;

            // First sight: enrage for a burst of 2x speed
            if !mon.boss_has_seen_player {
                state.level.monsters[i].boss_has_seen_player = true;
                state.level.monsters[i].boss_enraged_turns = 8;
            }

            // Tick down flee cooldown
            if state.level.monsters[i].boss_flee_cooldown > 0 {
                state.level.monsters[i].boss_flee_cooldown -= 1;
            }

            // Can flee if hurt, has budget, and not on cooldown
            let wants_to_flee = hp_pct <= 0.3;
            let can_flee = wants_to_flee
                && state.level.monsters[i].boss_flee_budget > 0
                && state.level.monsters[i].boss_flee_cooldown == 0;

            // Refill flee budget when first dropping below 30%
            if wants_to_flee && state.level.monsters[i].boss_flee_budget == 0
                && state.level.monsters[i].boss_flee_cooldown == 0 {
                state.level.monsters[i].boss_flee_budget = 5;
            }

            // Adjacent? Attack (unless fleeing)
            let adjacent = boss_body_adjacent(&body, px, py);
            if adjacent && !can_flee {
                monster_attack(state, i);
                state.level.monsters[i].boss_attacked_this_turn = true;
                if state.game_over { return events; }
                continue;
            }

            // Fleeing: run for up to 5 turns, then 8 turn cooldown before fleeing again
            let mut boss_moved = false;
            if can_flee {
                let dx = -(px - closest.0); // away from player
                let dy = -(py - closest.1);
                boss_moved = try_boss_move(state, i, dx, dy, &mut events);
                if !boss_moved {
                    // Cornered — regenerate instead of moving
                    let regen = (state.level.monsters[i].max_hp / 20).max(1);
                    state.level.monsters[i].hp = (state.level.monsters[i].hp + regen)
                        .min(state.level.monsters[i].max_hp);
                }
                state.level.monsters[i].boss_flee_budget -= 1;
                if state.level.monsters[i].boss_flee_budget == 0 {
                    // Out of flee budget — must fight for 8 turns
                    state.level.monsters[i].boss_flee_cooldown = 8;
                }
                continue;
            }

            // Charge toward player
            let steps = if state.level.monsters[i].boss_enraged_turns > 0 {
                state.level.monsters[i].boss_enraged_turns -= 1;
                2 // double speed while enraged
            } else {
                1
            };
            for _ in 0..steps {
                let body = &state.level.monsters[i].boss_body;
                if boss_body_adjacent(body, px, py) {
                    monster_attack(state, i);
                    state.level.monsters[i].boss_attacked_this_turn = true;
                    if state.game_over { return events; }
                    boss_moved = true;
                    break;
                }
                let dx = px - closest.0;
                let dy = py - closest.1;
                if try_boss_move(state, i, dx, dy, &mut events) {
                    boss_moved = true;
                } else {
                    break;
                }
            }

            // Regenerate only when standing still (no move, no attack)
            if !boss_moved && !state.level.monsters[i].boss_attacked_this_turn {
                let regen = (state.level.monsters[i].max_hp / 20).max(1);
                state.level.monsters[i].hp = (state.level.monsters[i].hp + regen)
                    .min(state.level.monsters[i].max_hp);
            }
            continue;
        }

        // ── Regular monster AI ──
        let dist = (mon.x - px).abs() + (mon.y - py).abs();
        if dist > 8 { continue; }
        if !has_line_of_sight(&state.level, mon.x, mon.y, px, py) { continue; }

        let adjacent = (mon.x - px).abs() <= 1 && (mon.y - py).abs() <= 1 && dist == 1;
        if adjacent {
            monster_attack(state, i);
            if state.game_over { return events; }
            continue;
        }

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
                    .any(|(j, m)| {
                        if j == i || !m.is_alive() { return false; }
                        if m.is_boss {
                            m.boss_body.iter().any(|&(bx, by)| bx == nx && by == ny)
                        } else {
                            m.x == nx && m.y == ny
                        }
                    });
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
    let r_f = radius as f32 + 0.5; // slight extra reach for smoother edge
    let steps = (radius * 16).max(64); // number of rays around the circle
    for i in 0..steps {
        let angle = (i as f32 / steps as f32) * std::f32::consts::TAU;
        let dx = angle.cos();
        let dy = angle.sin();

        // March along the ray, tracking float distance
        let origin_x = px as f32 + 0.5;
        let origin_y = py as f32 + 0.5;
        let mut x = origin_x;
        let mut y = origin_y;
        for _ in 0..=(radius + 2) {
            let tx = x as i32;
            let ty = y as i32;

            if tx < 0 || ty < 0 || tx >= level.width || ty >= level.height { break; }

            // Float distance from player center
            let fdx = x - origin_x;
            let fdy = y - origin_y;
            if fdx * fdx + fdy * fdy > r_f * r_f { break; }

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

/// Measure how open the space around the player is by casting rays in 8 directions
/// and averaging how far they reach before hitting a wall. Returns 0.0 (completely
/// boxed in) to 1.0 (wide open space), normalized against a max range.
pub fn measure_openness(level: &Level, px: i32, py: i32) -> f32 {
    let max_range: f32 = 12.0; // max distance to probe
    let directions: [(f32, f32); 8] = [
        (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0),
        (0.7071, 0.7071), (-0.7071, 0.7071), (0.7071, -0.7071), (-0.7071, -0.7071),
    ];
    let mut total_dist = 0.0f32;
    for &(dx, dy) in &directions {
        let mut dist = 0.0f32;
        loop {
            dist += 1.0;
            if dist > max_range { break; }
            let tx = (px as f32 + dx * dist) as i32;
            let ty = (py as f32 + dy * dist) as i32;
            if tx < 0 || ty < 0 || tx >= level.width || ty >= level.height { break; }
            let tile = &level.tiles[ty as usize][tx as usize];
            if !level.tile_defs.get(tile).map_or(false, |t| t.walkable) {
                break;
            }
        }
        total_dist += dist;
    }
    // Average distance across all rays, normalized to 0..1
    let avg = total_dist / (8.0 * max_range);
    avg.clamp(0.0, 1.0)
}
