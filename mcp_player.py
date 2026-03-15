"""MCP server that plays Scapegrace for map quality testing.

Tools:
  start_game     — generate a new level, return the ASCII map
  get_state      — get current game state (map, player, monsters)
  move_player    — move in a direction (up/down/left/right)
  use_potion     — drink a health potion
  play_to_boss   — auto-play: BFS pathfind to the boss and fight it

The server connects to the Scapegrace HTTP API at localhost:3000.
Start the game server first: cd scapegrace && cargo run --release
"""

import asyncio
import json
from collections import deque

import httpx
from mcp.server import Server
from mcp.server.stdio import stdio_server
from mcp.types import TextContent, Tool

BASE = "http://localhost:3000"
server = Server("scapegrace-player")


@server.list_tools()
async def list_tools() -> list[Tool]:
    return [
        Tool(
            name="start_game",
            description="Start a new Scapegrace game. Returns the ASCII map, player position, monsters, and items.",
            inputSchema={"type": "object", "properties": {}},
        ),
        Tool(
            name="get_state",
            description="Get the current game state: ASCII map, player stats, monsters, items, log.",
            inputSchema={"type": "object", "properties": {}},
        ),
        Tool(
            name="move_player",
            description="Move the player one tile in a direction.",
            inputSchema={
                "type": "object",
                "properties": {
                    "direction": {
                        "type": "string",
                        "enum": ["up", "down", "left", "right"],
                        "description": "Direction to move",
                    },
                },
                "required": ["direction"],
            },
        ),
        Tool(
            name="use_potion",
            description="Drink a health potion to heal.",
            inputSchema={"type": "object", "properties": {}},
        ),
        Tool(
            name="play_to_boss",
            description="Auto-play: pathfind to the boss and fight it. Returns the result (victory/death/stuck). Use this to test if a map is winnable.",
            inputSchema={"type": "object", "properties": {}},
        ),
    ]


@server.call_tool()
async def call_tool(name: str, arguments: dict) -> list[TextContent]:
    async with httpx.AsyncClient(timeout=120.0) as client:
        if name == "start_game":
            resp = await client.post(f"{BASE}/api/start")
            data = resp.json()
            if "error" in data:
                return [TextContent(type="text", text=f"Error: {data['error']}")]
            return [TextContent(type="text", text=format_state(data))]

        elif name == "get_state":
            resp = await client.get(f"{BASE}/api/state")
            data = resp.json()
            if "error" in data:
                return [TextContent(type="text", text=f"Error: {data['error']}")]
            return [TextContent(type="text", text=format_state(data))]

        elif name == "move_player":
            d = arguments.get("direction", "up")
            dx, dy = {"up": (0, -1), "down": (0, 1), "left": (-1, 0), "right": (1, 0)}.get(d, (0, 0))
            resp = await client.post(f"{BASE}/api/move", json={"dx": dx, "dy": dy})
            data = resp.json()
            if "error" in data:
                return [TextContent(type="text", text=f"Error: {data['error']}")]
            return [TextContent(type="text", text=format_state(data))]

        elif name == "use_potion":
            resp = await client.post(f"{BASE}/api/potion")
            data = resp.json()
            return [TextContent(type="text", text=format_state(data))]

        elif name == "play_to_boss":
            return [TextContent(type="text", text=await auto_play(client))]

    return [TextContent(type="text", text=f"Unknown tool: {name}")]


def format_state(data: dict) -> str:
    parts = []
    parts.append(f"=== {data.get('title', '?')} ===")
    parts.append(data.get("map", ""))
    p = data.get("player", {})
    parts.append(f"Player: ({p.get('x')},{p.get('y')}) HP:{p.get('hp')}/{p.get('max_hp')} ATK:{p.get('attack')} DEF:{p.get('defense')}")
    parts.append(f"Weapon: {p.get('weapon')}  Armor: {p.get('armor')}  Potions: {p.get('potions')}  Gold: {p.get('gold')}")

    monsters = data.get("monsters", [])
    if monsters:
        parts.append(f"Monsters ({len(monsters)}):")
        for m in monsters:
            boss = " [BOSS]" if m.get("is_boss") else ""
            parts.append(f"  {m['name']}{boss} at ({m['x']},{m['y']}) HP:{m['hp']}/{m['max_hp']}")

    items = data.get("items", [])
    if items:
        parts.append(f"Items ({len(items)}):")
        for it in items:
            parts.append(f"  {it['name']} ({it['type']}) at ({it['x']},{it['y']})")

    log = data.get("log", [])
    if log:
        parts.append(f"Log (last 5):")
        for entry in log[-5:]:
            parts.append(f"  {entry}")

    if data.get("game_over"):
        parts.append("*** GAME OVER ***")
    if data.get("victory"):
        parts.append("*** VICTORY ***")

    return "\n".join(parts)


def parse_map(map_str: str) -> list[list[str]]:
    """Parse ASCII map string into 2D grid."""
    return [list(line) for line in map_str.strip().split("\n") if line]


def bfs_path(grid: list[list[str]], start: tuple[int, int], target: tuple[int, int]) -> list[tuple[int, int]] | None:
    """BFS pathfind from start to target. Walkable = not '#'."""
    if not grid:
        return None
    h = len(grid)
    w = len(grid[0]) if grid else 0
    visited = set()
    queue = deque([(start, [start])])
    visited.add(start)

    while queue:
        (x, y), path = queue.popleft()
        if (x, y) == target:
            return path
        for dx, dy in [(0, 1), (0, -1), (1, 0), (-1, 0)]:
            nx, ny = x + dx, y + dy
            if 0 <= nx < w and 0 <= ny < h and (nx, ny) not in visited:
                ch = grid[ny][nx]
                # Can walk through anything except walls
                if ch != '#':
                    visited.add((nx, ny))
                    queue.append(((nx, ny), path + [(nx, ny)]))
    return None


async def walk_to(client: httpx.AsyncClient, target: tuple[int, int], results: list[str], max_steps: int = 200) -> dict | None:
    """Walk toward target, re-pathfinding after combat. Returns final state, or None if died."""
    steps = 0
    while steps < max_steps:
        resp = await client.get(f"{BASE}/api/state")
        data = resp.json()
        p = data.get("player", {})
        cur = (p.get("x", 0), p.get("y", 0))

        if cur == target:
            return data

        # Use potion if low
        if p.get("hp", 0) < p.get("max_hp", 30) * 0.3 and p.get("potions", 0) > 0:
            resp = await client.post(f"{BASE}/api/potion")
            data = resp.json()
            results.append(f"  Healed at HP {p['hp']}")

        grid = parse_map(data.get("map", ""))
        path = bfs_path(grid, cur, target)
        if not path or len(path) < 2:
            # Try adjacent tiles
            path = find_path_to(grid, cur, target)
            if not path or len(path) < 2:
                return data  # Can't reach, return current state

        # Take one step
        nx, ny = path[1]
        cx, cy = path[0]
        resp = await client.post(f"{BASE}/api/move", json={"dx": nx - cx, "dy": ny - cy})
        data = resp.json()
        steps += 1

        if data.get("game_over"):
            return None
        if data.get("victory"):
            return data

    return data


def find_path_to(grid, start: tuple[int, int], target: tuple[int, int]) -> list[tuple[int, int]] | None:
    """Find path to target or adjacent to target."""
    # Try the target itself
    p = bfs_path(grid, start, target)
    if p:
        return p
    # Try adjacent tiles
    best = None
    for dx, dy in [(0, -1), (0, 1), (-1, 0), (1, 0)]:
        t = (target[0] + dx, target[1] + dy)
        p = bfs_path(grid, start, t)
        if p and (best is None or len(p) < len(best)):
            best = p
    return best


async def auto_play(client: httpx.AsyncClient) -> str:
    """Auto-play: gear up, then fight the boss."""
    resp = await client.post(f"{BASE}/api/start")
    data = resp.json()
    if "error" in data:
        return f"Failed to start: {data['error']}"

    results = []
    results.append(f"Level: {data.get('title', '?')}")

    grid = parse_map(data.get("map", ""))
    player = data.get("player", {})
    px, py = player.get("x", 0), player.get("y", 0)

    # Find boss
    boss = None
    for m in data.get("monsters", []):
        if m.get("is_boss"):
            boss = m
            break
    if not boss:
        results.append("ERROR: No boss on the map!")
        return "\n".join(results)

    results.append(f"Player at ({px},{py}), Boss '{boss['name']}' at ({boss['x']},{boss['y']})")

    # Plan: collect weapon, armor, and potions before fighting boss
    # Prioritize: weapon > armor > potions > gold > boss
    items = data.get("items", [])
    weapon = next((it for it in items if it["type"] == "weapon"), None)
    armor = next((it for it in items if it["type"] == "armor"), None)
    potions = [it for it in items if it["type"] == "potion"]

    targets = []
    if weapon:
        targets.append(("weapon", weapon["name"], (weapon["x"], weapon["y"])))
    if armor:
        targets.append(("armor", armor["name"], (armor["x"], armor["y"])))
    for pot in potions:
        targets.append(("potion", pot["name"], (pot["x"], pot["y"])))

    results.append(f"Gear plan: {len(targets)} items to collect before boss")

    # Collect items in order
    cur = (px, py)
    for item_type, item_name, item_pos in targets:
        path = find_path_to(grid, cur, item_pos)
        if not path:
            results.append(f"  Can't reach {item_name} at {item_pos}, skipping")
            continue

        results.append(f"  Walking to {item_name} ({item_type}) at {item_pos}")
        data = await walk_to(client, item_pos, results)
        if data is None:
            results.append(f"DIED collecting {item_name}!")
            return "\n".join(results)
        if data.get("victory"):
            results.append("VICTORY (killed boss while collecting items!)")
            return "\n".join(results)

        p = data.get("player", {})
        cur = (p.get("x", 0), p.get("y", 0))
        results.append(f"  Stats: HP {p.get('hp')}/{p.get('max_hp')} ATK:{p.get('attack')} DEF:{p.get('defense')} Weapon:{p.get('weapon')} Armor:{p.get('armor')}")

    # Now fight the boss
    results.append(f"Geared up! Heading to boss...")

    # Re-read state for current positions
    resp = await client.get(f"{BASE}/api/state")
    data = resp.json()
    p = data.get("player", {})
    cur = (p.get("x", 0), p.get("y", 0))

    # Find boss position (may have moved if regular monster killed it... unlikely)
    bx, by = boss["x"], boss["y"]
    for m in data.get("monsters", []):
        if m.get("is_boss"):
            bx, by = m["x"], m["y"]
            break

    grid = parse_map(data.get("map", ""))
    path = find_path_to(grid, cur, (bx, by))
    if not path:
        results.append(f"PATHFINDING FAILED: Can't reach boss at ({bx},{by})")
        results.append(data.get("map", ""))
        return "\n".join(results)

    results.append(f"Heading to boss...")
    data = await walk_to(client, (bx, by), results)
    if data is None:
        results.append("DIED on the way to boss!")
        return "\n".join(results)
    if data.get("victory"):
        p = data.get("player", {})
        results.append(f"VICTORY! HP {p.get('hp')}/{p.get('max_hp')}, Level {p.get('level')}, Gold {p.get('gold')}")
        return "\n".join(results)

    # We're adjacent to boss — fight!
    results.append("Engaging boss...")
    fight_rounds = 0
    max_rounds = 60
    while fight_rounds < max_rounds:
        p = data.get("player", {})
        cur_x, cur_y = p.get("x", 0), p.get("y", 0)

        if p.get("hp", 0) < p.get("max_hp", 30) * 0.35 and p.get("potions", 0) > 0:
            resp = await client.post(f"{BASE}/api/potion")
            data = resp.json()
            results.append(f"  Potion at HP {p['hp']} -> {data.get('player',{}).get('hp')}")

        # Find boss
        boss_alive = False
        for m in data.get("monsters", []):
            if m.get("is_boss"):
                boss_alive = True
                bx, by = m["x"], m["y"]
                break
        if not boss_alive:
            break

        dx = max(-1, min(1, bx - cur_x))
        dy = max(-1, min(1, by - cur_y))
        if abs(bx - cur_x) >= abs(by - cur_y):
            resp = await client.post(f"{BASE}/api/move", json={"dx": dx, "dy": 0})
        else:
            resp = await client.post(f"{BASE}/api/move", json={"dx": 0, "dy": dy})

        data = resp.json()
        fight_rounds += 1

        if data.get("victory"):
            p = data.get("player", {})
            results.append(f"VICTORY after {fight_rounds} rounds! HP {p.get('hp')}/{p.get('max_hp')}, Level {p.get('level')}, Gold {p.get('gold')}")
            return "\n".join(results)

        if data.get("game_over"):
            p = data.get("player", {})
            results.append(f"DIED fighting boss after {fight_rounds} rounds. HP {p.get('hp')}/{p.get('max_hp')}")
            return "\n".join(results)

    results.append(f"TIMEOUT after {max_rounds} rounds")
    return "\n".join(results)


async def main():
    async with stdio_server() as (read_stream, write_stream):
        await server.run(read_stream, write_stream, server.create_initialization_options())


if __name__ == "__main__":
    asyncio.run(main())
