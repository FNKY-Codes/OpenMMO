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
    let npc_ids: std::collections::HashSet<_> = pack.npcs.iter().map(|n| n.id.0).collect();
    let dialogue_ids: std::collections::HashSet<_> =
        pack.dialogues.iter().map(|d| d.id.clone()).collect();
    let shop_ids: std::collections::HashSet<_> =
        pack.shops.iter().map(|s| s.id.clone()).collect();
    let spell_ids: std::collections::HashSet<_> =
        pack.spells.iter().map(|s| s.id.clone()).collect();

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
                openmmo_common::QuestObjective::KillNpc { npc_id, .. }
                | openmmo_common::QuestObjective::TalkToNpc { npc_id } => {
                    if !npc_ids.contains(&npc_id.0) {
                        errors.push(format!(
                            "Quest {} references missing NPC {}",
                            quest.name, npc_id.0
                        ));
                    }
                }
                openmmo_common::QuestObjective::Refine { recipe_id, .. }
                    if pack.recipe(recipe_id).is_none() =>
                {
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
                    openmmo_common::DialogueAction::StartQuest { quest_id } => {
                        if pack.quest(*quest_id).is_none() {
                            errors.push(format!(
                                "Dialogue {} references missing quest {}",
                                dialogue.id, quest_id.0
                            ));
                        }
                    }
                    openmmo_common::DialogueAction::OpenShop { shop_id } => {
                        if !shop_ids.contains(shop_id) {
                            errors.push(format!(
                                "Dialogue {} references missing shop {}",
                                dialogue.id, shop_id
                            ));
                        }
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

    if errors.is_empty() {
        println!(
            "OK: {} items, {} npcs, {} regions, {} quests, {} skills",
            pack.items.len(),
            pack.npcs.len(),
            pack.regions.len(),
            pack.quests.len(),
            pack.skills.len()
        );
        Ok(())
    } else {
        for e in &errors {
            eprintln!("ERROR: {e}");
        }
        anyhow::bail!("{} validation errors", errors.len())
    }
}
