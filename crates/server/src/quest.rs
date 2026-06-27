use std::collections::HashMap;

use openmmo_common::{EntityId, HarvestTag, NpcId, PlayerId, QuestId, QuestObjective, Skill, TilePos};
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

pub fn handle_talk_to_npc(
    world: &mut GameWorld,
    player_id: PlayerId,
    npc_entity: EntityId,
) -> Vec<ServerMessage> {
    let npc_id = world.npcs.get(&npc_entity).map(|n| n.npc_id);
    let Some(npc_id) = npc_id else {
        return Vec::new();
    };
    let def = world.content.npc(npc_id);
    if def.is_some_and(|d| d.aggro_range > 0) {
        return Vec::new();
    }
    let dialogue_id = format!("npc_{}", npc_id.0);
    let node = world.quests.dialogue_index.get(&dialogue_id).cloned();
    if let Some(node) = node {
        let mut out = on_talk_to_npc(world, player_id, npc_id);
        out.push(ServerMessage::Dialogue {
            npc_entity,
            node,
        });
        return out;
    }
    on_talk_to_npc(world, player_id, npc_id)
}

pub fn handle_dialogue_select(
    world: &mut GameWorld,
    player_id: PlayerId,
    npc_entity: EntityId,
    dialogue_id: &str,
    option_index: usize,
) -> Vec<ServerMessage> {
    let npc_id = world.npcs.get(&npc_entity).map(|n| n.npc_id);
    let Some(npc_id) = npc_id else {
        return Vec::new();
    };
    let dialogue_id = if dialogue_id.is_empty() {
        format!("npc_{}", npc_id.0)
    } else {
        dialogue_id.to_string()
    };
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
            if let Some(openmmo_common::DialogueAction::OpenShop { shop_id }) = &opt.action {
                if let Some(shop) = world.content.shops.iter().find(|s| s.id == *shop_id) {
                    return vec![ServerMessage::ShopOpen {
                        shop_id: shop_id.clone(),
                        stock: shop.stock.clone(),
                    }];
                }
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

pub fn on_harvest(world: &mut GameWorld, player_id: PlayerId, tag: Option<HarvestTag>) -> Vec<ServerMessage> {
    let Some(tag) = tag else {
        return Vec::new();
    };
    advance_quests(world, player_id, |obj| {
        matches!(obj, QuestObjective::Harvest { tag: t, .. } if *t == tag)
    })
}

pub fn on_refine(world: &mut GameWorld, player_id: PlayerId, recipe_id: &str) -> Vec<ServerMessage> {
    let rid = recipe_id.to_string();
    advance_quests(world, player_id, |obj| {
        matches!(obj, QuestObjective::Refine { recipe_id, .. } if recipe_id == &rid)
    })
}

pub fn on_kill(world: &mut GameWorld, player_id: PlayerId, npc_id: NpcId) -> Vec<ServerMessage> {
    complete_ledger_kill(world, player_id, npc_id);
    advance_quests(world, player_id, |obj| {
        matches!(obj, QuestObjective::KillNpc { npc_id: id, .. } if *id == npc_id)
    })
}

pub fn on_talk_to_npc(world: &mut GameWorld, player_id: PlayerId, npc_id: NpcId) -> Vec<ServerMessage> {
    advance_quests(world, player_id, |obj| {
        matches!(obj, QuestObjective::TalkToNpc { npc_id: id } if *id == npc_id)
    })
}

pub fn on_skill_level(world: &mut GameWorld, player_id: PlayerId, skill: Skill, level: u32) -> Vec<ServerMessage> {
    let mut out = Vec::new();
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
                if let QuestObjective::ReachSkillLevel {
                    skill: req_skill,
                    level: req_level,
                } = quest_stage.objective
                {
                    if req_skill == skill && level >= req_level {
                        if let Some(msg) = complete_stage(world, player_id, qid, stage) {
                            out.push(msg);
                        }
                    }
                }
            }
        }
    }
    out
}

pub fn on_visit_tile(world: &mut GameWorld, player_id: PlayerId, position: TilePos) -> Vec<ServerMessage> {
    let mut out = Vec::new();
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
                if let QuestObjective::VisitTile {
                    position: target,
                    radius,
                } = quest_stage.objective
                {
                    if position.chebyshev_distance(&target) <= radius {
                        if let Some(msg) = complete_stage(world, player_id, qid, stage) {
                            out.push(msg);
                        }
                    }
                }
            }
        }
    }
    out
}

fn advance_quests(
    world: &mut GameWorld,
    player_id: PlayerId,
    predicate: impl Fn(&QuestObjective) -> bool,
) -> Vec<ServerMessage> {
    let mut out = Vec::new();
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
                    let required = objective_count(&quest_stage.objective);
                    if required <= 1 {
                        if let Some(msg) = complete_stage(world, player_id, qid, stage) {
                            out.push(msg);
                        }
                    } else if let Some(msg) =
                        increment_counter(world, player_id, qid, stage, required)
                    {
                        out.push(msg);
                    }
                }
            }
        }
    }
    out
}

fn objective_count(obj: &QuestObjective) -> u32 {
    match obj {
        QuestObjective::Harvest { count, .. }
        | QuestObjective::Refine { count, .. }
        | QuestObjective::KillNpc { count, .. } => *count,
        _ => 1,
    }
}

fn increment_counter(
    world: &mut GameWorld,
    player_id: PlayerId,
    qid: QuestId,
    stage: u32,
    required: u32,
) -> Option<ServerMessage> {
    let Some(player) = world.players.get_mut(&player_id) else {
        return None;
    };
    let key = (qid, stage);
    let count = player.quest_counters.entry(key).or_insert(0);
    *count += 1;
    if *count >= required {
        player.quest_counters.remove(&key);
        drop(player);
        complete_stage(world, player_id, qid, stage)
    } else {
        None
    }
}

fn complete_stage(
    world: &mut GameWorld,
    player_id: PlayerId,
    qid: QuestId,
    stage: u32,
) -> Option<ServerMessage> {
    let Some(quest) = world.content.quest(qid).cloned() else {
        return None;
    };
    let Some(player) = world.players.get_mut(&player_id) else {
        return None;
    };
    let next = stage + 1;
    let completed = !quest.stages.iter().any(|s| s.stage == next);
    player.quest_progress.insert(qid, next);
    Some(ServerMessage::QuestUpdate {
        quest_id: qid,
        stage: next,
        completed,
    })
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

pub fn quest_update_messages(world: &GameWorld, player_id: PlayerId) -> Vec<ServerMessage> {
    let progress = world
        .players
        .get(&player_id)
        .map(|p| p.quest_progress.clone())
        .unwrap_or_default();
    progress
        .into_iter()
        .filter_map(|(qid, stage)| {
            let completed = world
                .content
                .quest(qid)
                .is_some_and(|q| !q.stages.iter().any(|s| s.stage == stage));
            Some(ServerMessage::QuestUpdate {
                quest_id: qid,
                stage,
                completed,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use openmmo_common::{
        ContentPack, EntityId, HarvestTag, Inventory, NpcId, NpcState, PlayerState, QuestId,
        Skill, SkillBook, TilePos,
    };
    use uuid::Uuid;

    use crate::state::GameWorld;

    fn load_test_content() -> ContentPack {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        openmmo_common::load_content(&path).expect("content dir")
    }

    fn test_player(id: u128, quest_progress: HashMap<QuestId, u32>) -> PlayerState {
        let pid = PlayerId(Uuid::from_u128(id));
        PlayerState {
            id: pid,
            name: "Tester".into(),
            entity_id: openmmo_common::EntityId(id as u32),
            position: TilePos::new(0, 0),
            hp: 10,
            max_hp: 10,
            skills: SkillBook::new_mvp(),
            inventory: Inventory::new(28),
            bank: Inventory::new(200),
            equipment: Default::default(),
            combat_target: None,
            action: Default::default(),
            quest_progress,
            quest_counters: Default::default(),
            friends: Vec::new(),
            ledger_rank: 0,
            ledger_points: 0,
            specialization: Default::default(),
            is_moderator: false,
            last_position: TilePos::new(0, 0),
            ticks_stationary: 1,
        }
    }

    fn world_with_player(id: u128, quest_progress: HashMap<QuestId, u32>) -> (GameWorld, PlayerId) {
        let content = load_test_content();
        let mut world = GameWorld::new(content.clone());
        world.quests.load(&content);
        let pid = PlayerId(Uuid::from_u128(id));
        world.players.insert(pid, test_player(id, quest_progress));
        (world, pid)
    }

    #[test]
    fn talk_to_guide_advances_first_steps_stage_one() {
        let (mut world, pid) = world_with_player(1, HashMap::from([(QuestId(1), 1)]));
        let msgs = on_talk_to_npc(&mut world, pid, NpcId(100));
        assert!(!msgs.is_empty());
        assert_eq!(
            world.players.get(&pid).unwrap().quest_progress.get(&QuestId(1)),
            Some(&2)
        );
    }

    #[test]
    fn harvest_advances_first_steps_stage_two() {
        let (mut world, pid) = world_with_player(1, HashMap::from([(QuestId(1), 2)]));
        let msgs = on_harvest(&mut world, pid, Some(HarvestTag::Timber));
        assert!(!msgs.is_empty());
        assert_eq!(
            world.players.get(&pid).unwrap().quest_progress.get(&QuestId(1)),
            Some(&3)
        );
    }

    #[test]
    fn skill_level_advances_reach_skill_objective() {
        let (mut world, pid) = world_with_player(1, HashMap::from([(QuestId(1), 5)]));
        let msgs = on_skill_level(&mut world, pid, Skill::Scavenging, 5);
        assert!(!msgs.is_empty());
        assert!(
            world
                .players
                .get(&pid)
                .unwrap()
                .quest_progress
                .get(&QuestId(1))
                .is_some_and(|s| *s > 5)
        );
    }

    #[test]
    fn quest_journal_lists_active_quests() {
        let (world, pid) = world_with_player(1, HashMap::from([(QuestId(1), 2)]));
        let journal = quest_journal(&world, pid);
        assert!(!journal.is_empty());
        assert!(journal.iter().any(|(name, _)| name.contains("First Steps")));
    }

    #[test]
    fn dialogue_select_opens_shop_from_nested_node() {
        let (mut world, pid) = world_with_player(1, HashMap::new());
        let entity_id = EntityId(100);
        world.npcs.insert(
            entity_id,
            NpcState {
                entity_id,
                npc_id: NpcId(100),
                name: "Guide".into(),
                position: TilePos::new(0, 0),
                home_position: TilePos::new(0, 0),
                hp: 10,
                max_hp: 10,
                prowess: 0,
                fortitude: 1,
                aggro_target: None,
                respawn_ticks: 0,
                alive: true,
                attack_cooldown: 0,
            },
        );
        let msgs = handle_dialogue_select(&mut world, pid, entity_id, "npc_100_callings", 0);
        assert!(
            msgs.iter().any(|m| matches!(
                m,
                ServerMessage::ShopOpen { shop_id, .. } if shop_id == "starter_supplies"
            ))
        );
    }
}
