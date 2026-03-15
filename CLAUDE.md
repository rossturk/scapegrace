# Scapegrace

LLM-generated roguelike. Rust backend, browser client.

## Architecture Principle

The LLM is the brain. The engine is the body. The body has no autonomic systems.

**LLM decides ALL creative output:**
- Map layout (tile grid, rooms, corridors, tile types, colors)
- Boss placement, stats, name, sprite — including WHERE it goes
- Monster templates (name, sprite, stats)
- Weapon/armor design (name, sprite)
- Theme, font, title, description

**Engine handles ONLY mechanical operations:**
- Validating maps (flood fill connectivity check)
- Spawning regular monsters/items on random reachable tiles from LLM templates
- Combat math (attack rolls, damage, HP, XP, leveling)
- Movement, collision, fog of war
- Victory detection (boss killed) and death (HP <= 0)

## Critical Rules

1. The engine MUST NOT make creative decisions. No placing bosses, no choosing positions for named entities, no deciding what goes where. That's the LLM's job.

2. The engine MUST NOT repair or patch LLM-generated maps. If validation fails, retry generation. The LLM fixes its own work.

3. The engine CAN place generic/mechanical things: random gold piles, potion drops, unnamed monster spawns from templates. These are mechanical, not creative.

4. If the LLM consistently fails at something (e.g. connected maps), fix it by improving the prompt, not by adding engine workarounds.
