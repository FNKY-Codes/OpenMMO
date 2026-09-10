use std::collections::HashSet;

use crate::{ContentPack, DialogueAction, QuestObjective, RegionDef, TilePos};

/// Cross-reference validation: checks that items, NPCs, quests, dialogues, shops,
/// and regions reference existing definitions.
pub fn validate_content(pack: &ContentPack) -> Vec<String> {
    let mut errors = Vec::new();

    let item_ids: HashSet<_> = pack.items.iter().map(|i| i.id.0).collect();
    let npc_ids: HashSet<_> = pack.npcs.iter().map(|n| n.id.0).collect();
    let dialogue_ids: HashSet<_> = pack.dialogues.iter().map(|d| d.id.clone()).collect();
    let shop_ids: HashSet<_> = pack.shops.iter().map(|s| s.id.clone()).collect();
    let spell_ids: HashSet<_> = pack.spells.iter().map(|s| s.id.clone()).collect();

    for npc in &pack.npcs {
        for loot in &npc.loot_table {
            if !item_ids.contains(&loot.item_id.0) {
                errors.push(format!(
                    "NPC {} references missing item {}",
                    npc.name, loot.item_id.0
                ));
            }
        }
    }
    for recipe in &pack.recipes {
        for input in &recipe.inputs {
            if !item_ids.contains(&input.item_id.0) {
                errors.push(format!(
                    "Recipe {} references missing item {}",
                    recipe.id, input.item_id.0
                ));
            }
        }
        if !item_ids.contains(&recipe.output.0) {
            errors.push(format!(
                "Recipe {} output item {} missing",
                recipe.id, recipe.output.0
            ));
        }
    }
    for quest in &pack.quests {
        for stage in &quest.stages {
            match &stage.objective {
                QuestObjective::KillNpc { npc_id, .. } | QuestObjective::TalkToNpc { npc_id } => {
                    if !npc_ids.contains(&npc_id.0) {
                        errors.push(format!(
                            "Quest {} references missing NPC {}",
                            quest.name, npc_id.0
                        ));
                    }
                }
                QuestObjective::Refine { recipe_id, .. } if pack.recipe(recipe_id).is_none() => {
                    errors.push(format!(
                        "Quest {} references missing recipe {}",
                        quest.name, recipe_id
                    ));
                }
                _ => {}
            }
        }
    }
    for dialogue in &pack.dialogues {
        for opt in &dialogue.options {
            if let Some(next) = &opt.next {
                if !dialogue_ids.contains(next) {
                    errors.push(format!(
                        "Dialogue {} references missing node {}",
                        dialogue.id, next
                    ));
                }
            }
            if let Some(action) = &opt.action {
                match action {
                    DialogueAction::StartQuest { quest_id } => {
                        if pack.quest(*quest_id).is_none() {
                            errors.push(format!(
                                "Dialogue {} references missing quest {}",
                                dialogue.id, quest_id.0
                            ));
                        }
                    }
                    DialogueAction::OpenShop { shop_id } if !shop_ids.contains(shop_id) => {
                        errors.push(format!(
                            "Dialogue {} references missing shop {}",
                            dialogue.id, shop_id
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
    for shop in &pack.shops {
        for stock in &shop.stock {
            if !item_ids.contains(&stock.item_id.0) {
                errors.push(format!(
                    "Shop {} references missing item {}",
                    shop.name, stock.item_id.0
                ));
            }
        }
    }
    for region in &pack.regions {
        for npc in &region.npcs {
            if !npc_ids.contains(&npc.npc_id.0) {
                errors.push(format!(
                    "Region {} references missing NPC {}",
                    region.name, npc.npc_id.0
                ));
            }
        }
        for obj in &region.objects {
            if pack.object(obj.object_id).is_none() {
                errors.push(format!(
                    "Region {} references missing object {}",
                    region.name, obj.object_id.0
                ));
            }
        }
    }
    for spell in &pack.spells {
        if !spell_ids.contains(&spell.id) {
            errors.push(format!("Spell {} missing from index", spell.id));
        }
    }

    errors
}

/// Region geometry linting: tile dimensions, spawn bounds, entity placement.
pub fn lint_regions(pack: &ContentPack) -> Vec<String> {
    let mut errors = Vec::new();
    let mut region_ids = HashSet::new();

    for region in &pack.regions {
        errors.extend(lint_region(region));

        if !region_ids.insert(region.id.0) {
            errors.push(format!(
                "Duplicate region id {} ({})",
                region.id.0, region.name
            ));
        }
    }

    let known_regions: HashSet<u32> = pack.regions.iter().map(|r| r.id.0).collect();
    for region in &pack.regions {
        for transition in &region.transitions {
            if !in_bounds(&transition.position, region) {
                errors.push(format!(
                    "Region {} transition at {:?} is out of bounds",
                    region.name, transition.position
                ));
            }
            if !known_regions.contains(&transition.target_region.0) {
                errors.push(format!(
                    "Region {} transition targets unknown region id {}",
                    region.name, transition.target_region.0
                ));
            }
            if let Some(target) = pack.region(transition.target_region) {
                if !in_bounds(&transition.target_spawn, target) {
                    errors.push(format!(
                        "Region {} transition spawn {:?} is out of bounds in target region {}",
                        region.name, transition.target_spawn, target.name
                    ));
                } else if !target.is_walkable(transition.target_spawn) {
                    errors.push(format!(
                        "Region {} transition spawn {:?} is on a blocked tile in target region {}",
                        region.name, transition.target_spawn, target.name
                    ));
                }
            }
        }
    }

    for region in &pack.regions {
        errors.extend(lint_reachability(region));
    }

    // Every station a recipe needs must exist somewhere in the world.
    let placed_stations: HashSet<crate::StationTag> = pack
        .regions
        .iter()
        .flat_map(|r| r.objects.iter())
        .filter_map(|o| pack.object(o.object_id).and_then(|d| d.station))
        .collect();
    for recipe in &pack.recipes {
        if let Some(station) = recipe.station {
            if !placed_stations.contains(&station) {
                errors.push(format!(
                    "Recipe {} needs a {:?} station but none is placed in any region",
                    recipe.id, station
                ));
            }
        }
    }

    errors
}

/// Flood-fill from the spawn over walkable tiles (8-directional, matching
/// movement) and report anything a player could never reach: NPC spawns,
/// transitions, and objects with no adjacent reachable tile.
fn lint_reachability(region: &RegionDef) -> Vec<String> {
    let mut errors = Vec::new();
    if region.width == 0 || region.height == 0 || !region.is_walkable(region.spawn) {
        return errors;
    }
    let w = region.width as i32;
    let h = region.height as i32;
    let mut seen = vec![false; (w * h) as usize];
    let idx = |p: TilePos| (p.y * w + p.x) as usize;
    let mut stack = vec![region.spawn];
    seen[idx(region.spawn)] = true;
    while let Some(p) = stack.pop() {
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let n = TilePos::new(p.x + dx, p.y + dy);
                if n.x < 0 || n.y < 0 || n.x >= w || n.y >= h {
                    continue;
                }
                if !seen[idx(n)] && region.is_walkable(n) {
                    seen[idx(n)] = true;
                    stack.push(n);
                }
            }
        }
    }
    let reachable = |p: TilePos| p.x >= 0 && p.y >= 0 && p.x < w && p.y < h && seen[idx(p)];
    let adjacent_reachable = |p: TilePos| {
        (-1..=1).any(|dy| (-1..=1).any(|dx| reachable(TilePos::new(p.x + dx, p.y + dy))))
    };

    for npc in &region.npcs {
        if in_bounds(&npc.position, region) && !reachable(npc.position) {
            errors.push(format!(
                "Region {} NPC {} at ({},{}) is unreachable from spawn",
                region.name, npc.npc_id.0, npc.position.x, npc.position.y
            ));
        }
    }
    for obj in &region.objects {
        if in_bounds(&obj.position, region) && !adjacent_reachable(obj.position) {
            errors.push(format!(
                "Region {} object {} at ({},{}) has no reachable neighbour",
                region.name, obj.object_id.0, obj.position.x, obj.position.y
            ));
        }
    }
    for t in &region.transitions {
        if in_bounds(&t.position, region) && !reachable(t.position) {
            errors.push(format!(
                "Region {} transition at ({},{}) is unreachable from spawn",
                region.name, t.position.x, t.position.y
            ));
        }
    }
    errors
}

fn lint_region(region: &RegionDef) -> Vec<String> {
    let mut errors = Vec::new();
    let expected_tiles = (region.width as usize).saturating_mul(region.height as usize);

    if region.width == 0 || region.height == 0 {
        errors.push(format!(
            "Region {} has zero width or height ({}x{})",
            region.name, region.width, region.height
        ));
    }

    if region.tiles.len() != expected_tiles {
        errors.push(format!(
            "Region {} tile count mismatch: got {}, expected {} ({}x{})",
            region.name,
            region.tiles.len(),
            expected_tiles,
            region.width,
            region.height
        ));
    }

    if !in_bounds(&region.spawn, region) {
        errors.push(format!(
            "Region {} spawn {:?} is out of bounds ({}x{})",
            region.name, region.spawn, region.width, region.height
        ));
    } else if !region.is_walkable(region.spawn) {
        errors.push(format!(
            "Region {} spawn {:?} is not on a walkable tile",
            region.name, region.spawn
        ));
    }

    if let Some((idx, byte)) = region
        .tiles
        .iter()
        .enumerate()
        .find(|(_, b)| crate::TileKind::from_byte(**b).is_none())
    {
        errors.push(format!(
            "Region {} has unknown tile byte {} at index {}",
            region.name, byte, idx
        ));
    }

    for npc in &region.npcs {
        if !in_bounds(&npc.position, region) {
            errors.push(format!(
                "Region {} NPC {} at {:?} is out of bounds",
                region.name, npc.npc_id.0, npc.position
            ));
        } else if !region.is_walkable(npc.position) {
            errors.push(format!(
                "Region {} NPC {} at {:?} is on a blocked tile",
                region.name, npc.npc_id.0, npc.position
            ));
        }
    }

    for obj in &region.objects {
        if !in_bounds(&obj.position, region) {
            errors.push(format!(
                "Region {} object {} at {:?} is out of bounds",
                region.name, obj.object_id.0, obj.position
            ));
        }
    }

    for t in &region.transitions {
        if in_bounds(&t.position, region) && !region.is_walkable(t.position) {
            errors.push(format!(
                "Region {} transition at {:?} is on a blocked tile (players can't reach it)",
                region.name, t.position
            ));
        }
    }

    if !region.layout.is_empty() {
        errors.push(format!(
            "Region {} still has an unexpanded layout (call expand_layout)",
            region.name
        ));
    }

    errors
}

fn in_bounds(pos: &TilePos, region: &RegionDef) -> bool {
    pos.x >= 0 && pos.y >= 0 && (pos.x as u32) < region.width && (pos.y as u32) < region.height
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NpcId, RegionId, RegionNpc, TilePos};

    #[test]
    fn lint_catches_tile_count_mismatch() {
        let region = RegionDef {
            id: RegionId(1),
            name: "Test".into(),
            width: 2,
            height: 2,
            spawn: TilePos::new(0, 0),
            tiles: vec![0],
            objects: vec![],
            npcs: vec![],
            transitions: vec![],
            ..Default::default()
        };
        let errors = lint_region(&region);
        assert!(errors.iter().any(|e| e.contains("tile count mismatch")));
    }

    #[test]
    fn lint_catches_out_of_bounds_spawn() {
        let region = RegionDef {
            id: RegionId(1),
            name: "Test".into(),
            width: 2,
            height: 2,
            spawn: TilePos::new(5, 5),
            tiles: vec![0; 4],
            objects: vec![],
            npcs: vec![],
            transitions: vec![],
            ..Default::default()
        };
        let errors = lint_region(&region);
        assert!(errors.iter().any(|e| e.contains("spawn")));
    }

    #[test]
    fn lint_catches_out_of_bounds_npc() {
        let region = RegionDef {
            id: RegionId(1),
            name: "Test".into(),
            width: 10,
            height: 10,
            spawn: TilePos::new(0, 0),
            tiles: vec![0; 100],
            objects: vec![],
            npcs: vec![RegionNpc {
                npc_id: NpcId(1),
                position: TilePos::new(10, 0),
            }],
            transitions: vec![],
            ..Default::default()
        };
        let errors = lint_region(&region);
        assert!(errors.iter().any(|e| e.contains("NPC")));
    }
}
