use crate::gen::TileDefRaw;
use rand::Rng;
use std::collections::{HashMap, HashSet};

pub struct Room {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub cx: i32,
    pub cy: i32,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct MapGenResult {
    pub tiles: Vec<Vec<String>>,
    pub player_start: [i32; 2],
    pub boss_position: [i32; 2],
    pub key_position: Option<[i32; 2]>,
}

struct BspNode {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    left: Option<Box<BspNode>>,
    right: Option<Box<BspNode>>,
    room: Option<Room>,
}


fn classify_tiles(tile_defs: &HashMap<String, TileDefRaw>) -> (String, String, Vec<String>) {
    let mut wall = None;
    let mut floor = None;
    let mut thematic = Vec::new();

    for (_ch, td) in tile_defs {
        if !td.walkable && wall.is_none() {
            wall = Some(td.name.clone());
        } else if td.walkable && floor.is_none() {
            floor = Some(td.name.clone());
        } else if td.walkable {
            thematic.push(td.name.clone());
        }
    }

    (
        wall.unwrap_or_else(|| "wall".into()),
        floor.unwrap_or_else(|| "floor".into()),
        thematic,
    )
}

const MIN_LEAF: i32 = 7;
const LOCKED_DOOR_TILE: &str = "locked_door";

fn bsp_split(rng: &mut impl Rng, x: i32, y: i32, w: i32, h: i32, depth: i32) -> BspNode {
    if depth <= 0 || (w < MIN_LEAF * 2 && h < MIN_LEAF * 2) {
        return BspNode { x, y, w, h, left: None, right: None, room: None };
    }

    let split_h = if w > h * 2 {
        false
    } else if h > w * 2 {
        true
    } else {
        rng.gen_bool(0.5)
    };

    if split_h {
        if h < MIN_LEAF * 2 {
            return BspNode { x, y, w, h, left: None, right: None, room: None };
        }
        let split = rng.gen_range(MIN_LEAF..=(h - MIN_LEAF));
        let left = bsp_split(rng, x, y, w, split, depth - 1);
        let right = bsp_split(rng, x, y + split, w, h - split, depth - 1);
        BspNode { x, y, w, h, left: Some(Box::new(left)), right: Some(Box::new(right)), room: None }
    } else {
        if w < MIN_LEAF * 2 {
            return BspNode { x, y, w, h, left: None, right: None, room: None };
        }
        let split = rng.gen_range(MIN_LEAF..=(w - MIN_LEAF));
        let left = bsp_split(rng, x, y, split, h, depth - 1);
        let right = bsp_split(rng, x + split, y, w - split, h, depth - 1);
        BspNode { x, y, w, h, left: Some(Box::new(left)), right: Some(Box::new(right)), room: None }
    }
}

enum RoomShape {
    Rect,
    LShape,
    Cross,
    Rounded,
    Jagged,
}

fn carve_rooms(rng: &mut impl Rng, node: &mut BspNode, grid: &mut Vec<Vec<String>>, floor_name: &str) {
    if node.left.is_none() && node.right.is_none() {
        let margin = 1;
        let max_w = node.w - margin * 2;
        let max_h = node.h - margin * 2;
        if max_w < 3 || max_h < 3 { return; }

        let rw = rng.gen_range(3..=max_w);
        let rh = rng.gen_range(3..=max_h);
        let rx = node.x + margin + rng.gen_range(0..=(max_w - rw));
        let ry = node.y + margin + rng.gen_range(0..=(max_h - rh));

        let shape = if rw >= 5 && rh >= 5 {
            match rng.gen_range(0..10) {
                0..=3 => RoomShape::Rect,
                4..=5 => RoomShape::LShape,
                6..=7 => RoomShape::Cross,
                8 => RoomShape::Rounded,
                _ => RoomShape::Jagged,
            }
        } else {
            RoomShape::Rect
        };

        match shape {
            RoomShape::Rect => {
                for y in ry..ry + rh {
                    for x in rx..rx + rw {
                        grid[y as usize][x as usize] = floor_name.to_string();
                    }
                }
            }
            RoomShape::LShape => {
                // Remove one random quadrant — asymmetric cut sizes
                let cut_w = rng.gen_range(rw / 4..=rw * 2 / 3);
                let cut_h = rng.gen_range(rh / 4..=rh * 2 / 3);
                let corner = rng.gen_range(0..4);
                for y in ry..ry + rh {
                    for x in rx..rx + rw {
                        let in_cut = match corner {
                            0 => x < rx + cut_w && y < ry + cut_h,
                            1 => x >= rx + rw - cut_w && y < ry + cut_h,
                            2 => x < rx + cut_w && y >= ry + rh - cut_h,
                            _ => x >= rx + rw - cut_w && y >= ry + rh - cut_h,
                        };
                        if !in_cut {
                            grid[y as usize][x as usize] = floor_name.to_string();
                        }
                    }
                }
            }
            RoomShape::Cross => {
                // Asymmetric cross — each arm trimmed independently
                let trim_top = rng.gen_range(1..=(rh / 3).max(1));
                let trim_bot = rng.gen_range(1..=(rh / 3).max(1));
                let trim_left = rng.gen_range(1..=(rw / 3).max(1));
                let trim_right = rng.gen_range(1..=(rw / 3).max(1));
                for y in ry..ry + rh {
                    for x in rx..rx + rw {
                        let in_h_bar = y >= ry + trim_top && y < ry + rh - trim_bot;
                        let in_v_bar = x >= rx + trim_left && x < rx + rw - trim_right;
                        if in_h_bar || in_v_bar {
                            grid[y as usize][x as usize] = floor_name.to_string();
                        }
                    }
                }
            }
            RoomShape::Rounded => {
                // Ellipse with random offset so it's not perfectly centered
                let off_x = rng.gen_range(-1.0f32..=1.0);
                let off_y = rng.gen_range(-1.0f32..=1.0);
                let cx = rx as f32 + rw as f32 / 2.0 + off_x;
                let cy = ry as f32 + rh as f32 / 2.0 + off_y;
                let rx_f = rw as f32 / 2.0;
                let ry_f = rh as f32 / 2.0;
                for y in ry..ry + rh {
                    for x in rx..rx + rw {
                        let dx = (x as f32 + 0.5 - cx) / rx_f;
                        let dy = (y as f32 + 0.5 - cy) / ry_f;
                        if dx * dx + dy * dy <= 1.0 {
                            grid[y as usize][x as usize] = floor_name.to_string();
                        }
                    }
                }
            }
            RoomShape::Jagged => {
                // Irregular walls: each row gets a random inset from left and right
                let mut left_insets: Vec<i32> = Vec::new();
                let mut right_insets: Vec<i32> = Vec::new();
                let max_inset = (rw / 4).max(1);
                for _ in 0..rh {
                    left_insets.push(rng.gen_range(0..=max_inset));
                    right_insets.push(rng.gen_range(0..=max_inset));
                }
                // Smooth slightly — average with neighbor to avoid single-tile spikes
                for i in 1..left_insets.len() - 1 {
                    left_insets[i] = (left_insets[i - 1] + left_insets[i] + left_insets[i + 1]) / 3;
                    right_insets[i] = (right_insets[i - 1] + right_insets[i] + right_insets[i + 1]) / 3;
                }
                for (row, y) in (ry..ry + rh).enumerate() {
                    let x_start = rx + left_insets[row];
                    let x_end = rx + rw - right_insets[row];
                    if x_start >= x_end { continue; }
                    for x in x_start..x_end {
                        grid[y as usize][x as usize] = floor_name.to_string();
                    }
                }
            }
        }

        node.room = Some(Room {
            x: rx, y: ry, w: rw, h: rh,
            cx: rx + rw / 2, cy: ry + rh / 2,
        });
    } else {
        if let Some(left) = &mut node.left {
            carve_rooms(rng, left, grid, floor_name);
        }
        if let Some(right) = &mut node.right {
            carve_rooms(rng, right, grid, floor_name);
        }
    }
}

fn get_room(node: &BspNode) -> Option<&Room> {
    if let Some(ref room) = node.room {
        return Some(room);
    }
    if let Some(ref left) = node.left {
        if let Some(r) = get_room(left) { return Some(r); }
    }
    if let Some(ref right) = node.right {
        if let Some(r) = get_room(right) { return Some(r); }
    }
    None
}

fn carve_corridors(rng: &mut impl Rng, node: &BspNode, grid: &mut Vec<Vec<String>>, floor_name: &str) {
    if let (Some(left), Some(right)) = (&node.left, &node.right) {
        carve_corridors(rng, left, grid, floor_name);
        carve_corridors(rng, right, grid, floor_name);

        if let (Some(lr), Some(rr)) = (get_room(left), get_room(right)) {
            carve_l_corridor(rng, lr.cx, lr.cy, rr.cx, rr.cy, grid, floor_name);
        }
    }
}

fn carve_l_corridor(
    rng: &mut impl Rng, x1: i32, y1: i32, x2: i32, y2: i32,
    grid: &mut Vec<Vec<String>>, floor_name: &str,
) {
    let w = grid[0].len() as i32;
    let h = grid.len() as i32;

    if rng.gen_bool(0.5) {
        let min_x = x1.min(x2);
        let max_x = x1.max(x2);
        for x in min_x..=max_x {
            if x >= 0 && x < w && y1 >= 0 && y1 < h {
                grid[y1 as usize][x as usize] = floor_name.to_string();
            }
        }
        let min_y = y1.min(y2);
        let max_y = y1.max(y2);
        for y in min_y..=max_y {
            if x2 >= 0 && x2 < w && y >= 0 && y < h {
                grid[y as usize][x2 as usize] = floor_name.to_string();
            }
        }
    } else {
        // Vertical then horizontal
        let min_y = y1.min(y2);
        let max_y = y1.max(y2);
        for y in min_y..=max_y {
            if x1 >= 0 && x1 < w && y >= 0 && y < h {
                grid[y as usize][x1 as usize] = floor_name.to_string();
            }
        }
        let min_x = x1.min(x2);
        let max_x = x1.max(x2);
        for x in min_x..=max_x {
            if x >= 0 && x < w && y2 >= 0 && y2 < h {
                grid[y2 as usize][x as usize] = floor_name.to_string();
            }
        }
    }
}

fn collect_rooms(node: &BspNode) -> Vec<&Room> {
    let mut rooms = Vec::new();
    if let Some(ref room) = node.room {
        rooms.push(room);
    }
    if let Some(ref left) = node.left {
        rooms.extend(collect_rooms(left));
    }
    if let Some(ref right) = node.right {
        rooms.extend(collect_rooms(right));
    }
    rooms
}

fn place_thematic_tiles(
    rng: &mut impl Rng,
    rooms: &[&Room],
    grid: &mut Vec<Vec<String>>,
    floor_name: &str,
    thematic: &[String],
) {
    if thematic.is_empty() || rooms.is_empty() { return; }

    for tile_name in thematic {
        let room_count = rng.gen_range(1..=2.min(rooms.len()));
        for _ in 0..room_count {
            let room = &rooms[rng.gen_range(0..rooms.len())];
            let pct = rng.gen_range(0.15..=0.35);
            for y in room.y..room.y + room.h {
                for x in room.x..room.x + room.w {
                    if grid[y as usize][x as usize] == floor_name && rng.gen::<f32>() < pct {
                        grid[y as usize][x as usize] = tile_name.clone();
                    }
                }
            }
        }
    }
}

/// Scan the map for a walkable tile that, when blocked, disconnects the boss from the player.
/// Prioritizes narrow corridor tiles (walls on two opposite sides).
fn find_lock_position(
    grid: &[Vec<String>],
    tile_def_map: &HashMap<String, crate::game::TileDef>,
    player_start: [i32; 2],
    boss_pos: [i32; 2],
    width: i32, height: i32,
) -> Option<(i32, i32)> {
    let mut blocked_defs = tile_def_map.clone();
    blocked_defs.insert("__blocked__".into(), crate::game::TileDef {
        name: "__blocked__".into(), color: "#000".into(), walkable: false, char_display: String::new(), damage: 0, image: None,
    });

    let is_walkable = |x: i32, y: i32| -> bool {
        if x < 0 || y < 0 || x >= width || y >= height { return false; }
        let tile = &grid[y as usize][x as usize];
        tile_def_map.get(tile).map_or(false, |t| t.walkable)
    };

    // Collect narrow corridor candidates: walkable tiles with walls on two opposite sides
    let mut candidates: Vec<(i32, i32, i32)> = Vec::new(); // (x, y, distance_from_player)
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            if !is_walkable(x, y) { continue; }
            // Skip player start and boss position
            if x == player_start[0] && y == player_start[1] { continue; }
            if x == boss_pos[0] && y == boss_pos[1] { continue; }

            // Check if it's a narrow passage: walls on opposite sides
            let h_walls = !is_walkable(x - 1, y) && !is_walkable(x + 1, y); // vertical corridor
            let v_walls = !is_walkable(x, y - 1) && !is_walkable(x, y + 1); // horizontal corridor
            // Also check for 1-wide corridors: exactly 2 walkable neighbors
            let walkable_neighbors = [(x-1,y),(x+1,y),(x,y-1),(x,y+1)].iter()
                .filter(|(nx, ny)| is_walkable(*nx, *ny)).count();

            if h_walls || v_walls || walkable_neighbors <= 2 {
                let dist = (x - player_start[0]).abs() + (y - player_start[1]).abs();
                candidates.push((x, y, dist));
            }
        }
    }

    // Sort by distance from player (prefer locks that are somewhat far but not at the boss)
    candidates.sort_by_key(|&(_, _, d)| d);

    // Test each candidate to see if blocking it cuts off the boss
    for &(cx, cy, _) in &candidates {
        let mut test_grid: Vec<Vec<String>> = grid.to_vec();
        test_grid[cy as usize][cx as usize] = "__blocked__".into();

        let reachable = flood_fill(&test_grid, &blocked_defs, player_start[0], player_start[1], width, height);
        if !reachable.contains(&(boss_pos[0], boss_pos[1])) {
            return Some((cx, cy));
        }
    }
    None
}

pub fn generate_map(tile_defs: &HashMap<String, TileDefRaw>) -> MapGenResult {
    generate_map_with_options(tile_defs, false)
}

pub fn generate_map_with_options(tile_defs: &HashMap<String, TileDefRaw>, skip_locked_door: bool) -> MapGenResult {
    let mut rng = rand::thread_rng();
    let width = 60i32;
    let height = 36i32;

    let (wall_name, floor_name, thematic) = classify_tiles(tile_defs);

    let mut grid: Vec<Vec<String>> = vec![vec![wall_name.clone(); width as usize]; height as usize];

    let depth = rng.gen_range(4..=5);
    let mut root = bsp_split(&mut rng, 0, 0, width, height, depth);

    carve_rooms(&mut rng, &mut root, &mut grid, &floor_name);
    carve_corridors(&mut rng, &root, &mut grid, &floor_name);

    let rooms = collect_rooms(&root);

    place_thematic_tiles(&mut rng, &rooms, &mut grid, &floor_name, &thematic);

    // Pick player start: room closest to top-left
    let player_room = rooms.iter()
        .min_by_key(|r| r.cx + r.cy)
        .expect("at least one room");
    let player_start = [player_room.cx, player_room.cy];

    // Pick boss position: room farthest from player
    let boss_room = rooms.iter()
        .max_by_key(|r| (r.cx - player_start[0]).abs() + (r.cy - player_start[1]).abs())
        .expect("at least one room");

    // Ensure boss 2x2 footprint fits
    let mut boss_pos = [boss_room.cx, boss_room.cy];
    'outer: for y in boss_room.y..boss_room.y + boss_room.h - 1 {
        for x in boss_room.x..boss_room.x + boss_room.w - 1 {
            let all_walkable = (0..2).all(|dy| (0..2).all(|dx| {
                let name = &grid[(y + dy) as usize][(x + dx) as usize];
                tile_defs.values().any(|td| td.name == *name && td.walkable)
            }));
            if all_walkable {
                boss_pos = [x, y];
                break 'outer;
            }
        }
    }

    // Build tile_def_map for flood fill
    let tile_def_map: HashMap<String, crate::game::TileDef> = tile_defs.values().map(|td| {
        (td.name.clone(), crate::game::TileDef {
            name: td.name.clone(),
            color: td.color.clone(),
            walkable: td.walkable,
            char_display: td.char.clone().unwrap_or_default(),
            damage: 0,
            image: td.image.clone(),
        })
    }).collect();

    // Verify connectivity
    let reachable = flood_fill(&grid, &tile_def_map, player_start[0], player_start[1], width, height);
    if !reachable.contains(&(boss_pos[0], boss_pos[1])) {
        eprintln!("WARNING: BSP map connectivity check failed, regenerating...");
        return generate_map(tile_defs);
    }

    // Place locked door at a chokepoint that separates the player from the boss
    let mut key_position = None;
    if !skip_locked_door {
    if let Some((mx, my)) = find_lock_position(
        &grid, &tile_def_map, player_start, boss_pos, width, height
    ) {
        // Place the locked door
        grid[my as usize][mx as usize] = LOCKED_DOOR_TILE.to_string();

        // Find key position: a reachable tile on the player's side (reachable without the door)
        let mut blocked_defs = tile_def_map.clone();
        blocked_defs.insert(LOCKED_DOOR_TILE.into(), crate::game::TileDef {
            name: LOCKED_DOOR_TILE.into(), color: "#000".into(), walkable: false, char_display: String::new(), damage: 0, image: None,
        });
        let player_side = flood_fill(&grid, &blocked_defs, player_start[0], player_start[1], width, height);

        // Place key on the player's side, far from start
        let player_side_tiles: Vec<(i32, i32)> = player_side.into_iter().collect();
        if !player_side_tiles.is_empty() {
            let best = player_side_tiles.iter()
                .filter(|(x, y)| {
                    (*x - player_start[0]).abs() + (*y - player_start[1]).abs() >= 4
                })
                .max_by_key(|(x, y)| (*x - player_start[0]).abs() + (*y - player_start[1]).abs());
            if let Some(&(kx, ky)) = best {
                key_position = Some([kx, ky]);
                eprintln!("Locked door at ({},{}) — key at ({},{}) — boss behind lock", mx, my, kx, ky);
            } else {
                // No good key spot, remove the door
                grid[my as usize][mx as usize] = floor_name.clone();
            }
        } else {
            // Player can't reach anything without the door — remove it
            grid[my as usize][mx as usize] = floor_name.clone();
        }
    }
    } // !skip_locked_door

    MapGenResult {
        tiles: grid,
        player_start,
        boss_position: boss_pos,
        key_position,
    }
}

fn flood_fill(
    tiles: &[Vec<String>], tile_defs: &HashMap<String, crate::game::TileDef>,
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
