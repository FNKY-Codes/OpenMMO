use serde::{Deserialize, Serialize};

use crate::{EntityId, ItemId, NpcId, PlayerId, Skill, TilePos};

pub const TICK_MS: u64 = 600;
pub const INVENTORY_SIZE: usize = 28;
pub const BANK_SIZE: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub id: PlayerId,
    pub name: String,
    pub entity_id: EntityId,
    pub position: TilePos,
    pub hp: u32,
    pub max_hp: u32,
    pub skills: crate::SkillBook,
    pub inventory: crate::Inventory,
    pub bank: crate::Inventory,
    pub equipment: Equipment,
    pub combat_target: Option<EntityId>,
    pub action: PlayerAction,
    pub quest_progress: std::collections::HashMap<crate::QuestId, u32>,
    #[serde(default)]
    pub quest_counters: std::collections::HashMap<(crate::QuestId, u32), u32>,
    pub friends: Vec<String>,
    pub ledger_rank: u32,
    pub ledger_points: u32,
    pub specialization: std::collections::HashMap<Skill, String>,
    #[serde(default)]
    pub is_moderator: bool,
    #[serde(default)]
    pub last_position: TilePos,
    #[serde(default = "default_one")]
    pub ticks_stationary: u64,
}

fn default_one() -> u64 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Equipment {
    pub head: Option<crate::InventorySlot>,
    pub body: Option<crate::InventorySlot>,
    pub legs: Option<crate::InventorySlot>,
    pub weapon: Option<crate::InventorySlot>,
    pub shield: Option<crate::InventorySlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlayerAction {
    #[default]
    Idle,
    Walking {
        path: Vec<TilePos>,
        index: usize,
        #[serde(default)]
        attack_target: Option<EntityId>,
        #[serde(default)]
        attack_style: Option<crate::CombatStyle>,
    },
    Scavenging {
        object_entity: EntityId,
        ticks_remaining: u32,
    },
    Fabricating {
        recipe_id: String,
        ticks_remaining: u32,
    },
    Combat {
        target: EntityId,
        style: crate::CombatStyle,
        #[serde(default)]
        player_attack_cooldown: u32,
    },
    Casting {
        target: EntityId,
        spell_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcState {
    pub entity_id: EntityId,
    pub npc_id: NpcId,
    pub name: String,
    pub position: TilePos,
    pub home_position: TilePos,
    pub hp: u32,
    pub max_hp: u32,
    pub prowess: u32,
    pub fortitude: u32,
    pub aggro_target: Option<PlayerId>,
    pub respawn_ticks: u32,
    #[serde(default)]
    pub attack_cooldown: u32,
    pub alive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectState {
    pub entity_id: EntityId,
    pub object_id: crate::ObjectId,
    pub position: TilePos,
    pub depleted: bool,
    pub respawn_ticks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundItem {
    pub entity_id: EntityId,
    pub item_id: ItemId,
    pub quantity: u32,
    pub position: TilePos,
    pub despawn_ticks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEntity {
    pub entity_id: EntityId,
    pub kind: EntityKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntityKind {
    Player {
        player_id: PlayerId,
        name: String,
        position: TilePos,
        hp: u32,
        max_hp: u32,
    },
    Npc {
        npc_id: NpcId,
        name: String,
        position: TilePos,
        hp: u32,
        max_hp: u32,
        #[serde(default)]
        aggro_range: i32,
    },
    Boss {
        name: String,
        position: TilePos,
        hp: u32,
        max_hp: u32,
    },
    Object {
        object_id: crate::ObjectId,
        position: TilePos,
    },
    GroundItem {
        item_id: ItemId,
        quantity: u32,
        position: TilePos,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOffer {
    pub id: uuid::Uuid,
    pub player_name: String,
    pub item_id: ItemId,
    pub quantity: u32,
    pub price_per: u32,
    pub is_buy: bool,
    pub created_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeOffer {
    pub from: PlayerId,
    pub to: PlayerId,
    pub items: Vec<crate::InventorySlot>,
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinigameLobby {
    pub id: String,
    pub players: Vec<PlayerId>,
    pub countdown: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BossState {
    pub entity_id: EntityId,
    pub name: String,
    pub position: TilePos,
    pub hp: u32,
    pub max_hp: u32,
    pub prowess: u32,
    pub fortitude: u32,
    pub phase: u32,
    pub mechanics_active: Vec<String>,
    #[serde(default = "default_boss_attack_ticks")]
    pub attack_ticks: u32,
    #[serde(default)]
    pub attack_cooldown: u32,
}

fn default_boss_attack_ticks() -> u32 {
    5
}
