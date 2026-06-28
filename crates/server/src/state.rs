use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use openmmo_common::{
    BossState, ContentPack, EntityId, EntityKind, GroundItem, MarketOffer, MinigameLobby, NpcFootprint, NpcId,
    NpcState, ObjectId, ObjectState, PlayerAction, PlayerId, PlayerState, QuestId, RegionDef,
    RegionId, Skill, SkillBook, TilePos, WorldEntity, INVENTORY_SIZE,
};
use openmmo_protocol::ServerMessage;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::economy::EconomyState;
use crate::minigame::MinigameState;
use crate::quest::QuestEngine;
use crate::social::SocialState;

pub struct GameWorld {
    pub content: ContentPack,
    pub tick: u64,
    pub next_entity_id: u32,
    pub players: HashMap<PlayerId, PlayerState>,
    pub npcs: HashMap<EntityId, NpcState>,
    pub objects: HashMap<EntityId, ObjectState>,
    pub ground_items: HashMap<EntityId, GroundItem>,
    pub regions: HashMap<RegionId, RegionDef>,
    pub walkable: HashSet<TilePos>,
    pub region_walkable: HashMap<RegionId, HashSet<TilePos>>,
    pub economy: EconomyState,
    pub social: SocialState,
    pub quests: QuestEngine,
    pub minigames: MinigameState,
    pub boss: Option<BossState>,
    pub tick_ms: u64,
    pub audit_log: VecDeque<String>,
}

impl GameWorld {
    pub fn new(content: ContentPack) -> Self {
        let mut world = Self {
            content,
            tick: 0,
            next_entity_id: 1,
            players: HashMap::new(),
            npcs: HashMap::new(),
            objects: HashMap::new(),
            ground_items: HashMap::new(),
            regions: HashMap::new(),
            walkable: HashSet::new(),
            region_walkable: HashMap::new(),
            economy: EconomyState::default(),
            social: SocialState::default(),
            quests: QuestEngine::default(),
            minigames: MinigameState::default(),
            boss: None,
            tick_ms: openmmo_common::TICK_MS,
            audit_log: VecDeque::new(),
        };
        world.load_regions();
        world
    }

    pub fn alloc_entity(&mut self) -> EntityId {
        let id = EntityId(self.next_entity_id);
        self.next_entity_id += 1;
        id
    }

    pub fn load_regions(&mut self) {
        self.walkable.clear();
        self.region_walkable.clear();
        for region in &self.content.regions.clone() {
            self.regions.insert(region.id, region.clone());
            let mut region_tiles = HashSet::new();
            for y in 0..region.height {
                for x in 0..region.width {
                    let idx = (y * region.width + x) as usize;
                    let tile = region.tiles.get(idx).copied().unwrap_or(1);
                    if tile != 255 {
                        let pos = TilePos::new(x as i32, y as i32);
                        region_tiles.insert(pos);
                        self.walkable.insert(pos);
                    }
                }
            }
            self.region_walkable.insert(region.id, region_tiles);
            for obj in &region.objects {
                let eid = self.alloc_entity();
                self.objects.insert(
                    eid,
                    ObjectState {
                        entity_id: eid,
                        object_id: obj.object_id,
                        position: obj.position,
                        depleted: false,
                        respawn_ticks: 0,
                        region_id: region.id,
                    },
                );
            }
            for npc_spawn in &region.npcs {
                self.spawn_npc(npc_spawn.npc_id, npc_spawn.position, region.id);
            }
        }
        if self.regions.is_empty() {
            for x in 0..50 {
                for y in 0..50 {
                    self.walkable.insert(TilePos::new(x, y));
                }
            }
        }
    }

    pub fn spawn_npc(&mut self, npc_id: NpcId, position: TilePos, region_id: RegionId) -> EntityId {
        let def = self.content.npc(npc_id).expect("npc def").clone();
        let eid = self.alloc_entity();
        self.npcs.insert(
            eid,
            NpcState {
                entity_id: eid,
                npc_id,
                name: def.name.clone(),
                position,
                home_position: position,
                hp: def.max_hp,
                max_hp: def.max_hp,
                prowess: def.prowess,
                fortitude: def.fortitude,
                aggro_target: None,
                respawn_ticks: 0,
                alive: true,
                attack_cooldown: 0,
                region_id,
            },
        );
        eid
    }

    pub fn add_player(&mut self, name: String) -> PlayerId {
        let id = PlayerId::new();
        let entity_id = self.alloc_entity();
        let (spawn, region_id) = self
            .content
            .regions
            .first()
            .map(|r| (r.spawn, r.id))
            .unwrap_or((TilePos::new(5, 5), RegionId(1)));
        let mut skills = SkillBook::new_mvp();
        for skill in Skill::BETA {
            skills.skills.insert(*skill, Default::default());
        }
        let mut player = PlayerState {
            id,
            name: name.clone(),
            entity_id,
            position: spawn,
            hp: 10,
            max_hp: 10,
            skills,
            inventory: openmmo_common::Inventory::new(INVENTORY_SIZE),
            bank: openmmo_common::Inventory::new(openmmo_common::BANK_SIZE),
            equipment: Default::default(),
            combat_target: None,
            action: PlayerAction::Idle,
            quest_progress: HashMap::new(),
            quest_counters: HashMap::new(),
            friends: Vec::new(),
            ledger_rank: 0,
            ledger_points: 0,
            specialization: HashMap::new(),
            is_moderator: false,
            last_position: spawn,
            ticks_stationary: 1,
            region_id,
        };
        player.max_hp = 10 + player.skills.level(Skill::Endurance);
        player.hp = player.max_hp;
        self.players.insert(id, player);
        self.audit(&format!("Player {name} joined"));
        id
    }

    pub fn remove_player(&mut self, id: PlayerId) {
        if let Some(p) = self.players.remove(&id) {
            self.audit(&format!("Player {} left", p.name));
        }
    }

    pub fn entities_snapshot(&self) -> Vec<WorldEntity> {
        self.entities_snapshot_for_region(None)
    }

    pub fn entities_snapshot_for_region(&self, region_id: Option<RegionId>) -> Vec<WorldEntity> {
        let mut entities = Vec::new();
        for p in self.players.values() {
            if region_id.is_some_and(|r| p.region_id != r) {
                continue;
            }
            entities.push(WorldEntity {
                entity_id: p.entity_id,
                region_id: p.region_id,
                kind: EntityKind::Player {
                    player_id: p.id,
                    name: p.name.clone(),
                    position: p.position,
                    hp: p.hp,
                    max_hp: p.max_hp,
                },
            });
        }
        for n in self.npcs.values() {
            if !n.alive || region_id.is_some_and(|r| n.region_id != r) {
                continue;
            }
            let aggro_range = self
                .content
                .npc(n.npc_id)
                .map(|d| d.aggro_range)
                .unwrap_or(0);
            entities.push(WorldEntity {
                entity_id: n.entity_id,
                region_id: n.region_id,
                kind: EntityKind::Npc {
                    npc_id: n.npc_id,
                    name: n.name.clone(),
                    position: n.position,
                    hp: n.hp,
                    max_hp: n.max_hp,
                    aggro_range,
                },
            });
        }
        if let Some(boss) = &self.boss {
            entities.push(WorldEntity {
                entity_id: boss.entity_id,
                region_id: RegionId(1),
                kind: EntityKind::Boss {
                    name: boss.name.clone(),
                    position: boss.position,
                    hp: boss.hp,
                    max_hp: boss.max_hp,
                },
            });
        }
        for o in self.objects.values() {
            if o.depleted || region_id.is_some_and(|r| o.region_id != r) {
                continue;
            }
            entities.push(WorldEntity {
                entity_id: o.entity_id,
                region_id: o.region_id,
                kind: EntityKind::Object {
                    object_id: o.object_id,
                    position: o.position,
                },
            });
        }
        for g in self.ground_items.values() {
            if region_id.is_some_and(|r| g.region_id != r) {
                continue;
            }
            entities.push(WorldEntity {
                entity_id: g.entity_id,
                region_id: g.region_id,
                kind: EntityKind::GroundItem {
                    item_id: g.item_id,
                    quantity: g.quantity,
                    position: g.position,
                },
            });
        }
        entities
    }

    pub fn try_region_transition(&mut self, player_id: PlayerId) -> Option<openmmo_protocol::ServerMessage> {
        let (region_id, position, in_combat) = {
            let player = self.players.get(&player_id)?;
            let in_combat = matches!(player.action, PlayerAction::Combat { .. })
                || player.combat_target.is_some();
            (player.region_id, player.position, in_combat)
        };
        if in_combat {
            return None;
        }
        let region = self.regions.get(&region_id)?;
        let transition = region.transitions.iter().find(|t| {
            t.position.x == position.x
                && t.position.y == position.y
                && t.position.plane == position.plane
        })?;
        if !self.regions.contains_key(&transition.target_region) {
            return None;
        }
        let target_region = transition.target_region;
        let target_spawn = transition.target_spawn;
        if let Some(player) = self.players.get_mut(&player_id) {
            player.region_id = target_region;
            player.position = target_spawn;
            player.last_position = target_spawn;
            player.action = PlayerAction::Idle;
        }
        self.audit(&format!(
            "Player {player_id:?} transitioned to region {}",
            target_region.0
        ));
        Some(ServerMessage::RegionChanged {
            region_id: target_region,
            position: target_spawn,
            entities: self.entities_snapshot_for_region(Some(target_region)),
        })
    }

    pub fn audit(&mut self, msg: &str) {
        let entry = format!("[tick {}] {msg}", self.tick);
        self.audit_log.push_back(entry.clone());
        if self.audit_log.len() > 1000 {
            self.audit_log.pop_front();
        }
        if let Ok(db_url) = std::env::var("DATABASE_URL") {
            tokio::spawn(async move {
                let _ = crate::persistence::persist_audit(&db_url, &entry).await;
            });
        }
    }

    pub fn is_walkable(&self, pos: TilePos) -> bool {
        self.walkable.contains(&pos)
    }

    pub fn is_walkable_in_region(&self, region_id: RegionId, pos: TilePos) -> bool {
        self.region_walkable
            .get(&region_id)
            .is_some_and(|tiles| tiles.contains(&pos))
    }

    /// Tiles currently occupied by living NPCs and bosses.
    pub fn entity_occupied_tiles(&self) -> HashSet<TilePos> {
        self.entity_occupied_tiles_in_region(None, None)
    }

    pub fn entity_occupied_tiles_excluding(&self, skip_entity: Option<EntityId>) -> HashSet<TilePos> {
        self.entity_occupied_tiles_in_region(None, skip_entity)
    }

    pub fn entity_occupied_tiles_in_region(
        &self,
        region_id: Option<RegionId>,
        skip_entity: Option<EntityId>,
    ) -> HashSet<TilePos> {
        let mut occupied = HashSet::new();
        for (entity_id, npc) in &self.npcs {
            if !npc.alive || skip_entity == Some(*entity_id) {
                continue;
            }
            if region_id.is_some_and(|r| npc.region_id != r) {
                continue;
            }
            let fp = self
                .content
                .npc(npc.npc_id)
                .map(NpcFootprint::from_def)
                .unwrap_or_default();
            for tile in fp.occupied_tiles(npc.position) {
                occupied.insert(tile);
            }
        }
        if let Some(boss) = &self.boss {
            if boss.hp > 0 && skip_entity != Some(boss.entity_id) {
                occupied.insert(boss.position);
            }
        }
        occupied
    }

    /// Walkable tiles for a player, excluding tiles occupied by NPCs/bosses
    /// (except the player's current tile).
    pub fn walkable_for_player(&self, player_id: PlayerId) -> HashSet<TilePos> {
        let (player_pos, region_id) = match self.players.get(&player_id) {
            Some(p) => (Some(p.position), p.region_id),
            None => return HashSet::new(),
        };
        let occupied = self.entity_occupied_tiles_in_region(Some(region_id), None);
        self.region_walkable
            .get(&region_id)
            .into_iter()
            .flatten()
            .filter(|tile| player_pos == Some(**tile) || !occupied.contains(*tile))
            .copied()
            .collect()
    }
}

pub type SharedWorld = Arc<RwLock<GameWorld>>;

pub fn world_channel() -> broadcast::Sender<String> {
    broadcast::channel(256).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmmo_common::{ContentPack, RegionDef, RegionTransition, TilePos};

    #[test]
    fn region_transition_moves_player() {
        let mut pack = ContentPack::default();
        pack.regions = vec![
            RegionDef {
                id: RegionId(1),
                name: "A".into(),
                width: 5,
                height: 5,
                spawn: TilePos::new(0, 0),
                tiles: vec![0; 25],
                objects: vec![],
                npcs: vec![],
                transitions: vec![RegionTransition {
                    position: TilePos::new(4, 2),
                    target_region: RegionId(2),
                    target_spawn: TilePos::new(1, 1),
                }],
            },
            RegionDef {
                id: RegionId(2),
                name: "B".into(),
                width: 5,
                height: 5,
                spawn: TilePos::new(0, 0),
                tiles: vec![0; 25],
                objects: vec![],
                npcs: vec![],
                transitions: vec![],
            },
        ];
        let mut world = GameWorld::new(pack);
        let pid = world.add_player("Traveler".into());
        if let Some(player) = world.players.get_mut(&pid) {
            player.position = TilePos::new(4, 2);
        }
        let msg = world.try_region_transition(pid).expect("transition");
        assert!(matches!(msg, ServerMessage::RegionChanged { .. }));
        let player = world.players.get(&pid).unwrap();
        assert_eq!(player.region_id, RegionId(2));
        assert_eq!(player.position, TilePos::new(1, 1));
    }

    #[test]
    fn region_transition_blocked_during_combat() {
        let mut pack = ContentPack::default();
        pack.regions = vec![
            RegionDef {
                id: RegionId(1),
                name: "A".into(),
                width: 5,
                height: 5,
                spawn: TilePos::new(0, 0),
                tiles: vec![0; 25],
                objects: vec![],
                npcs: vec![],
                transitions: vec![],
            },
            RegionDef {
                id: RegionId(2),
                name: "B".into(),
                width: 5,
                height: 5,
                spawn: TilePos::new(0, 0),
                tiles: vec![0; 25],
                objects: vec![],
                npcs: vec![],
                transitions: vec![RegionTransition {
                    position: TilePos::new(0, 2),
                    target_region: RegionId(1),
                    target_spawn: TilePos::new(4, 2),
                }],
            },
        ];
        let mut world = GameWorld::new(pack);
        let pid = world.add_player("Traveler".into());
        if let Some(player) = world.players.get_mut(&pid) {
            player.region_id = RegionId(2);
            player.position = TilePos::new(0, 2);
            player.action = PlayerAction::Combat {
                target: EntityId(1),
                style: openmmo_common::CombatStyle::Melee,
                player_attack_cooldown: 0,
            };
            player.combat_target = Some(EntityId(1));
        }
        assert!(world.try_region_transition(pid).is_none());
        assert_eq!(world.players.get(&pid).unwrap().region_id, RegionId(2));
    }
}
