/// MCP server for playtesting Scapegrace.
///
/// Instead of tile-by-tile control, this server has a built-in bot that
/// plays the game autonomously with A* pathfinding and combat AI.
/// MCP tools expose high-level simulation: "play this campaign", "simulate
/// 10 campaigns in sequence", "show me the balance curve".
use scapegrace::{game, gen};
use game::{GameState, Level, Player};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::cmp::Reverse;
use std::io::{self, BufRead, Write};

// ══════════════════════════════════════════════════════════════════════
//  A* pathfinding
// ══════════════════════════════════════════════════════════════════════

fn astar(level: &Level, from: (i32, i32), to: (i32, i32), avoid: &HashSet<(i32, i32)>, has_keys: bool) -> Option<Vec<(i32, i32)>> {
    if from == to { return Some(vec![to]); }

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();

    let h = |a: (i32, i32)| (a.0 - to.0).abs() + (a.1 - to.1).abs();
    g_score.insert(from, 0);
    open.push(Reverse((h(from), from)));

    while let Some(Reverse((_f, cur))) = open.pop() {
        if cur == to {
            let mut path = vec![cur];
            let mut c = cur;
            while let Some(&prev) = came_from.get(&c) {
                path.push(prev);
                c = prev;
            }
            path.reverse();
            return Some(path);
        }

        let g = g_score[&cur];
        for &(dx, dy) in &[(0, -1), (0, 1), (-1, 0), (1, 0)] {
            let nx = cur.0 + dx;
            let ny = cur.1 + dy;
            if nx < 0 || ny < 0 || nx >= level.width || ny >= level.height { continue; }

            let tile = &level.tiles[ny as usize][nx as usize];
            let walkable = if tile == "locked_door" {
                // Can path through locked doors if player has keys
                has_keys
            } else if (nx, ny) == to {
                level.tile_defs.get(tile).map_or(false, |t| t.walkable)
            } else {
                level.tile_defs.get(tile).map_or(false, |t| t.walkable)
                    && !avoid.contains(&(nx, ny))
            };
            if !walkable { continue; }

            // Heavily penalize damage tiles — a 50-tile detour is better than 3 HP/step
            let cost = if level.tile_defs.get(tile).map_or(false, |t| t.damage > 0) { 50 } else { 1 };
            let ng = g + cost;
            if ng < *g_score.get(&(nx, ny)).unwrap_or(&i32::MAX) {
                g_score.insert((nx, ny), ng);
                came_from.insert((nx, ny), cur);
                open.push(Reverse((ng + h((nx, ny)), (nx, ny))));
            }
        }
    }
    None
}

// ══════════════════════════════════════════════════════════════════════
//  Smart Bot — state machine that explores, collects, clears, bosses
// ══════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq, Debug)]
enum BotPhase {
    Explore,   // reveal the full map, grab items on the way
    Collect,   // pick up weapon/armor/key/potions
    Clear,     // kill non-boss monsters
    Boss,      // kill the boss
}

#[derive(Clone, Copy, PartialEq)]
enum Strategy {
    Thorough,  // clear everything
    Rush,      // skip to boss ASAP
    Balanced,  // clear what's nearby
}

impl Strategy {
    fn from_str(s: &str) -> Self {
        match s {
            "rush" => Self::Rush,
            "thorough" => Self::Thorough,
            _ => Self::Balanced,
        }
    }
}

struct LevelResult {
    victory: bool,
    turns: i32,
    kills: i32,
    damage_dealt: i32,
    damage_taken: i32,
    potions_used: i32,
    bombs_used: i32,
    speed_potions_used: i32,
    player_level_before: i32,
    player_level_after: i32,
    player_hp_at_end: i32,
    lowest_hp: i32,
    cause_of_death: Option<String>,
    level_title: String,
    boss_name: String,
    boss_hp: i32,
    monster_count: i32,
    difficulty: String,
}

fn rate_difficulty(lowest_hp: i32, max_hp: i32, potions_used: i32, _damage_taken: i32, victory: bool) -> String {
    if !victory { return "IMPOSSIBLE".into(); }
    let hp_pct = lowest_hp as f32 / max_hp as f32;
    if potions_used == 0 && hp_pct > 0.7 { return "trivial".into(); }
    if potions_used == 0 && hp_pct > 0.4 { return "easy".into(); }
    if hp_pct > 0.2 { return "moderate".into(); }
    if hp_pct > 0.05 { return "hard".into(); }
    "brutal".into()
}

// ── A* with monster avoidance ──

/// A* that can optionally route around monsters and heavily penalizes damage tiles.
fn astar_smart(level: &Level, from: (i32, i32), to: (i32, i32),
               avoid_monsters: &HashSet<(i32, i32)>, has_keys: bool) -> Option<Vec<(i32, i32)>> {
    astar(level, from, to, avoid_monsters, has_keys)
}

/// Try to path avoiding monsters first; if impossible or much longer, path through them.
fn path_to(level: &Level, from: (i32, i32), to: (i32, i32),
           monster_tiles: &HashSet<(i32, i32)>, has_keys: bool) -> Option<Vec<(i32, i32)>> {
    let direct = astar(level, from, to, &HashSet::new(), has_keys);
    let avoiding = astar_smart(level, from, to, monster_tiles, has_keys);
    match (&avoiding, &direct) {
        (Some(a), Some(d)) => {
            // Only avoid if the detour isn't more than 50% longer
            if a.len() <= d.len() * 3 / 2 { avoiding } else { direct }
        }
        (Some(_), None) => avoiding,
        (None, _) => direct,
    }
}

/// Get all tiles occupied by alive monsters (for avoidance).
fn monster_positions(state: &GameState) -> HashSet<(i32, i32)> {
    let mut set = HashSet::new();
    for m in &state.level.monsters {
        if !m.is_alive() { continue; }
        if m.is_boss {
            for &(bx, by) in &m.boss_body { set.insert((bx, by)); }
        } else {
            set.insert((m.x, m.y));
        }
    }
    set
}

// ── Exploration ──

fn find_frontier(state: &GameState) -> Vec<(i32, i32)> {
    let mut frontier = Vec::new();
    for &(rx, ry) in &state.level.revealed {
        let tile = &state.level.tiles[ry as usize][rx as usize];
        if !state.level.tile_defs.get(tile).map_or(false, |t| t.walkable) { continue; }
        for &(dx, dy) in &[(0, -1), (0, 1), (-1, 0), (1, 0)] {
            let nx = rx + dx;
            let ny = ry + dy;
            if nx >= 0 && ny >= 0 && nx < state.level.width && ny < state.level.height
                && !state.level.revealed.contains(&(nx, ny)) {
                frontier.push((rx, ry));
                break;
            }
        }
    }
    frontier
}

fn exploration_complete(state: &GameState) -> bool {
    find_frontier(state).is_empty()
}

// ── Target picking per phase ──

fn pick_explore_target(state: &GameState, mon_tiles: &HashSet<(i32, i32)>) -> Option<(i32, i32)> {
    let px = state.player.x;
    let py = state.player.y;
    let has_keys = state.player.keys > 0;

    // Fight any adjacent monster immediately (don't run from combat)
    let adjacent_monster = state.level.monsters.iter()
        .filter(|m| m.is_alive() && !m.is_boss)
        .find(|m| (m.x - px).abs() + (m.y - py).abs() == 1)
        .map(|m| (m.x, m.y));
    if adjacent_monster.is_some() { return adjacent_monster; }

    // Grab weapon/armor/key if close (within 8 tiles) while exploring
    let nearby_equip = state.level.items.iter()
        .filter(|it| it.item_type == "weapon" || it.item_type == "armor" || it.item_type == "key")
        .filter(|it| state.level.revealed.contains(&(it.x, it.y)))
        .filter(|it| (it.x - px).abs() + (it.y - py).abs() <= 8)
        .filter(|it| path_to(&state.level, (px, py), (it.x, it.y), mon_tiles, has_keys).is_some())
        .min_by_key(|it| (it.x - px).abs() + (it.y - py).abs())
        .map(|it| (it.x, it.y));
    if nearby_equip.is_some() { return nearby_equip; }

    // Go to nearest reachable frontier tile
    let frontier = find_frontier(state);
    frontier.iter()
        .filter(|&&(x, y)| path_to(&state.level, (px, py), (x, y), mon_tiles, has_keys).is_some())
        .min_by_key(|&&(x, y)| (x - px).abs() + (y - py).abs())
        .copied()
}

fn pick_collect_target(state: &GameState, mon_tiles: &HashSet<(i32, i32)>) -> Option<(i32, i32)> {
    let px = state.player.x;
    let py = state.player.y;
    let has_keys = state.player.keys > 0;
    let reachable = |x: i32, y: i32| -> bool {
        path_to(&state.level, (px, py), (x, y), mon_tiles, has_keys).is_some()
    };

    // Key first (unlocks areas)
    let key = state.level.items.iter()
        .filter(|it| it.item_type == "key" && state.level.revealed.contains(&(it.x, it.y)))
        .filter(|it| reachable(it.x, it.y))
        .min_by_key(|it| (it.x - px).abs() + (it.y - py).abs())
        .map(|it| (it.x, it.y));
    if key.is_some() { return key; }

    // Locked door if we have a key
    if state.player.keys > 0 {
        for y in 0..state.level.height {
            for x in 0..state.level.width {
                if state.level.tiles[y as usize][x as usize] == "locked_door"
                    && state.level.revealed.contains(&(x, y))
                {
                    return Some((x, y));
                }
            }
        }
    }

    // Weapon (if we don't have one yet or it's better)
    let weapon = state.level.items.iter()
        .filter(|it| it.item_type == "weapon" && state.level.revealed.contains(&(it.x, it.y)))
        .filter(|it| it.value > state.player.weapon_damage)
        .filter(|it| reachable(it.x, it.y))
        .min_by_key(|it| (it.x - px).abs() + (it.y - py).abs())
        .map(|it| (it.x, it.y));
    if weapon.is_some() { return weapon; }

    // Armor
    let armor = state.level.items.iter()
        .filter(|it| it.item_type == "armor" && state.level.revealed.contains(&(it.x, it.y)))
        .filter(|it| it.value > state.player.armor_defense)
        .filter(|it| reachable(it.x, it.y))
        .min_by_key(|it| (it.x - px).abs() + (it.y - py).abs())
        .map(|it| (it.x, it.y));
    if armor.is_some() { return armor; }

    // Potions / gold
    state.level.items.iter()
        .filter(|it| state.level.revealed.contains(&(it.x, it.y)))
        .filter(|it| reachable(it.x, it.y))
        .min_by_key(|it| (it.x - px).abs() + (it.y - py).abs())
        .map(|it| (it.x, it.y))
}

fn pick_clear_target(state: &GameState, _mon_tiles: &HashSet<(i32, i32)>) -> Option<(i32, i32)> {
    let px = state.player.x;
    let py = state.player.y;
    let has_keys = state.player.keys > 0;

    // Kill nearest non-boss monster
    state.level.monsters.iter()
        .filter(|m| m.is_alive() && !m.is_boss)
        .filter(|m| path_to(&state.level, (px, py), (m.x, m.y), &HashSet::new(), has_keys).is_some())
        .min_by_key(|m| (m.x - px).abs() + (m.y - py).abs())
        .map(|m| (m.x, m.y))
}

fn pick_boss_target(state: &GameState) -> Option<(i32, i32)> {
    let px = state.player.x;
    let py = state.player.y;
    let boss = state.level.monsters.iter().find(|m| m.is_boss && m.is_alive())?;
    // Target the closest body tile directly (bump attack)
    boss.boss_body.iter()
        .min_by_key(|&&(bx, by)| (bx - px).abs() + (by - py).abs())
        .copied()
}

fn no_reachable_non_boss(state: &GameState) -> bool {
    let px = state.player.x;
    let py = state.player.y;
    let has_keys = state.player.keys > 0;
    !state.level.monsters.iter().any(|m| {
        m.is_alive() && !m.is_boss
            && astar(&state.level, (px, py), (m.x, m.y), &HashSet::new(), has_keys).is_some()
    })
}

// ── The main play loop ──

fn play_level(state: &mut GameState, strategy: Strategy) -> LevelResult {
    let level_title = state.level.title.clone();
    let boss_name = state.level.monsters.iter()
        .find(|m| m.is_boss).map(|m| m.name.clone()).unwrap_or_default();
    let boss_hp = state.level.monsters.iter()
        .find(|m| m.is_boss).map(|m| m.max_hp).unwrap_or(0);
    let monster_count = state.level.monsters.iter().filter(|m| !m.is_boss && m.is_alive()).count() as i32;
    let player_level_before = state.player.level;
    let initial_max_hp = state.player.max_hp;

    let mut turns = 0;
    let mut kills = 0;
    let mut damage_dealt = 0;
    let mut damage_taken = 0;
    let mut potions_used = 0;
    let mut bombs_used = 0;
    let mut speed_potions_used = 0;
    let mut lowest_hp = state.player.hp;
    let max_turns = 5000;

    // State machine
    let mut phase = if strategy == Strategy::Rush { BotPhase::Boss } else { BotPhase::Explore };
    let mut stuck_turns = 0;
    let mut last_pos = (state.player.x, state.player.y);
    let mut phase_turns = 0; // turns in current phase

    while !state.game_over && !state.victory && turns < max_turns {
        turns += 1;
        phase_turns += 1;
        let hp_before = state.player.hp;
        let alive_before: HashMap<String, i32> = state.level.monsters.iter()
            .filter(|m| m.is_alive())
            .map(|m| (m.id.clone(), m.hp))
            .collect();

        // ── Phase transitions ──
        match phase {
            BotPhase::Explore => {
                if exploration_complete(state) || phase_turns > 800 {
                    phase = BotPhase::Collect;
                    phase_turns = 0;
                }
            }
            BotPhase::Collect => {
                let has_items = state.level.items.iter().any(|it| {
                    state.level.revealed.contains(&(it.x, it.y))
                        && (it.item_type == "weapon" || it.item_type == "armor" || it.item_type == "key")
                        && astar(&state.level, (state.player.x, state.player.y),
                                 (it.x, it.y), &HashSet::new(), state.player.keys > 0).is_some()
                });
                if !has_items || phase_turns > 300 {
                    phase = if strategy == Strategy::Rush { BotPhase::Boss }
                            else { BotPhase::Clear };
                    phase_turns = 0;
                }
            }
            BotPhase::Clear => {
                if no_reachable_non_boss(state) || phase_turns > 600 {
                    phase = BotPhase::Boss;
                    phase_turns = 0;
                }
            }
            BotPhase::Boss => {} // terminal phase
        }

        // ── Stuck detection: random walk if no movement for 8 turns ──
        let cur_pos = (state.player.x, state.player.y);
        if cur_pos == last_pos { stuck_turns += 1; } else { stuck_turns = 0; }
        last_pos = cur_pos;

        let px = state.player.x;
        let py = state.player.y;
        let mon_tiles = monster_positions(state);
        let has_keys = state.player.keys > 0;

        // ── Consumable decisions ──
        let hp_pct = state.player.hp as f32 / state.player.max_hp as f32;

        // Adjacent monster check (are we in combat?)
        let in_combat = state.level.monsters.iter().any(|m| {
            if !m.is_alive() { return false; }
            if m.is_boss {
                m.boss_body.iter().any(|&(bx, by)| (bx - px).abs() + (by - py).abs() <= 1)
            } else {
                (m.x - px).abs() + (m.y - py).abs() <= 1
            }
        });

        // Antidote: use when near damage tiles and not immune (covers whole level)
        if state.player.antidotes > 0 && state.player.antidote_steps == 0 {
            // Check if this level has any damage tiles at all
            let has_damage_tiles = state.level.tile_defs.values().any(|t| t.damage > 0);
            if has_damage_tiles {
                game::use_antidote(state);
            }
        }

        // Heal: in combat below 40%, or out of combat below 25%
        if state.player.potions > 0 {
            if (in_combat && hp_pct < 0.40) || hp_pct < 0.25 {
                game::use_potion(state);
                potions_used += 1;
                do_monster_turn(state);
                if state.player.hp < lowest_hp { lowest_hp = state.player.hp; }
                track_combat(&state, &alive_before, &mut kills, &mut damage_dealt);
                let hp_after = state.player.hp;
                if hp_after < hp_before { damage_taken += hp_before - hp_after; }
                continue;
            }
        }

        // Bomb: 3+ monsters in range, or fighting boss and boss is nearby
        if state.player.bombs > 0 {
            let nearby_count = state.level.monsters.iter()
                .filter(|m| m.is_alive())
                .filter(|m| {
                    if m.is_boss {
                        m.boss_body.iter().any(|&(bx, by)| (bx-px).abs() <= 3 && (by-py).abs() <= 3)
                    } else {
                        (m.x-px).abs() <= 3 && (m.y-py).abs() <= 3
                    }
                })
                .count();
            if nearby_count >= 3 || (phase == BotPhase::Boss && nearby_count >= 1) {
                game::use_bomb(state);
                bombs_used += 1;
                do_monster_turn(state);
                if state.player.hp < lowest_hp { lowest_hp = state.player.hp; }
                track_combat(&state, &alive_before, &mut kills, &mut damage_dealt);
                let hp_after = state.player.hp;
                if hp_after < hp_before { damage_taken += hp_before - hp_after; }
                continue;
            }
        }

        // Speed potion: boss is near and we're in boss phase
        if state.player.speed_potions > 0 && phase == BotPhase::Boss {
            let boss_near = state.level.monsters.iter().any(|m| {
                m.is_boss && m.is_alive()
                    && m.boss_body.iter().any(|&(bx, by)| (bx-px).abs() <= 3 && (by-py).abs() <= 3)
            });
            if boss_near {
                game::use_speed_potion(state);
                speed_potions_used += 1;
                // No monster turn (speed potion freezes them)
                continue;
            }
        }

        // ── Movement decision ──
        let target = if stuck_turns >= 8 {
            // Random walk to unstick
            stuck_turns = 0;
            None
        } else {
            match phase {
                BotPhase::Explore => pick_explore_target(state, &mon_tiles),
                BotPhase::Collect => pick_collect_target(state, &mon_tiles),
                BotPhase::Clear => pick_clear_target(state, &mon_tiles),
                BotPhase::Boss => pick_boss_target(state),
            }
        };

        if let Some(target_pos) = target {
            // Path: avoid monsters during Explore/Collect, walk through during Clear/Boss
            let path = match phase {
                BotPhase::Explore | BotPhase::Collect => {
                    path_to(&state.level, (px, py), target_pos, &mon_tiles, has_keys)
                }
                BotPhase::Clear | BotPhase::Boss => {
                    astar(&state.level, (px, py), target_pos, &HashSet::new(), has_keys)
                }
            };

            if let Some(path) = path {
                if path.len() >= 2 {
                    let next = path[1];
                    game::try_move(state, next.0 - px, next.1 - py);
                    do_monster_turn(state);
                }
            } else {
                // Can't reach target, try direct move
                let dx = (target_pos.0 - px).signum();
                let dy = (target_pos.1 - py).signum();
                if dx != 0 || dy != 0 {
                    let (mx, my) = if (target_pos.0 - px).abs() >= (target_pos.1 - py).abs() { (dx, 0) } else { (0, dy) };
                    game::try_move(state, mx, my);
                    do_monster_turn(state);
                }
            }
        } else {
            // Random walk
            use rand::seq::SliceRandom;
            let mut rng = rand::thread_rng();
            let dirs: [(i32,i32); 4] = [(0,-1),(0,1),(-1,0),(1,0)];
            let safe: Vec<(i32,i32)> = dirs.iter().filter(|&&(dx,dy)| {
                let nx = px + dx;
                let ny = py + dy;
                nx >= 0 && ny >= 0 && nx < state.level.width && ny < state.level.height && {
                    let tile = &state.level.tiles[ny as usize][nx as usize];
                    state.level.tile_defs.get(tile).map_or(false, |t| t.walkable && t.damage == 0)
                }
            }).copied().collect();
            if let Some(&(dx, dy)) = safe.choose(&mut rng) {
                game::try_move(state, dx, dy);
                do_monster_turn(state);
            }
        }

        // ── Track stats ──
        let hp_after = state.player.hp;
        if hp_after < hp_before { damage_taken += hp_before - hp_after; }
        if state.player.hp < lowest_hp && state.player.hp > 0 { lowest_hp = state.player.hp; }
        track_combat(&state, &alive_before, &mut kills, &mut damage_dealt);
    }

    let cause_of_death = if state.game_over {
        if state.player.hp <= 0 { lowest_hp = 0; }
        state.log.iter().rev()
            .find(|e| e.text.contains("died"))
            .map(|e| e.text.clone())
            .or(Some("killed in combat".into()))
    } else if !state.victory {
        Some(format!("timeout after {} turns in {:?} phase", max_turns, phase))
    } else {
        None
    };

    let final_max_hp = state.player.max_hp.max(initial_max_hp);
    let difficulty = rate_difficulty(lowest_hp, final_max_hp, potions_used, damage_taken, state.victory);

    LevelResult {
        victory: state.victory,
        turns, kills, damage_dealt, damage_taken, potions_used, bombs_used, speed_potions_used,
        player_level_before,
        player_level_after: state.player.level,
        player_hp_at_end: state.player.hp,
        lowest_hp,
        cause_of_death,
        level_title, boss_name, boss_hp, monster_count,
        difficulty,
    }
}

fn do_monster_turn(state: &mut GameState) {
    if state.game_over || state.victory { return; }
    if state.player.speed_turns > 0 {
        state.player.speed_turns -= 1;
    } else {
        game::monster_turns(state);
    }
}

fn track_combat(state: &GameState, alive_before: &HashMap<String, i32>, kills: &mut i32, damage_dealt: &mut i32) {
    for m in &state.level.monsters {
        if let Some(&old_hp) = alive_before.get(&m.id) {
            if m.hp < old_hp { *damage_dealt += old_hp - m.hp; }
            if old_hp > 0 && m.hp <= 0 { *kills += 1; }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════
//  Campaign simulation — plays an entire campaign
// ══════════════════════════════════════════════════════════════════════

struct CampaignResult {
    campaign_name: String,
    campaign_index: usize,
    completed: bool,
    levels: Vec<LevelResult>,
    player_at_end: PlayerSnapshot,
    died_on_level: Option<String>,
    store_purchases: Vec<String>,
    store_available: Vec<(String, i32, i32)>, // (name, price, stock) before buying
}

#[derive(Clone)]
struct PlayerSnapshot {
    level: i32,
    max_hp: i32,
    attack: i32,
    defense: i32,
    gold: i32,
    potions: i32,
    bombs: i32,
    speed_potions: i32,
    xp: i32,
    xp_to_next: i32,
    potion_cap: i32,
    antidotes: i32,
    scout_maps: i32,
}

impl From<&Player> for PlayerSnapshot {
    fn from(p: &Player) -> Self {
        Self {
            level: p.level, max_hp: p.max_hp, attack: p.attack, defense: p.defense,
            gold: p.gold, potions: p.potions, bombs: p.bombs, speed_potions: p.speed_potions,
            xp: p.xp, xp_to_next: p.xp_to_next, potion_cap: p.potion_cap,
            antidotes: p.antidotes, scout_maps: p.scout_maps,
        }
    }
}

fn apply_snapshot(player: &mut Player, snap: &PlayerSnapshot) {
    player.level = snap.level;
    player.max_hp = snap.max_hp;
    player.hp = snap.max_hp;
    player.attack = snap.attack;
    player.defense = snap.defense;
    player.gold = snap.gold;
    player.potions = snap.potions;
    player.bombs = snap.bombs;
    player.speed_potions = snap.speed_potions;
    player.xp = snap.xp;
    player.xp_to_next = snap.xp_to_next;
    player.potion_cap = snap.potion_cap;
    player.antidotes = snap.antidotes;
    player.scout_maps = snap.scout_maps;
}

/// Store buy instructions: list of (item_type, quantity) to purchase.
/// item_type is "potion", "speed_potion", or "bomb".
type StoreBuyPlan = Vec<(String, i32)>;

fn simulate_campaign(
    campaign: &gen::BundledCampaign,
    campaign_idx: usize,
    player_snapshot: &PlayerSnapshot,
    strategy: Strategy,
) -> CampaignResult {
    simulate_campaign_with_store(campaign, campaign_idx, player_snapshot, strategy, &vec![])
}

fn simulate_campaign_with_store(
    campaign: &gen::BundledCampaign,
    campaign_idx: usize,
    player_snapshot: &PlayerSnapshot,
    strategy: Strategy,
    buy_plan: &StoreBuyPlan,
) -> CampaignResult {
    let mut ow = match gen::build_overworld_from_result(campaign.overworld.clone()) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[sim] Failed to build overworld: {}", e);
            return CampaignResult {
                campaign_name: campaign.overworld.name.clone(),
                campaign_index: campaign_idx,
                completed: false,
                levels: vec![],
                player_at_end: player_snapshot.clone(),
                died_on_level: Some(format!("overworld build error: {}", e)),
                store_purchases: vec![],
                store_available: vec![],
            };
        }
    };
    ow.scale_store_prices(campaign_idx as i32);

    let mut state = GameState::new();
    apply_snapshot(&mut state.player, player_snapshot);

    let mut level_results = Vec::new();
    let mut died_on = None;
    let mut store_purchases = Vec::new();

    // Record store stock before buying
    let store_available: Vec<(String, i32, i32)> = ow.store_stock.iter()
        .map(|s| (format!("{} ({})", s.name, s.item_type), s.price, s.stock))
        .collect();

    // Execute store buy plan
    let mut ow = ow;
    for (item_type, qty) in buy_plan {
        for _ in 0..*qty {
            if let Some(slot) = ow.store_stock.iter_mut().find(|s| &s.item_type == item_type && s.stock > 0) {
                if state.player.gold >= slot.price {
                    state.player.gold -= slot.price;
                    slot.stock -= 1;
                    let val = slot.value;
                    match item_type.as_str() {
                        "potion" => state.player.potions = (state.player.potions + 1).min(state.player.potion_cap),
                        "speed_potion" => state.player.speed_potions += 1,
                        "bomb" => state.player.bombs += 1,
                        "max_hp" => {
                            state.player.max_hp += val;
                            state.player.hp += val;
                        }
                        "potion_cap" => {
                            state.player.potion_cap = (state.player.potion_cap + val).min(30);
                        }
                        "scout_map" => {
                            state.player.scout_maps += 1;
                        }
                        "antidote" => {
                            state.player.antidotes = (state.player.antidotes + 1).min(3);
                        }
                        _ => {}
                    }
                    store_purchases.push(format!("{} for {}g", slot.name, slot.price));
                }
            }
        }
    }

    // Walk the main path: nodes that are Level type, in connection order
    let main_path = find_main_path(&ow);
    let mut design_idx = 0;

    for &node_idx in &main_path {
        let node = &ow.nodes[node_idx];
        if node.node_type != game::NodeType::Level { continue; }

        // Build the level
        let config = gen::LevelConfig {
            title: node.name.clone(),
            font: node.font.clone(),
            description: node.description.clone(),
            theme: node.theme.clone(),
            palette: node.palette.clone(),
            budget: node.budget,
            floor: node_idx as i32 + 1,
            campaign_tier: campaign_idx as i32,
        };

        let design = match campaign.designs.get(design_idx) {
            Some(d) => d,
            None => {
                eprintln!("[sim] No design for level {}", design_idx);
                break;
            }
        };
        design_idx += 1;

        let (level, start, _remaining) = match gen::build_level_from_design_with_settings(
            &config, design, &campaign.settings, campaign.monster_templates.as_deref(), &Default::default(),
        ) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[sim] Level gen failed: {}", e);
                continue;
            }
        };

        state.level = level;
        state.player.x = start[0];
        state.player.y = start[1];
        state.player.floor = node_idx as i32 + 1;
        state.game_over = false;
        state.victory = false;
        state.log.clear();
        // Auto-use scout map on level entry
        if state.player.scout_maps > 0 {
            game::use_scout_map(&mut state);
        } else {
            game::reveal_around(&mut state.level, state.player.x, state.player.y, state.vision_radius);
        }
        // Auto-use antidote if level has damage tiles
        if state.player.antidotes > 0 {
            let has_damage = state.level.tile_defs.values().any(|t| t.damage > 0);
            if has_damage {
                game::use_antidote(&mut state);
            }
        }

        // Play the level
        let result = play_level(&mut state, strategy);
        let won = result.victory;
        level_results.push(result);

        if !won {
            died_on = Some(node.name.clone());
            break;
        }

        // Between-level transition: keep everything, restore HP
        state.player.hp = state.player.max_hp;
        state.player.speed_turns = 0;
        state.victory = false;
        state.log.clear();
    }

    let completed = died_on.is_none() && !level_results.is_empty();

    CampaignResult {
        campaign_name: campaign.overworld.name.clone(),
        campaign_index: campaign_idx,
        completed,
        levels: level_results,
        player_at_end: PlayerSnapshot::from(&state.player),
        died_on_level: died_on,
        store_purchases,
        store_available,
    }
}

/// Walk connections from node 0 to find the main path of Level nodes.
fn find_main_path(ow: &game::Overworld) -> Vec<usize> {
    let mut path = vec![0usize];
    let mut visited = HashSet::new();
    visited.insert(0);

    loop {
        let cur = *path.last().unwrap();
        // Find connected nodes we haven't visited, preferring main path (lower indices)
        let mut neighbors: Vec<usize> = ow.connections.iter()
            .filter_map(|&(a, b)| {
                if a == cur && !visited.contains(&b) { Some(b) }
                else if b == cur && !visited.contains(&a) { Some(a) }
                else { None }
            })
            .collect();
        neighbors.sort();

        // Prefer Level nodes, then follow the chain
        if let Some(&next) = neighbors.iter().find(|&&n| ow.nodes[n].node_type == game::NodeType::Level) {
            visited.insert(next);
            path.push(next);
            if ow.nodes[next].is_final { break; }
        } else {
            break;
        }
    }
    path
}

fn campaign_result_to_json(r: &CampaignResult) -> Value {
    let levels: Vec<Value> = r.levels.iter().map(|l| json!({
        "title": l.level_title,
        "victory": l.victory,
        "difficulty": l.difficulty,
        "turns": l.turns,
        "kills": l.kills,
        "damage_dealt": l.damage_dealt,
        "damage_taken": l.damage_taken,
        "potions_used": l.potions_used,
        "bombs_used": l.bombs_used,
        "speed_potions_used": l.speed_potions_used,
        "lowest_hp": l.lowest_hp,
        "player_level_before": l.player_level_before,
        "player_level_after": l.player_level_after,
        "hp_at_end": l.player_hp_at_end,
        "cause_of_death": l.cause_of_death,
        "boss_name": l.boss_name,
        "boss_hp": l.boss_hp,
        "monster_count": l.monster_count,
    })).collect();

    json!({
        "campaign_name": r.campaign_name,
        "campaign_index": r.campaign_index,
        "completed": r.completed,
        "died_on_level": r.died_on_level,
        "levels_played": r.levels.len(),
        "total_kills": r.levels.iter().map(|l| l.kills).sum::<i32>(),
        "total_damage_dealt": r.levels.iter().map(|l| l.damage_dealt).sum::<i32>(),
        "total_damage_taken": r.levels.iter().map(|l| l.damage_taken).sum::<i32>(),
        "total_potions_used": r.levels.iter().map(|l| l.potions_used).sum::<i32>(),
        "total_bombs_used": r.levels.iter().map(|l| l.bombs_used).sum::<i32>(),
        "total_turns": r.levels.iter().map(|l| l.turns).sum::<i32>(),
        "player_at_end": {
            "level": r.player_at_end.level,
            "max_hp": r.player_at_end.max_hp,
            "attack": r.player_at_end.attack,
            "defense": r.player_at_end.defense,
            "gold": r.player_at_end.gold,
            "potions": r.player_at_end.potions,
            "bombs": r.player_at_end.bombs,
            "speed_potions": r.player_at_end.speed_potions,
            "potion_cap": r.player_at_end.potion_cap,
            "antidotes": r.player_at_end.antidotes,
            "scout_maps": r.player_at_end.scout_maps,
        },
        "store_available": r.store_available.iter().map(|(n, p, s)| json!({"name": n, "price": p, "stock": s})).collect::<Vec<_>>(),
        "store_purchases": r.store_purchases,
        "levels": levels,
    })
}

// ══════════════════════════════════════════════════════════════════════
//  MCP protocol
// ══════════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
#[allow(dead_code)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(serde::Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(serde::Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "list_campaigns",
            "description": "List all available campaigns with names, descriptions, level count, and budgets.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "simulate_campaign",
            "description": "Run a bot through a single campaign autonomously. The bot uses A* pathfinding and combat AI to play each level. Returns detailed per-level stats: turns, kills, damage dealt/taken, potions used, player level changes, and whether the campaign was completed or the player died. Strategy options: 'thorough' (clear everything), 'rush' (beeline for boss), 'balanced' (default).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "campaign_index": { "type": "integer", "description": "Campaign index (0-based)" },
                    "strategy": { "type": "string", "enum": ["thorough", "rush", "balanced"], "default": "balanced" },
                    "player_level": { "type": "integer", "description": "Starting player level (default 1). Simulates a player who has leveled up from prior campaigns." },
                    "player_gold": { "type": "integer", "description": "Starting gold (default 0)" },
                    "player_potions": { "type": "integer", "description": "Starting potions (default 1)" }
                },
                "required": ["campaign_index"]
            }
        },
        {
            "name": "play_campaign",
            "description": "Play a single campaign with full control over player state and store purchases. Use this to chain campaigns manually with persistent progression. Pass the player_at_end from the previous campaign as input. Store purchases are executed before any levels are played.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "campaign_index": { "type": "integer", "description": "Campaign index (0-based)" },
                    "strategy": { "type": "string", "enum": ["thorough", "rush", "balanced"], "default": "balanced" },
                    "player_level": { "type": "integer", "description": "Player level" },
                    "player_gold": { "type": "integer", "description": "Gold" },
                    "player_potions": { "type": "integer", "description": "Health potions" },
                    "player_bombs": { "type": "integer", "description": "Bombs" },
                    "player_speed_potions": { "type": "integer", "description": "Speed potions" },
                    "buy_potions": { "type": "integer", "description": "Number of healing potions to buy from store (default 0)" },
                    "buy_bombs": { "type": "integer", "description": "Number of bombs to buy from store (default 0)" },
                    "buy_speed_potions": { "type": "integer", "description": "Number of speed potions to buy from store (default 0)" },
                    "buy_potion_cap": { "type": "boolean", "description": "Buy a Potion Pouch (+5 potion capacity, 150g scaled by tier)" },
                    "buy_max_hp": { "type": "boolean", "description": "Buy a Vitality Charm (+15 max HP, 100g scaled by tier)" },
                    "buy_antidotes": { "type": "integer", "description": "Number of antidotes to buy (max 3, 50g each scaled by tier)" },
                    "buy_scout_maps": { "type": "integer", "description": "Number of scout maps to buy (max 2, 75g each scaled by tier)" },
                    "player_potion_cap": { "type": "integer", "description": "Current potion capacity (default 10)" },
                    "player_antidotes": { "type": "integer", "description": "Current antidotes (default 0)" },
                    "player_scout_maps": { "type": "integer", "description": "Current scout maps (default 0)" }
                },
                "required": ["campaign_index"]
            }
        },
        {
            "name": "simulate_sequence",
            "description": "Play through multiple campaigns in order with persistent player leveling (like a real player would). Returns the balance curve: how player power scales vs enemy difficulty across campaigns. Use this to answer 'is the game too easy/hard after 10 campaigns?'",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "count": { "type": "integer", "description": "Number of campaigns to play through (default: all)" },
                    "strategy": { "type": "string", "enum": ["thorough", "rush", "balanced"], "default": "balanced" },
                    "start_campaign": { "type": "integer", "description": "Campaign index to start from (default 0)" }
                }
            }
        },
        {
            "name": "analyze_difficulty",
            "description": "Static analysis of a campaign's difficulty curve WITHOUT playing it. Shows budget allocation, boss HP scaling, monster counts per level, and how it compares to player power at a given level.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "campaign_index": { "type": "integer", "description": "Campaign index (0-based)" },
                    "player_level": { "type": "integer", "description": "Assumed player level for power comparison (default 1)" }
                },
                "required": ["campaign_index"]
            }
        },
        {
            "name": "stress_test",
            "description": "Run a campaign N times with randomized level generation and report win rate, average deaths, and variance. Good for checking if a campaign is consistently beatable or relies on lucky RNG.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "campaign_index": { "type": "integer", "description": "Campaign index" },
                    "runs": { "type": "integer", "description": "Number of runs (default 10)" },
                    "strategy": { "type": "string", "enum": ["thorough", "rush", "balanced"], "default": "balanced" },
                    "player_level": { "type": "integer", "description": "Starting player level (default 1)" }
                },
                "required": ["campaign_index"]
            }
        }
    ])
}

struct Session {
    campaigns: Vec<gen::BundledCampaign>,
}

fn make_player_snapshot(level: i32, gold: i32, potions: i32) -> PlayerSnapshot {
    // Compute stats for a player at the given level
    let mut p = Player::default();
    for _ in 1..level {
        p.max_hp += 5;
        p.attack += 1;
        p.defense += 1;
        p.xp_to_next = (p.xp_to_next as f64 * 1.25) as i32;
    }
    p.level = level;
    p.hp = p.max_hp;
    p.gold = gold;
    p.potions = potions;
    PlayerSnapshot::from(&p)
}

fn handle_tool(session: &Session, name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "list_campaigns" => {
            let campaigns: Vec<Value> = session.campaigns.iter().enumerate().map(|(i, c)| {
                let budgets: Vec<i32> = c.overworld.levels.iter().map(|l| l.budget).collect();
                json!({
                    "index": i,
                    "name": c.overworld.name,
                    "description": c.overworld.description,
                    "levels": c.overworld.levels.len(),
                    "level_names": c.overworld.levels.iter().map(|l| &l.name).collect::<Vec<_>>(),
                    "budgets": budgets,
                    "total_budget": budgets.iter().sum::<i32>(),
                })
            }).collect();
            Ok(json!({ "campaigns": campaigns, "total": campaigns.len() }))
        }

        "simulate_campaign" => {
            let idx = args["campaign_index"].as_i64().ok_or("missing campaign_index")? as usize;
            if idx >= session.campaigns.len() {
                return Err(format!("index {} out of range", idx));
            }
            let strategy = Strategy::from_str(args["strategy"].as_str().unwrap_or("balanced"));
            let level = args["player_level"].as_i64().unwrap_or(1) as i32;
            let gold = args["player_gold"].as_i64().unwrap_or(0) as i32;
            let potions = args["player_potions"].as_i64().unwrap_or(1) as i32;
            let snap = make_player_snapshot(level, gold, potions);

            let result = simulate_campaign(&session.campaigns[idx], idx, &snap, strategy);
            Ok(campaign_result_to_json(&result))
        }

        "play_campaign" => {
            let idx = args["campaign_index"].as_i64().ok_or("missing campaign_index")? as usize;
            if idx >= session.campaigns.len() {
                return Err(format!("index {} out of range", idx));
            }
            let strategy = Strategy::from_str(args["strategy"].as_str().unwrap_or("balanced"));
            let level = args["player_level"].as_i64().unwrap_or(1) as i32;
            let gold = args["player_gold"].as_i64().unwrap_or(0) as i32;
            let potions = args["player_potions"].as_i64().unwrap_or(1) as i32;
            let bombs = args["player_bombs"].as_i64().unwrap_or(0) as i32;
            let speed_potions = args["player_speed_potions"].as_i64().unwrap_or(0) as i32;

            let potion_cap = args["player_potion_cap"].as_i64().unwrap_or(10) as i32;
            let antidotes = args["player_antidotes"].as_i64().unwrap_or(0) as i32;
            let scout_maps = args["player_scout_maps"].as_i64().unwrap_or(0) as i32;

            let mut snap = make_player_snapshot(level, gold, potions);
            snap.bombs = bombs;
            snap.speed_potions = speed_potions;
            snap.potion_cap = potion_cap;
            snap.antidotes = antidotes;
            snap.scout_maps = scout_maps;

            let mut buy_plan: StoreBuyPlan = vec![];
            let bp = args["buy_potions"].as_i64().unwrap_or(0) as i32;
            let bb = args["buy_bombs"].as_i64().unwrap_or(0) as i32;
            let bs = args["buy_speed_potions"].as_i64().unwrap_or(0) as i32;
            let bpc = args["buy_potion_cap"].as_bool().unwrap_or(false);
            let bmh = args["buy_max_hp"].as_bool().unwrap_or(false);
            let ba = args["buy_antidotes"].as_i64().unwrap_or(0) as i32;
            let bsm = args["buy_scout_maps"].as_i64().unwrap_or(0) as i32;
            if bp > 0 { buy_plan.push(("potion".into(), bp)); }
            if bb > 0 { buy_plan.push(("bomb".into(), bb)); }
            if bs > 0 { buy_plan.push(("speed_potion".into(), bs)); }
            if bpc { buy_plan.push(("potion_cap".into(), 1)); }
            if bmh { buy_plan.push(("max_hp".into(), 1)); }
            if ba > 0 { buy_plan.push(("antidote".into(), ba)); }
            if bsm > 0 { buy_plan.push(("scout_map".into(), bsm)); }

            let result = simulate_campaign_with_store(&session.campaigns[idx], idx, &snap, strategy, &buy_plan);
            Ok(campaign_result_to_json(&result))
        }

        "simulate_sequence" => {
            let count = args["count"].as_i64().unwrap_or(session.campaigns.len() as i64) as usize;
            let start = args["start_campaign"].as_i64().unwrap_or(0) as usize;
            let strategy = Strategy::from_str(args["strategy"].as_str().unwrap_or("balanced"));
            let count = count.min(session.campaigns.len() - start);

            let mut snap = make_player_snapshot(1, 0, 1);
            let mut results = Vec::new();
            let mut balance_curve = Vec::new();

            for i in start..(start + count) {
                let r = simulate_campaign(&session.campaigns[i], i, &snap, strategy);
                balance_curve.push(json!({
                    "campaign": i,
                    "campaign_name": r.campaign_name,
                    "completed": r.completed,
                    "player_level_start": snap.level,
                    "player_level_end": r.player_at_end.level,
                    "player_attack_end": r.player_at_end.attack,
                    "player_defense_end": r.player_at_end.defense,
                    "player_max_hp_end": r.player_at_end.max_hp,
                    "player_gold_end": r.player_at_end.gold,
                    "died_on": r.died_on_level,
                    "levels_played": r.levels.len(),
                    "total_kills": r.levels.iter().map(|l| l.kills).sum::<i32>(),
                    "total_damage_taken": r.levels.iter().map(|l| l.damage_taken).sum::<i32>(),
                    "total_potions_used": r.levels.iter().map(|l| l.potions_used).sum::<i32>(),
                }));

                // Persist player state to next campaign (even if died — they keep their level)
                if r.completed {
                    snap = r.player_at_end.clone();
                } else {
                    // On death, keep the level/stats but restore HP
                    snap = r.player_at_end.clone();
                }

                results.push(campaign_result_to_json(&r));
            }

            Ok(json!({
                "campaigns_played": results.len(),
                "campaigns_completed": results.iter().filter(|r| r["completed"].as_bool() == Some(true)).count(),
                "balance_curve": balance_curve,
                "campaigns": results,
            }))
        }

        "analyze_difficulty" => {
            let idx = args["campaign_index"].as_i64().ok_or("missing campaign_index")? as usize;
            if idx >= session.campaigns.len() {
                return Err(format!("index {} out of range", idx));
            }
            let player_level = args["player_level"].as_i64().unwrap_or(1) as i32;
            let campaign = &session.campaigns[idx];

            let player_snap = make_player_snapshot(player_level, 0, 1);
            let player_total_atk = player_snap.attack; // no weapon
            let player_total_def = player_snap.defense; // no armor

            let ow = gen::build_overworld_from_result(campaign.overworld.clone())
                .map_err(|e| format!("overworld error: {}", e))?;

            let mut levels = Vec::new();
            for (i, design) in campaign.designs.iter().enumerate() {
                let node = ow.nodes.iter().filter(|n| n.node_type == game::NodeType::Level).nth(i);
                let budget = node.map(|n| n.budget).unwrap_or(0);
                let floor = i as i32 + 1;
                let tier_scale = 1.0 + idx as f32 * 0.25;
                let scaled_budget = (budget as f32 * tier_scale).round() as i32;

                let hp_per_point = 2.0 + floor as f32 * 0.7;
                let atk_scale = 3 + floor;

                // Estimate boss stats (using midpoint of 15-25% budget range)
                let boss_cost = scaled_budget * 20 / 100;
                let boss_hp = (boss_cost as f32 * hp_per_point).round() as i32;
                let boss_atk = atk_scale + boss_cost / 12;

                // Estimate how many monsters fit in remaining budget
                let remaining = scaled_budget - boss_cost;
                let avg_mon_cost = 6;
                let est_monsters = remaining / avg_mon_cost;

                levels.push(json!({
                    "level": i + 1,
                    "name": node.map(|n| n.name.as_str()).unwrap_or("?"),
                    "budget": budget,
                    "scaled_budget": scaled_budget,
                    "est_boss_hp": boss_hp,
                    "est_boss_atk": boss_atk,
                    "est_monster_count": est_monsters,
                    "boss_name": design.boss.name,
                    "monster_types": design.monster_types.iter().map(|m| &m.name).collect::<Vec<_>>(),
                    "weapon": design.weapon.name,
                    "armor": design.armor.name,
                    "player_can_hit_for": (player_total_atk - boss_atk / 4).max(1),
                    "boss_can_hit_for": (boss_atk - player_total_def / 2).max(1),
                    "est_turns_to_kill_boss": if (player_total_atk - 0).max(1) > 0 {
                        boss_hp / (player_total_atk).max(1)
                    } else { 999 },
                }));
            }

            Ok(json!({
                "campaign_name": campaign.overworld.name,
                "campaign_index": idx,
                "player_level": player_level,
                "player_stats": {
                    "max_hp": player_snap.max_hp,
                    "attack": player_snap.attack,
                    "defense": player_snap.defense,
                },
                "tier_scale": 1.0 + idx as f32 * 0.25,
                "levels": levels,
            }))
        }

        "stress_test" => {
            let idx = args["campaign_index"].as_i64().ok_or("missing campaign_index")? as usize;
            if idx >= session.campaigns.len() {
                return Err(format!("index {} out of range", idx));
            }
            let runs = args["runs"].as_i64().unwrap_or(10) as usize;
            let strategy = Strategy::from_str(args["strategy"].as_str().unwrap_or("balanced"));
            let player_level = args["player_level"].as_i64().unwrap_or(1) as i32;
            let snap = make_player_snapshot(player_level, 0, 1);

            let mut wins = 0;
            let mut total_deaths = 0;
            let mut death_levels: HashMap<String, i32> = HashMap::new();
            let mut levels_reached = Vec::new();

            for _ in 0..runs {
                let r = simulate_campaign(&session.campaigns[idx], idx, &snap, strategy);
                if r.completed { wins += 1; }
                if let Some(ref dl) = r.died_on_level {
                    total_deaths += 1;
                    *death_levels.entry(dl.clone()).or_insert(0) += 1;
                }
                levels_reached.push(r.levels.len());
            }

            let avg_levels = levels_reached.iter().sum::<usize>() as f64 / runs as f64;

            Ok(json!({
                "campaign_name": session.campaigns[idx].overworld.name,
                "runs": runs,
                "wins": wins,
                "win_rate": wins as f64 / runs as f64,
                "deaths": total_deaths,
                "avg_levels_reached": avg_levels,
                "death_breakdown": death_levels,
                "player_level": player_level,
            }))
        }

        _ => Err(format!("unknown tool: {}", name)),
    }
}

fn handle_request(session: &Session, req: &JsonRpcRequest) -> Value {
    match req.method.as_str() {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "scapegrace-playtest", "version": "0.2.0" }
        }),
        "notifications/initialized" => Value::Null,
        "tools/list" => json!({ "tools": tool_definitions() }),
        "tools/call" => {
            let tool_name = req.params["name"].as_str().unwrap_or("");
            let args = &req.params["arguments"];
            match handle_tool(session, tool_name, args) {
                Ok(v) => json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap() }]
                }),
                Err(e) => json!({
                    "content": [{ "type": "text", "text": e }],
                    "isError": true
                }),
            }
        }
        "ping" => json!({}),
        _ => json!({"error": format!("unknown method: {}", req.method)}),
    }
}

fn main() {
    eprintln!("[mcp] Scapegrace playtest server v0.2 starting...");

    let campaigns = gen::load_bundled_pack()
        .map(|p| p.campaigns)
        .unwrap_or_default();
    eprintln!("[mcp] Loaded {} campaigns", campaigns.len());

    let session = Session { campaigns };
    let stdin = io::stdin();
    let stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        let line = line.trim().to_string();
        if line.is_empty() { continue; }

        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => { eprintln!("[mcp] parse error: {}", e); continue; }
        };

        let is_notification = req.id.is_none();
        let result = handle_request(&session, &req);

        if is_notification || result.is_null() { continue; }

        let response = JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req.id.unwrap_or(Value::Null),
            result: Some(result),
            error: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        let mut out = stdout.lock();
        let _ = writeln!(out, "{}", json);
        let _ = out.flush();
    }
}
