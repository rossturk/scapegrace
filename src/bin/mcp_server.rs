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

            // Penalize damage tiles
            let cost = if level.tile_defs.get(tile).map_or(false, |t| t.damage > 0) { 5 } else { 1 };
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
//  Bot AI — plays a single level to completion or death
// ══════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy)]
enum Strategy {
    /// Clear all monsters, pick up everything, then fight boss
    Thorough,
    /// Beeline for boss, only fight what's in the way
    Rush,
    /// Clear monsters near items/path, balanced approach
    Balanced,
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
    #[allow(dead_code)]
    items_collected: Vec<String>,
    player_level_before: i32,
    player_level_after: i32,
    player_hp_at_end: i32,
    cause_of_death: Option<String>,
    level_title: String,
    boss_name: String,
    boss_hp: i32,
    monster_count: i32,
}

fn play_level(state: &mut GameState, strategy: Strategy) -> LevelResult {
    let level_title = state.level.title.clone();
    let boss_name = state.level.monsters.iter()
        .find(|m| m.is_boss).map(|m| m.name.clone()).unwrap_or_default();
    let boss_hp = state.level.monsters.iter()
        .find(|m| m.is_boss).map(|m| m.max_hp).unwrap_or(0);
    let monster_count = state.level.monsters.iter().filter(|m| !m.is_boss && m.is_alive()).count() as i32;
    let player_level_before = state.player.level;

    let mut turns = 0;
    let mut kills = 0;
    let mut damage_dealt = 0;
    let mut damage_taken = 0;
    let mut potions_used = 0;
    let mut bombs_used = 0;
    let items_collected = Vec::new();
    let max_turns = 2500; // safety valve
    let mut stuck_counter = 0;
    let mut last_pos = (state.player.x, state.player.y);
    let mut blacklist: HashSet<(i32, i32)> = HashSet::new();
    let mut committed_target: Option<(i32, i32)> = None;
    let mut last_move: (i32, i32) = (0, 0);
    let mut no_progress_turns = 0; // turns since last kill or item pickup
    let mut last_kill_count = 0;
    let mut last_item_count = state.level.items.len();

    while !state.game_over && !state.victory && turns < max_turns {
        turns += 1;
        let hp_before = state.player.hp;
        let alive_before: HashMap<String, i32> = state.level.monsters.iter()
            .filter(|m| m.is_alive())
            .map(|m| (m.id.clone(), m.hp))
            .collect();

        // ── Stuck detection ──
        let cur_pos = (state.player.x, state.player.y);
        let cur_kills = state.level.monsters.iter().filter(|m| !m.is_alive()).count();
        let cur_items = state.level.items.len();

        if cur_kills > last_kill_count || cur_items < last_item_count {
            no_progress_turns = 0;
            last_kill_count = cur_kills;
            last_item_count = cur_items;
        } else {
            no_progress_turns += 1;
        }

        if cur_pos == last_pos {
            stuck_counter += 1;
        } else {
            stuck_counter = 0;
        }

        // After 10 turns in same spot OR 30 turns with no progress, blacklist unreachable targets
        if stuck_counter >= 10 || no_progress_turns >= 30 {
            committed_target = None;
            let no_avoid = HashSet::new();
            let has_keys = state.player.keys > 0;
            for item in &state.level.items {
                if astar(&state.level, cur_pos, (item.x, item.y), &no_avoid, has_keys).is_none() {
                    blacklist.insert((item.x, item.y));
                }
            }
            for m in &state.level.monsters {
                if m.is_alive() && astar(&state.level, cur_pos, (m.x, m.y), &no_avoid, has_keys).is_none() {
                    blacklist.insert((m.x, m.y));
                }
            }
            if stuck_counter >= 10 { stuck_counter = 0; }
            if no_progress_turns >= 30 { no_progress_turns = 0; }
        }
        last_pos = cur_pos;

        // ── Decision: random walk if deeply stuck, otherwise normal AI ──
        let pos_before_action = (state.player.x, state.player.y);
        let action = if stuck_counter >= 5 || no_progress_turns >= 20 {
            // Random walk to unstick: pick a random walkable adjacent tile
            let mut rng = rand::thread_rng();
            use rand::seq::SliceRandom;
            let dirs: [(i32,i32); 4] = [(0,-1),(0,1),(-1,0),(1,0)];
            let walkable_dirs: Vec<(i32,i32)> = dirs.iter().filter(|&&(dx,dy)| {
                let nx = state.player.x + dx;
                let ny = state.player.y + dy;
                nx >= 0 && ny >= 0 && nx < state.level.width && ny < state.level.height
                    && state.level.tile_defs.get(&state.level.tiles[ny as usize][nx as usize])
                        .map_or(false, |t| t.walkable)
            }).copied().collect();
            if let Some(&(dx, dy)) = walkable_dirs.choose(&mut rng) {
                committed_target = None;
                Action::Move(dx, dy)
            } else {
                Action::Wait
            }
        } else {
            decide_action(state, strategy, &blacklist, &mut committed_target, last_move)
        };

        execute_action(state, &action);

        // Track last move direction
        if let Action::Move(dx, dy) = action {
            let actually_moved = (state.player.x, state.player.y) != pos_before_action;
            if actually_moved {
                last_move = (dx, dy);
            }
        }


        // ── Track stats ──
        let hp_after = state.player.hp;
        if hp_after < hp_before { damage_taken += hp_before - hp_after; }

        for m in &state.level.monsters {
            if let Some(&old_hp) = alive_before.get(&m.id) {
                if m.hp < old_hp { damage_dealt += old_hp - m.hp; }
                if old_hp > 0 && m.hp <= 0 { kills += 1; }
            }
        }

        match &action {
            Action::UsePotion => potions_used += 1,
            Action::UseBomb => bombs_used += 1,
            _ => {}
        }
    }

    let cause_of_death = if state.game_over {
        state.log.iter().rev()
            .find(|e| e.text.contains("died") || e.text.contains("killed"))
            .map(|e| e.text.clone())
            .or(Some("killed in combat".into()))
    } else {
        None
    };

    LevelResult {
        victory: state.victory,
        turns,
        kills,
        damage_dealt,
        damage_taken,
        potions_used,
        bombs_used,
        items_collected,
        player_level_before,
        player_level_after: state.player.level,
        player_hp_at_end: state.player.hp,
        cause_of_death,
        level_title,
        boss_name,
        boss_hp,
        monster_count,
    }
}

#[derive(Debug)]
enum Action {
    Move(i32, i32), // dx, dy
    UsePotion,
    UseBomb,
    UseSpeedPotion,
    Wait, // shouldn't happen, safety fallback
}

fn decide_action(state: &GameState, strategy: Strategy, blacklist: &HashSet<(i32, i32)>,
                  committed_target: &mut Option<(i32, i32)>, last_move: (i32, i32)) -> Action {
    let p = &state.player;
    let px = p.x;
    let py = p.y;

    // ── Emergency heal: use potion if low HP ──
    let hp_pct = p.hp as f32 / p.max_hp as f32;
    if hp_pct < 0.35 && p.potions > 0 {
        return Action::UsePotion;
    }

    // ── Bomb if surrounded by 3+ monsters in radius 2 ──
    if p.bombs > 0 {
        let nearby = state.level.monsters.iter()
            .filter(|m| m.is_alive() && !m.is_boss)
            .filter(|m| (m.x - px).abs() <= 3 && (m.y - py).abs() <= 3)
            .count();
        if nearby >= 3 {
            return Action::UseBomb;
        }
    }

    // ── Speed potion if boss is adjacent and enraged ──
    if p.speed_potions > 0 {
        let boss_adjacent = state.level.monsters.iter().any(|m| {
            m.is_boss && m.is_alive() && m.boss_enraged_turns > 0
                && m.boss_body.iter().any(|&(bx, by)| (bx - px).abs() <= 2 && (by - py).abs() <= 2)
        });
        if boss_adjacent {
            return Action::UseSpeedPotion;
        }
    }

    // No avoidance — the bot walks into monsters to attack them (bump combat).
    let no_avoid = HashSet::new();
    let has_keys = p.keys > 0;

    // ── Pick a target (commit to it for multiple turns to avoid oscillation) ──
    // Clear committed target if we've reached it or it's gone
    if let Some(ct) = *committed_target {
        let at_target = (px - ct.0).abs() + (py - ct.1).abs() <= 1;
        let item_gone = !state.level.items.iter().any(|it| it.x == ct.0 && it.y == ct.1);
        let monster_dead = state.level.monsters.iter()
            .find(|m| m.x == ct.0 && m.y == ct.1)
            .map_or(false, |m| !m.is_alive());
        if at_target || (item_gone && monster_dead) {
            *committed_target = None;
        }
    }
    if committed_target.is_none() {
        *committed_target = pick_target(state, strategy, blacklist);
    }
    let target = *committed_target;

    if let Some(target_pos) = target {
        // Path to target (monsters are walkable — bumping attacks them)
        if let Some(path) = astar(&state.level, (px, py), target_pos, &no_avoid, has_keys) {
            if path.len() >= 2 {
                let next = path[1];
                let dx = next.0 - px;
                let dy = next.1 - py;
                // Anti-oscillation: if this reverses our last move, try step 2 or skip
                if (dx, dy) == (-last_move.0, -last_move.1) && last_move != (0, 0) && path.len() >= 3 {
                    let alt = path[2];
                    let adx = (alt.0 - px).signum();
                    let ady = (alt.1 - py).signum();
                    if adx != 0 || ady != 0 {
                        return Action::Move(adx, ady);
                    }
                }
                return Action::Move(dx, dy);
            }
        } else {
            *committed_target = None; // unreachable, pick a new one next turn
        }
        // Can't path? Move directly toward target
        let dx = (target_pos.0 - px).signum();
        let dy = (target_pos.1 - py).signum();
        if dx != 0 || dy != 0 {
            if (target_pos.0 - px).abs() >= (target_pos.1 - py).abs() {
                return Action::Move(dx, 0);
            } else {
                return Action::Move(0, dy);
            }
        }
    } else {
        eprintln!("[bot] no target found at ({},{}) bl={}", px, py, blacklist.len());
    }

    // ── Fallback: explore unrevealed area ──
    if let Some(explore_target) = find_exploration_target(state) {
        if let Some(path) = astar(&state.level, (px, py), explore_target, &no_avoid, has_keys) {
            if path.len() >= 2 {
                let next = path[1];
                return Action::Move(next.0 - px, next.1 - py);
            }
        }
    }

    // ── Last resort: try each direction ──
    for &(dx, dy) in &[(0, -1), (0, 1), (-1, 0), (1, 0)] {
        let nx = px + dx;
        let ny = py + dy;
        if nx >= 0 && ny >= 0 && nx < state.level.width && ny < state.level.height {
            let tile = &state.level.tiles[ny as usize][nx as usize];
            if state.level.tile_defs.get(tile).map_or(false, |t| t.walkable) {
                return Action::Move(dx, dy);
            }
        }
    }

    Action::Wait
}

fn pick_target(state: &GameState, strategy: Strategy, blacklist: &HashSet<(i32, i32)>) -> Option<(i32, i32)> {
    let p = &state.player;
    let px = p.x;
    let py = p.y;
    let no_avoid = HashSet::new();
    let has_keys = p.keys > 0;

    // Helper: is this position reachable (not blacklisted, has a path)?
    let reachable = |x: i32, y: i32| -> bool {
        !blacklist.contains(&(x, y))
            && astar(&state.level, (px, py), (x, y), &no_avoid, has_keys).is_some()
    };

    // Priority 1: always grab keys first (needed to unlock doors to reach boss)
    let key_target = state.level.items.iter()
        .filter(|it| it.item_type == "key")
        .filter(|it| state.level.revealed.contains(&(it.x, it.y)))
        .filter(|it| reachable(it.x, it.y))
        .min_by_key(|it| (it.x - px).abs() + (it.y - py).abs())
        .map(|it| (it.x, it.y));
    if key_target.is_some() { return key_target; }

    // Priority 2: if we have a key, go unlock the locked door
    if p.keys > 0 {
        for y in 0..state.level.height {
            for x in 0..state.level.width {
                if state.level.tiles[y as usize][x as usize] == "locked_door"
                    && state.level.revealed.contains(&(x, y))
                {
                    // Path to a tile adjacent to the door
                    for &(dx, dy) in &[(0, -1), (0, 1), (-1, 0), (1, 0)] {
                        let ax = x + dx;
                        let ay = y + dy;
                        if ax >= 0 && ay >= 0 && ax < state.level.width && ay < state.level.height {
                            let tile = &state.level.tiles[ay as usize][ax as usize];
                            if state.level.tile_defs.get(tile).map_or(false, |t| t.walkable) && reachable(ax, ay) {
                                return Some((x, y)); // walk into the door to unlock it
                            }
                        }
                    }
                }
            }
        }
    }

    // Items sorted by distance, filtered to reachable (prioritize weapons/armor)
    let nearest_item = |max_dist: i32| -> Option<(i32, i32)> {
        // Prioritize weapon/armor pickups
        let equip = state.level.items.iter()
            .filter(|it| it.item_type == "weapon" || it.item_type == "armor")
            .filter(|it| state.level.revealed.contains(&(it.x, it.y)))
            .filter(|it| (it.x - px).abs() + (it.y - py).abs() <= max_dist)
            .filter(|it| reachable(it.x, it.y))
            .min_by_key(|it| (it.x - px).abs() + (it.y - py).abs())
            .map(|it| (it.x, it.y));
        if equip.is_some() { return equip; }

        state.level.items.iter()
            .filter(|it| state.level.revealed.contains(&(it.x, it.y)))
            .filter(|it| (it.x - px).abs() + (it.y - py).abs() <= max_dist)
            .filter(|it| reachable(it.x, it.y))
            .min_by_key(|it| (it.x - px).abs() + (it.y - py).abs())
            .map(|it| (it.x, it.y))
    };

    // Nearest reachable non-boss monster
    let nearest_monster = |max_dist: i32| -> Option<(i32, i32)> {
        state.level.monsters.iter()
            .filter(|m| m.is_alive() && !m.is_boss)
            .filter(|m| (m.x - px).abs() + (m.y - py).abs() <= max_dist)
            .filter(|m| reachable(m.x, m.y))
            .min_by_key(|m| (m.x - px).abs() + (m.y - py).abs())
            .map(|m| (m.x, m.y))
    };

    // Boss target (adjacent tile)
    let boss_target = || -> Option<(i32, i32)> {
        let boss = state.level.monsters.iter().find(|m| m.is_boss && m.is_alive())?;
        let closest_body = boss.boss_body.iter()
            .min_by_key(|&&(bx, by)| (bx - px).abs() + (by - py).abs())
            .copied()
            .unwrap_or((boss.x, boss.y));
        let adj = adjacent_to(state, closest_body);
        adj.filter(|&(x, y)| reachable(x, y)).or(adj)
    };

    match strategy {
        Strategy::Rush => boss_target(),
        Strategy::Thorough => {
            nearest_item(15)
                .or_else(|| nearest_monster(20))
                .or_else(|| nearest_item(999))
                .or_else(boss_target)
        }
        Strategy::Balanced => {
            nearest_item(10)
                .or_else(|| nearest_monster(8))
                .or_else(boss_target)
        }
    }
}

/// Find a walkable tile adjacent to the given position (for pathing to a monster/boss).
fn adjacent_to(state: &GameState, pos: (i32, i32)) -> Option<(i32, i32)> {
    let px = state.player.x;
    let py = state.player.y;
    let mut best: Option<(i32, i32)> = None;
    let mut best_dist = i32::MAX;
    for &(dx, dy) in &[(0, -1), (0, 1), (-1, 0), (1, 0)] {
        let nx = pos.0 + dx;
        let ny = pos.1 + dy;
        if nx < 0 || ny < 0 || nx >= state.level.width || ny >= state.level.height { continue; }
        let tile = &state.level.tiles[ny as usize][nx as usize];
        if !state.level.tile_defs.get(tile).map_or(false, |t| t.walkable) { continue; }
        let d = (nx - px).abs() + (ny - py).abs();
        if d < best_dist { best_dist = d; best = Some((nx, ny)); }
    }
    // If no adjacent walkable tile, just target the position directly (bump attack)
    best.or(Some(pos))
}

fn find_exploration_target(state: &GameState) -> Option<(i32, i32)> {
    let px = state.player.x;
    let py = state.player.y;
    let has_keys = state.player.keys > 0;
    let no_avoid = HashSet::new();

    // Find walkable revealed tiles that border unrevealed tiles
    let mut frontier: Vec<(i32, i32)> = Vec::new();
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

    // Prefer reachable frontier tiles
    let reachable: Vec<(i32, i32)> = frontier.iter()
        .filter(|&&(x, y)| astar(&state.level, (px, py), (x, y), &no_avoid, has_keys).is_some())
        .copied()
        .collect();

    if !reachable.is_empty() {
        reachable.into_iter()
            .min_by_key(|&(x, y)| (x - px).abs() + (y - py).abs())
    } else {
        frontier.into_iter()
            .min_by_key(|&(x, y)| (x - px).abs() + (y - py).abs())
    }
}

fn execute_action(state: &mut GameState, action: &Action) {
    match action {
        Action::Move(dx, dy) => {
            game::try_move(state, *dx, *dy);
        }
        Action::UsePotion => { game::use_potion(state); }
        Action::UseBomb => { game::use_bomb(state); }
        Action::UseSpeedPotion => { game::use_speed_potion(state); }
        Action::Wait => {}
    }

    // Monster turns (unless frozen)
    if !state.game_over && !state.victory {
        if state.player.speed_turns > 0 {
            state.player.speed_turns -= 1;
        } else {
            game::monster_turns(state);
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
}

impl From<&Player> for PlayerSnapshot {
    fn from(p: &Player) -> Self {
        Self {
            level: p.level, max_hp: p.max_hp, attack: p.attack, defense: p.defense,
            gold: p.gold, potions: p.potions, bombs: p.bombs, speed_potions: p.speed_potions,
            xp: p.xp, xp_to_next: p.xp_to_next,
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
    let ow = match gen::build_overworld_from_result(campaign.overworld.clone()) {
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
                    match item_type.as_str() {
                        "potion" => state.player.potions += 1,
                        "speed_potion" => state.player.speed_potions += 1,
                        "bomb" => state.player.bombs += 1,
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
            &config, design, &campaign.settings, campaign.monster_templates.as_deref(),
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
        game::reveal_around(&mut state.level, state.player.x, state.player.y, state.vision_radius);

        // Play the level
        let result = play_level(&mut state, strategy);
        let won = result.victory;
        level_results.push(result);

        if !won {
            died_on = Some(node.name.clone());
            break;
        }

        // Between-level transition
        state.player.weapon = "Fists".into();
        state.player.weapon_damage = 0;
        state.player.armor = "None".into();
        state.player.armor_defense = 0;
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
        "turns": l.turns,
        "kills": l.kills,
        "damage_dealt": l.damage_dealt,
        "damage_taken": l.damage_taken,
        "potions_used": l.potions_used,
        "bombs_used": l.bombs_used,
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
                    "buy_speed_potions": { "type": "integer", "description": "Number of speed potions to buy from store (default 0)" }
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
        p.xp_to_next = (p.xp_to_next as f64 * 1.12) as i32;
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

            let mut snap = make_player_snapshot(level, gold, potions);
            snap.bombs = bombs;
            snap.speed_potions = speed_potions;

            let mut buy_plan: StoreBuyPlan = vec![];
            let bp = args["buy_potions"].as_i64().unwrap_or(0) as i32;
            let bb = args["buy_bombs"].as_i64().unwrap_or(0) as i32;
            let bs = args["buy_speed_potions"].as_i64().unwrap_or(0) as i32;
            if bp > 0 { buy_plan.push(("potion".into(), bp)); }
            if bb > 0 { buy_plan.push(("bomb".into(), bb)); }
            if bs > 0 { buy_plan.push(("speed_potion".into(), bs)); }

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
                let tier_scale = 1.0 + idx as f32 * 0.15;
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
                "tier_scale": 1.0 + idx as f32 * 0.15,
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
