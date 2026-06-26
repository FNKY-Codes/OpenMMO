use std::collections::HashMap;

use openmmo_common::{EntityId, HarvestTag, PlayerId, QuestId};
use openmmo_protocol::ServerMessage;

use crate::economy::complete_ledger_kill;
use crate::state::GameWorld;

#[derive(Default)]
pub struct QuestEngine {
    pub dialogue_index: HashMap<String, openmmo_common::DialogueNode>,
}

impl QuestEngine {
    pub fn load(&mut self, content: &openmmo_common::ContentPack) {
        for d in &content.dialogues {
            self.dialogue_index.insert(d.id.clone(), d.clone());
        }
    }
}

pub fn handle_dialogue_select(
    world: &mut GameWorld,
    player_id: PlayerId,
    npc_entity: EntityId,
    option_index: usize,
) -> Vec<ServerMessage> {
    let npc_id = world.npcs.get(&npc_entity).map(|n| n.npc_id);
    let Some(npc_id) = npc_id else {
        return Vec::new();
    };
    let dialogue_id = format!("npc_{}", npc_id.0);
    let node = world.quests.dialogue_index.get(&dialogue_id).cloned();
    if let Some(node) = node {
        if let Some(opt) = node.options.get(option_index) {
            if let Some(openmmo_common::DialogueAction::StartQuest { quest_id }) = &opt.action {
                if let Some(player) = world.players.get_mut(&player_id) {
                    player.quest_progress.insert(*quest_id, 1);
                }
                return vec![ServerMessage::QuestUpdate {
                    quest_id: *quest_id,
                    stage: 1,
                    completed: false,
                }];
            }
            if let Some(next) = &opt.next {
                if let Some(next_node) = world.quests.dialogue_index.get(next) {
                    return vec![ServerMessage::Dialogue {
                        npc_entity,
                        node: next_node.clone(),
                    }];
                }
            }
        }
    }
    Vec::new()
}

pub fn on_harvest(world: &mut GameWorld, player_id: PlayerId, tag: Option<HarvestTag>) {
    let Some(tag) = tag else { return };
    advance_quests(world, player_id, |obj| {
        if let openmmo_common::QuestObjective::Harvest { tag: t, count } = obj {
            *t == tag
        } else {
            false
        }
    });
}

pub fn on_refine(world: &mut GameWorld, player_id: PlayerId, recipe_id: &str) {
    let rid = recipe_id.to_string();
    advance_quests(world, player_id, |obj| {
        if let openmmo_common::QuestObjective::Refine { recipe_id, .. } = obj {
            recipe_id == &rid
        } else {
            false
        }
    });
}

pub fn on_kill(world: &mut GameWorld, player_id: PlayerId, npc_id: openmmo_common::NpcId) {
    complete_ledger_kill(world, player_id, npc_id);
    advance_quests(world, player_id, |obj| {
        if let openmmo_common::QuestObjective::KillNpc { npc_id: id, .. } = obj {
            *id == npc_id
        } else {
            false
        }
    });
}

fn advance_quests(
    world: &mut GameWorld,
    player_id: PlayerId,
    predicate: impl Fn(&openmmo_common::QuestObjective) -> bool,
) {
    let quests: Vec<QuestId> = world.content.quests.iter().map(|q| q.id).collect();
    for qid in quests {
        let stage = world
            .players
            .get(&player_id)
            .and_then(|p| p.quest_progress.get(&qid).copied())
            .unwrap_or(0);
        if stage == 0 {
            continue;
        }
        if let Some(quest) = world.content.quest(qid) {
            if let Some(quest_stage) = quest.stages.iter().find(|s| s.stage == stage) {
                if predicate(&quest_stage.objective) {
                    if let Some(player) = world.players.get_mut(&player_id) {
                        let next = stage + 1;
                        let completed = quest.stages.iter().all(|s| s.stage < next);
                        player.quest_progress.insert(qid, next);
                        let _ = completed;
                    }
                }
            }
        }
    }
}

pub fn quest_journal(world: &GameWorld, player_id: PlayerId) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let progress = world
        .players
        .get(&player_id)
        .map(|p| p.quest_progress.clone())
        .unwrap_or_default();
    for quest in &world.content.quests {
        if let Some(stage) = progress.get(&quest.id) {
            if *stage > 0 {
                let desc = quest
                    .stages
                    .iter()
                    .find(|s| s.stage == *stage)
                    .map(|s| s.description.clone())
                    .unwrap_or_else(|| quest.description.clone());
                entries.push((quest.name.clone(), desc));
            }
        }
    }
    entries
}
