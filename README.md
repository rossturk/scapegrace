# Scapegrace

A roguelike where the LLM is the dungeon master.

Every campaign is unique — the AI designs the overworld, names the levels, picks the themes, places the bosses, chooses the musical mode for each dungeon, and draws the maps. The engine just runs the physics.

## How it works

1. You provide an [OpenRouter](https://openrouter.ai/) API key (the game calls it a "passphrase")
2. The LLM designs a branching campaign overworld with 5-8 levels
3. You navigate the map Super Mario World-style, choosing your path
4. Each level is generated in two LLM calls: one for objects + tile definitions, one for the map
5. Beat the final boss to win

## What the LLM decides

- Campaign structure, level names, descriptions, themes
- Tile definitions and colors
- Boss stats, name, sprite, and placement
- Monster templates, weapons, armor, traps
- Musical mode for each level (all sound effects play in-key)
- Google Font for each level's title

## What the engine does

- Validates maps (flood fill connectivity)
- Spawns monsters/items on random reachable tiles from LLM templates
- Combat math, movement, collision
- Raycasting line-of-sight fog of war
- Analog synth sound effects (no samples)

## Running

```
cargo run --release
```

Requires an [OpenRouter API key](https://openrouter.ai/keys). You can either:
- Set `OPENROUTER_API_KEY` in your environment or `.env` file
- Enter it at the in-game passphrase screen

Optionally set `ALLMUDDY_MODEL` to override the LLM model (defaults to `anthropic/claude-sonnet-4`).

## Controls

**Overworld:** Arrow keys / WASD to navigate, Enter to play a level

**In-game:** WASD to move, bump into enemies to attack, P for potions

## Built with

- [macroquad](https://github.com/not-fl3/macroquad) — game framework
- [rodio](https://github.com/RustAudio/rodio) — audio (analog synth, no samples)
- [OpenRouter](https://openrouter.ai/) — LLM API
