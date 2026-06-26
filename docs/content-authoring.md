# Content Authoring

Content lives in `content/` as YAML files organized by type:

- `items/`, `npcs/`, `objects/`, `recipes/`, `spells/`
- `regions/`, `quests/`, `dialogues/`, `shops/`
- `skills/`, `specializations/`

## Validation

```bash
cargo run -p openmmo-content-validator -- content
```

Checks for orphan item IDs, broken quest references, and missing recipe outputs.

## Adding a Quest

1. Create `content/quests/my_quest.yaml`
2. Add dialogue in `content/dialogues/`
3. Run the validator
4. Restart the server (content hot-reload planned)

## Scavenging Tags

Scavenge nodes use `harvest_tag`: `timber`, `ore`, `water`, `flora`.
Object definitions use `scavenging_level`, `scavenging_xp`, and `scavenging_ticks`.
Recipes use `fabrication_level` and `fabrication_xp`. Spells use `electrics_level`.

Tools use `tool_tag`: `axe`, `pick`, `rod`, `knife`.
