# Content Authoring

Content lives in `content/` as YAML files organized by type:

- `items/`, `npcs/`, `objects/`, `recipes/`, `spells/`
- `regions/`, `quests/`, `dialogues/`, `shops/`
- `skills/`, `specializations/`

## Validation

```bash
cargo run -p openmmo-content-validator -- content
cargo run -p openmmo-region-linter -- content
```

Checks for orphan item IDs, broken quest references, missing recipe outputs, and region geometry (tile dimensions, spawn bounds).

## Third-party content packs

To ship your own MMO as a separate repository, see [Content pack layout](content-pack.md) for the `openmmo.toml` manifest and directory structure. Use [`templates/openmmo-game-template/`](../templates/openmmo-game-template/) as a starting point.

## Adding a Quest

1. Create `content/quests/my_quest.yaml`
2. Add dialogue in `content/dialogues/`
3. Run the validator
4. Save files — the server hot-reloads content automatically (`spawn_hot_reload_task` in `crates/server/src/content.rs`)

## Scavenging Tags

Scavenge nodes use `harvest_tag`: `timber`, `ore`, `water`, `flora`.
Object definitions use `scavenging_level`, `scavenging_xp`, and `scavenging_ticks`.
Recipes use `fabrication_level` and `fabrication_xp`. Spells use `electrics_level`.

Tools use `tool_tag`: `axe`, `pick`, `rod`, `knife`.
