use openmmo_common::{
    BossState, EntityId, GroundItem, ItemId, MarketOffer, NpcId, PlayerId, QuestId, Skill, TilePos,
    WorldEntity,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Client → Server messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Login {
        username: String,
        character_name: String,
    },
    WalkIntent {
        target: TilePos,
    },
    InteractObject {
        object_entity: EntityId,
    },
    Scavenge {
        object_entity: EntityId,
    },
    Refine {
        recipe_id: String,
    },
    Attack {
        target: EntityId,
        style: openmmo_common::CombatStyle,
    },
    TalkToNpc {
        npc_entity: EntityId,
    },
    CastSpell {
        target: EntityId,
        spell_id: String,
    },
    PickupItem {
        ground_entity: EntityId,
    },
    DropItem {
        slot: usize,
        quantity: u32,
    },
    BankDeposit {
        inv_slot: usize,
        quantity: u32,
    },
    BankWithdraw {
        bank_slot: usize,
        quantity: u32,
    },
    EquipItem {
        inv_slot: usize,
    },
    UnequipItem {
        slot: openmmo_common::EquipSlot,
    },
    ShopBuy {
        shop_id: String,
        item_id: ItemId,
        quantity: u32,
    },
    Chat {
        channel: ChatChannel,
        message: String,
    },
    TradeRequest {
        target_player: PlayerId,
    },
    TradeOffer {
        target: PlayerId,
        items: Vec<openmmo_common::InventorySlot>,
    },
    TradeAccept {
        target: PlayerId,
    },
    MarketPlaceOffer {
        item_id: ItemId,
        quantity: u32,
        price_per: u32,
        is_buy: bool,
    },
    MarketCancelOffer {
        offer_id: Uuid,
    },
    FriendAdd {
        name: String,
    },
    PrivateMessage {
        to: String,
        message: String,
    },
    DialogueSelect {
        npc_entity: EntityId,
        option_index: usize,
    },
    SelectSpecialization {
        skill: Skill,
        branch: String,
    },
    JoinMinigame {
        minigame_id: String,
    },
    ModeratorCommand {
        command: ModCommand,
    },
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModCommand {
    Kick {
        player: PlayerId,
    },
    Ban {
        player: PlayerId,
    },
    Teleport {
        player: PlayerId,
        target: TilePos,
    },
    SpawnItem {
        player: PlayerId,
        item_id: ItemId,
        qty: u32,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatChannel {
    Local,
    Global,
    Clan,
    Private,
}

/// Server → Client messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    LoginResult {
        success: bool,
        player_id: Option<PlayerId>,
        message: String,
    },
    WorldSnapshot {
        tick: u64,
        entities: Vec<WorldEntity>,
        local_player: Option<PlayerId>,
    },
    StateDelta {
        tick: u64,
        entities: Vec<WorldEntity>,
    },
    PlayerUpdate {
        player_id: PlayerId,
        position: TilePos,
        hp: u32,
        max_hp: u32,
        action: openmmo_common::PlayerAction,
    },
    InventoryUpdate {
        inventory: openmmo_common::Inventory,
        bank: openmmo_common::Inventory,
        equipment: openmmo_common::Equipment,
    },
    SkillUpdate {
        skills: openmmo_common::SkillBook,
        levels_gained: Vec<(Skill, u32)>,
    },
    ChatMessage {
        channel: ChatChannel,
        from: String,
        message: String,
    },
    XpDrop {
        skill: Skill,
        amount: u64,
    },
    Damage {
        source: EntityId,
        target: EntityId,
        amount: u32,
        style: openmmo_common::CombatStyle,
    },
    Death {
        entity: EntityId,
        killer: Option<EntityId>,
    },
    LootSpawn {
        items: Vec<GroundItem>,
    },
    QuestUpdate {
        quest_id: QuestId,
        stage: u32,
        completed: bool,
    },
    QuestJournal {
        entries: Vec<(String, String)>,
    },
    Dialogue {
        npc_entity: EntityId,
        node: openmmo_common::DialogueNode,
    },
    ShopOpen {
        shop_id: String,
        stock: Vec<openmmo_common::ShopStock>,
    },
    TradeUpdate {
        partner: String,
        their_items: Vec<openmmo_common::InventorySlot>,
        your_items: Vec<openmmo_common::InventorySlot>,
        partner_accepted: bool,
        you_accepted: bool,
    },
    OpenMarketUpdate {
        offers: Vec<MarketOffer>,
    },
    FriendsUpdate {
        friends: Vec<String>,
        online: Vec<String>,
    },
    MinigameStart {
        minigame_id: String,
        wave: u32,
    },
    BossUpdate {
        boss: BossState,
    },
    LedgerUpdate {
        rank: u32,
        points: u32,
        active_contract: Option<LedgerContract>,
    },
    CollectionLogEntry {
        item_id: ItemId,
        source: String,
    },
    Error {
        message: String,
    },
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerContract {
    pub npc_id: NpcId,
    pub name: String,
    pub remaining: u32,
    pub reward_points: u32,
}

pub fn encode_client(msg: &ClientMessage) -> Result<String, serde_json::Error> {
    serde_json::to_string(msg)
}

pub fn decode_client(data: &str) -> Result<ClientMessage, serde_json::Error> {
    serde_json::from_str(data)
}

pub fn encode_server(msg: &ServerMessage) -> Result<String, serde_json::Error> {
    serde_json::to_string(msg)
}

pub fn decode_server(data: &str) -> Result<ServerMessage, serde_json::Error> {
    serde_json::from_str(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_walk_intent() {
        let msg = ClientMessage::WalkIntent {
            target: TilePos::new(5, 10),
        };
        let json = encode_client(&msg).unwrap();
        let decoded = decode_client(&json).unwrap();
        assert!(matches!(decoded, ClientMessage::WalkIntent { .. }));
    }
}
