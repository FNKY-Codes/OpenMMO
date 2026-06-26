use std::path::PathBuf;

use openmmo_common::load_content;

fn main() -> anyhow::Result<()> {
    let path = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "content".to_string()),
    );
    let pack = load_content(&path)?;
    let mut errors = Vec::new();

    let item_ids: std::collections::HashSet<_> = pack.items.iter().map(|i| i.id.0).collect();
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
                openmmo_common::QuestObjective::KillNpc { npc_id, .. } => {
                    if !pack.npcs.iter().any(|n| n.id == *npc_id) {
                        errors.push(format!("Quest {} references missing NPC {}", quest.name, npc_id.0));
                    }
                }
                openmmo_common::QuestObjective::Refine { recipe_id, .. } => {
                    if pack.recipe(recipe_id).is_none() {
                        errors.push(format!("Quest {} references missing recipe {}", quest.name, recipe_id));
                    }
                }
                _ => {}
            }
        }
    }

    if errors.is_empty() {
        println!(
            "OK: {} items, {} npcs, {} regions, {} quests",
            pack.items.len(),
            pack.npcs.len(),
            pack.regions.len(),
            pack.quests.len()
        );
        Ok(())
    } else {
        for e in &errors {
            eprintln!("ERROR: {e}");
        }
        anyhow::bail!("{} validation errors", errors.len())
    }
}
