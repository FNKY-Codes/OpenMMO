use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use openmmo_common::{
    BossState, ContentPack, EntityId, EntityKind, GroundItem, MarketOffer, MinigameLobby, NpcId,
    NpcState, ObjectId, ObjectState, PlayerAction, PlayerId, PlayerState, QuestId, RegionDef,
    RegionId, Skill, SkillBook, TilePos, WorldEntity, INVENTORY_SIZE,
};
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
        for region in &self.content.regions.clone() {
            self.regions.insert(region.id, region.clone());
            for y in 0..region.height {
                for x in 0..region.width {
                    let idx = (y * region.width + x) as usize;
                    let tile = region.tiles.get(idx).copied().unwrap_or(1);
                    if tile != 255 {
                        self.walkable.insert(TilePos::new(x as i32, y as i32));
                    }
                }
            }
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
                    },
                );
            }
            for npc_spawn in &region.npcs {
                self.spawn_npc(npc_spawn.npc_id, npc_spawn.position);
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

    pub fn spawn_npc(&mut self, npc_id: NpcId, position: TilePos) -> EntityId {
        let def = self.content.npc(npc_id).expect("npc def").clone();
        let eid = self.alloc_entity();
        self.npcs.insert(
            eid,
            NpcState {
                entity_id: eid,
                npc_id,
                name: def.name.clone(),
                position,
                hp: def.max_hp,
                max_hp: def.max_hp,
                prowess: def.prowess,
                fortitude: def.fortitude,
                aggro_target: None,
                respawn_ticks: 0,
                alive: true,
            },
        );
        eid
    }

    pub fn add_player(&mut self, name: String) -> PlayerId {
        let id = PlayerId::new();
        let entity_id = self.alloc_entity();
        let spawn = self
            .content
            .regions
            .first()
            .map(|r| r.spawn)
            .unwrap_or(TilePos::new(5, 5));
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
        let mut entities = Vec::new();
        for p in self.players.values() {
            entities.push(WorldEntity {
                entity_id: p.entity_id,
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
            if n.alive {
                entities.push(WorldEntity {
                    entity_id: n.entity_id,
                    kind: EntityKind::Npc {
                        npc_id: n.npc_id,
                        name: n.name.clone(),
                        position: n.position,
                        hp: n.hp,
                        max_hp: n.max_hp,
                    },
                });
            }
        }
        for o in self.objects.values() {
            if !o.depleted {
                entities.push(WorldEntity {
                    entity_id: o.entity_id,
                    kind: EntityKind::Object {
                        object_id: o.object_id,
                        position: o.position,
                    },
                });
            }
        }
        for g in self.ground_items.values() {
            entities.push(WorldEntity {
                entity_id: g.entity_id,
                kind: EntityKind::GroundItem {
                    item_id: g.item_id,
                    quantity: g.quantity,
                    position: g.position,
                },
            });
        }
        entities
    }

    pub fn audit(&mut self, msg: &str) {
        self.audit_log
            .push_back(format!("[tick {}] {msg}", self.tick));
        if self.audit_log.len() > 1000 {
            self.audit_log.pop_front();
        }
    }

    pub fn is_walkable(&self, pos: TilePos) -> bool {
        self.walkable.contains(&pos)
    }
}

pub type SharedWorld = Arc<RwLock<GameWorld>>;

pub fn world_channel() -> broadcast::Sender<String> {
    broadcast::channel(256).0
}
