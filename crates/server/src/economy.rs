use std::collections::HashMap;

use openmmo_common::{InventorySlot, ItemId, MarketOffer, PlayerId};
use openmmo_protocol::ServerMessage;
use uuid::Uuid;

use crate::state::GameWorld;
use crate::tick::inventory_update;

#[derive(Debug, Clone)]
pub struct TradeSession {
    pub player_a: PlayerId,
    pub player_b: PlayerId,
    pub items_a: Vec<InventorySlot>,
    pub items_b: Vec<InventorySlot>,
    pub accepted_a: bool,
    pub accepted_b: bool,
}

#[derive(Debug, Clone)]
pub struct LedgerContractState {
    pub npc_id: openmmo_common::NpcId,
    pub name: String,
    pub remaining: u32,
    pub reward_points: u32,
}

#[derive(Default)]
pub struct EconomyState {
    pub market_offers: Vec<MarketOffer>,
    pub trades: HashMap<PlayerId, TradeSession>,
    pub ledger_contracts: HashMap<PlayerId, LedgerContractState>,
    pub price_history: HashMap<u32, Vec<u32>>,
}

fn find_trade_session<'a>(
    trades: &'a mut HashMap<PlayerId, TradeSession>,
    a: PlayerId,
    b: PlayerId,
) -> Option<&'a mut TradeSession> {
    trades.values_mut().find(|s| {
        (s.player_a == a && s.player_b == b) || (s.player_a == b && s.player_b == a)
    })
}

#[allow(dead_code)]
fn _find_trade_session_alias<'a>(
    trades: &'a mut HashMap<PlayerId, TradeSession>,
    a: PlayerId,
    b: PlayerId,
) -> Option<&'a mut TradeSession> {
    find_trade_session(trades, a, b)
}

pub fn handle_market_offer(
    world: &mut GameWorld,
    player_id: PlayerId,
    item_id: ItemId,
    quantity: u32,
    price_per: u32,
    is_buy: bool,
) -> Vec<ServerMessage> {
    let name = world
        .players
        .get(&player_id)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    if !is_buy {
        let player = match world.players.get_mut(&player_id) {
            Some(p) => p,
            None => return Vec::new(),
        };
        let mut remaining = quantity;
        for slot in player.inventory.slots.iter_mut() {
            if let Some(s) = slot {
                if s.item_id == item_id && remaining > 0 {
                    let take = remaining.min(s.quantity);
                    s.quantity -= take;
                    remaining -= take;
                    if s.quantity == 0 {
                        *slot = None;
                    }
                }
            }
        }
        if remaining > 0 {
            return vec![ServerMessage::Error {
                message: "Not enough items for offer".into(),
            }];
        }
    }
    let offer = MarketOffer {
        id: Uuid::new_v4(),
        player_name: name,
        item_id,
        quantity,
        price_per,
        is_buy,
        created_tick: world.tick,
    };
    world.economy.market_offers.push(offer);
    world.audit(&format!("Open Market offer placed by {player_id:?}"));
    vec![open_market_update(world)]
}

pub fn handle_market_cancel(
    world: &mut GameWorld,
    player_id: PlayerId,
    offer_id: Uuid,
) -> Vec<ServerMessage> {
    let name = world
        .players
        .get(&player_id)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    if let Some(offer) = world
        .economy
        .market_offers
        .iter()
        .find(|o| o.id == offer_id && o.player_name == name)
        .cloned()
    {
        if !offer.is_buy {
            if let Some(player) = world.players.get_mut(&player_id) {
                let stackable = world
                    .content
                    .item(offer.item_id)
                    .map(|i| i.stackable)
                    .unwrap_or(true);
                let _ = player
                    .inventory
                    .add_item(offer.item_id, offer.quantity, stackable);
            }
        }
    }
    world
        .economy
        .market_offers
        .retain(|o| o.id != offer_id || o.player_name != name);
    vec![open_market_update(world)]
}

pub fn handle_trade_request(
    world: &mut GameWorld,
    from: PlayerId,
    to: PlayerId,
) -> Vec<ServerMessage> {
    world.economy.trades.insert(
        from,
        TradeSession {
            player_a: from,
            player_b: to,
            items_a: Vec::new(),
            items_b: Vec::new(),
            accepted_a: false,
            accepted_b: false,
        },
    );
    let partner = world
        .players
        .get(&to)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    vec![ServerMessage::TradeUpdate {
        partner,
        their_items: Vec::new(),
        your_items: Vec::new(),
        partner_accepted: false,
        you_accepted: false,
    }]
}

pub fn handle_trade_offer(
    world: &mut GameWorld,
    from: PlayerId,
    to: PlayerId,
    items: Vec<InventorySlot>,
) -> Vec<ServerMessage> {
    let initiator = world
        .economy
        .trades
        .iter()
        .find(|(_, s)| {
            (s.player_a == from && s.player_b == to) || (s.player_a == to && s.player_b == from)
        })
        .map(|(k, _)| *k);
    let Some(initiator) = initiator else {
        return Vec::new();
    };
    let session = match world.economy.trades.get_mut(&initiator) {
        Some(s) => s,
        None => return Vec::new(),
    };
    if from == session.player_a {
        session.items_a = items;
        session.accepted_a = false;
    } else if from == session.player_b {
        session.items_b = items;
        session.accepted_b = false;
    } else {
        return Vec::new();
    }
    let partner = world
        .players
        .get(&to)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let (your_items, their_items, you_accepted, partner_accepted) = if from == session.player_a {
        (
            session.items_a.clone(),
            session.items_b.clone(),
            session.accepted_a,
            session.accepted_b,
        )
    } else {
        (
            session.items_b.clone(),
            session.items_a.clone(),
            session.accepted_b,
            session.accepted_a,
        )
    };
    vec![ServerMessage::TradeUpdate {
        partner,
        their_items,
        your_items,
        partner_accepted,
        you_accepted,
    }]
}

pub fn handle_trade_accept(
    world: &mut GameWorld,
    from: PlayerId,
    to: PlayerId,
) -> Vec<ServerMessage> {
    let initiator = world
        .economy
        .trades
        .iter()
        .find(|(_, s)| {
            (s.player_a == from && s.player_b == to) || (s.player_a == to && s.player_b == from)
        })
        .map(|(k, _)| *k);
    let Some(initiator) = initiator else {
        return Vec::new();
    };
    let session = match world.economy.trades.get_mut(&initiator) {
        Some(s) => s,
        None => return Vec::new(),
    };
    if from == session.player_a {
        session.accepted_a = true;
    } else if from == session.player_b {
        session.accepted_b = true;
    } else {
        return Vec::new();
    }
    if session.accepted_a && session.accepted_b {
        return execute_trade(world, initiator);
    }
    let partner = world
        .players
        .get(&to)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let (your_items, their_items, you_accepted, partner_accepted) = if from == session.player_a {
        (
            session.items_a.clone(),
            session.items_b.clone(),
            session.accepted_a,
            session.accepted_b,
        )
    } else {
        (
            session.items_b.clone(),
            session.items_a.clone(),
            session.accepted_b,
            session.accepted_a,
        )
    };
    vec![ServerMessage::TradeUpdate {
        partner,
        their_items,
        your_items,
        partner_accepted,
        you_accepted,
    }]
}

fn execute_trade(world: &mut GameWorld, initiator: PlayerId) -> Vec<ServerMessage> {
    let session = match world.economy.trades.remove(&initiator) {
        Some(s) => s,
        None => return Vec::new(),
    };
    let items_a = session.items_a.clone();
    let items_b = session.items_b.clone();
    let player_a = session.player_a;
    let player_b = session.player_b;
    let mut out = Vec::new();
    if let Some(player_b_ref) = world.players.get_mut(&player_b) {
        for item in items_a {
            let stackable = world
                .content
                .item(item.item_id)
                .map(|i| i.stackable)
                .unwrap_or(true);
            let _ = player_b_ref
                .inventory
                .add_item(item.item_id, item.quantity, stackable);
        }
    }
    if let Some(player_a_ref) = world.players.get_mut(&player_a) {
        for item in items_b {
            let stackable = world
                .content
                .item(item.item_id)
                .map(|i| i.stackable)
                .unwrap_or(true);
            let _ = player_a_ref
                .inventory
                .add_item(item.item_id, item.quantity, stackable);
        }
        out.push(inventory_update(player_a_ref));
    }
    if let Some(player_b_ref) = world.players.get(&player_b) {
        out.push(inventory_update(player_b_ref));
    }
    world.audit(&format!(
        "Trade executed between {:?} and {:?}",
        player_a, player_b
    ));
    out
}

pub fn match_offers(world: &mut GameWorld) {
    let mut i = 0;
    while i < world.economy.market_offers.len() {
        let buy_idx = world.economy.market_offers.iter().position(|o| o.is_buy);
        let sell_idx = world.economy.market_offers.iter().position(|o| !o.is_buy);
        if let (Some(bi), Some(si)) = (buy_idx, sell_idx) {
            let buy = world.economy.market_offers[bi].clone();
            let sell = world.economy.market_offers[si].clone();
            if buy.item_id == sell.item_id && buy.price_per >= sell.price_per {
                let qty = buy.quantity.min(sell.quantity);
                let total = sell.price_per.saturating_mul(qty);
                if let Some(buyer) = world
                    .players
                    .values_mut()
                    .find(|p| p.name == buy.player_name)
                {
                    let stackable = world
                        .content
                        .item(buy.item_id)
                        .map(|i| i.stackable)
                        .unwrap_or(true);
                    let _ = buyer.inventory.add_item(buy.item_id, qty, stackable);
                    let mut remaining = total;
                    for slot in buyer.inventory.slots.iter_mut() {
                        if let Some(s) = slot {
                            if s.item_id.0 == 1 && remaining > 0 {
                                let take = remaining.min(s.quantity);
                                s.quantity -= take;
                                remaining -= take;
                                if s.quantity == 0 {
                                    *slot = None;
                                }
                            }
                        }
                    }
                }
                if let Some(seller) = world
                    .players
                    .values_mut()
                    .find(|p| p.name == sell.player_name)
                {
                    let stackable = world
                        .content
                        .item(ItemId(1))
                        .map(|i| i.stackable)
                        .unwrap_or(true);
                    let _ = seller.inventory.add_item(ItemId(1), total, stackable);
                }
                world
                    .economy
                    .price_history
                    .entry(buy.item_id.0)
                    .or_default()
                    .push(sell.price_per);
                world.economy.market_offers[bi].quantity -= qty;
                world.economy.market_offers[si].quantity -= qty;
                let mut sell_idx = si;
                if world.economy.market_offers[bi].quantity == 0 {
                    world.economy.market_offers.remove(bi);
                    if sell_idx > bi {
                        sell_idx -= 1;
                    }
                }
                if sell_idx < world.economy.market_offers.len()
                    && world.economy.market_offers[sell_idx].quantity == 0
                {
                    world.economy.market_offers.remove(sell_idx);
                }
                world.audit(&format!(
                    "Open Market matched {} x item {}",
                    qty, buy.item_id.0
                ));
                continue;
            }
        }
        i += 1;
        if i > 100 {
            break;
        }
    }
}

pub fn open_market_update(world: &GameWorld) -> ServerMessage {
    ServerMessage::OpenMarketUpdate {
        offers: world.economy.market_offers.clone(),
    }
}

pub fn assign_ledger_contract(world: &mut GameWorld, player_id: PlayerId) {
    if world.economy.ledger_contracts.contains_key(&player_id) {
        return;
    }
    let npc = world.content.npcs.first().cloned();
    if let Some(npc) = npc {
        world.economy.ledger_contracts.insert(
            player_id,
            LedgerContractState {
                npc_id: npc.id,
                name: format!("Hunt {}", npc.name),
                remaining: 5,
                reward_points: 10,
            },
        );
    }
}

pub fn complete_ledger_kill(
    world: &mut GameWorld,
    player_id: PlayerId,
    npc_id: openmmo_common::NpcId,
) {
    if let Some(contract) = world.economy.ledger_contracts.get_mut(&player_id) {
        if contract.npc_id == npc_id && contract.remaining > 0 {
            contract.remaining -= 1;
            if contract.remaining == 0 {
                if let Some(player) = world.players.get_mut(&player_id) {
                    player.ledger_points += contract.reward_points;
                    if player.ledger_points >= 50 {
                        player.ledger_rank += 1;
                        player.ledger_points = 0;
                    }
                }
                world.economy.ledger_contracts.remove(&player_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmmo_common::{ContentPack, Inventory, ItemId, PlayerState, RegionId, SkillBook, TilePos};

    use uuid::Uuid;

    fn test_player(id: u128, name: &str) -> PlayerState {
        let pid = PlayerId(Uuid::from_u128(id));
        PlayerState {
            id: pid,
            name: name.to_string(),
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
            quest_progress: Default::default(),
            quest_counters: Default::default(),
            friends: Vec::new(),
            ledger_rank: 0,
            ledger_points: 0,
            specialization: Default::default(),
            is_moderator: false,
            last_position: TilePos::new(0, 0),
            ticks_stationary: 1,
            region_id: RegionId(1),
        }
    }

    #[test]
    fn bilateral_trade_accept_executes_swap() {
        let mut world = GameWorld::new(ContentPack::default());
        let a = PlayerId(Uuid::from_u128(1));
        let b = PlayerId(Uuid::from_u128(2));
        world.players.insert(a, test_player(1, "Alice"));
        world.players.insert(b, test_player(2, "Bob"));
        world.economy.trades.insert(
            a,
            TradeSession {
                player_a: a,
                player_b: b,
                items_a: vec![openmmo_common::InventorySlot {
                    item_id: ItemId(2),
                    quantity: 3,
                }],
                items_b: vec![openmmo_common::InventorySlot {
                    item_id: ItemId(3),
                    quantity: 1,
                }],
                accepted_a: false,
                accepted_b: false,
            },
        );
        let _ = handle_trade_accept(&mut world, a, b);
        let msgs = handle_trade_accept(&mut world, b, a);
        assert!(!msgs.is_empty());
        assert!(world.economy.trades.is_empty());
        assert_eq!(
            world.players.get(&b).unwrap().inventory.slots[0]
                .as_ref()
                .map(|s| s.item_id),
            Some(ItemId(2))
        );
    }

    #[test]
    fn match_offers_pairs_buy_and_sell_at_compatible_prices() {
        let mut world = GameWorld::new(ContentPack::default());
        let buyer_id = PlayerId(Uuid::from_u128(1));
        let seller_id = PlayerId(Uuid::from_u128(2));
        let mut buyer_state = test_player(1, "buyer");
        buyer_state.inventory.slots[0] = Some(openmmo_common::InventorySlot {
            item_id: ItemId(1),
            quantity: 100,
        });
        world.players.insert(buyer_id, buyer_state);
        world.players.insert(seller_id, test_player(2, "seller"));

        world.economy.market_offers.push(openmmo_common::MarketOffer {
            id: Uuid::new_v4(),
            player_name: "buyer".into(),
            item_id: ItemId(2),
            quantity: 5,
            price_per: 20,
            is_buy: true,
            created_tick: 0,
        });
        world.economy.market_offers.push(openmmo_common::MarketOffer {
            id: Uuid::new_v4(),
            player_name: "seller".into(),
            item_id: ItemId(2),
            quantity: 5,
            price_per: 15,
            is_buy: false,
            created_tick: 0,
        });

        match_offers(&mut world);
        assert!(world.economy.market_offers.is_empty());
        let buyer = world.players.get(&buyer_id).unwrap();
        assert!(
            buyer
                .inventory
                .slots
                .iter()
                .flatten()
                .any(|s| s.item_id == ItemId(2))
        );
    }

    #[test]
    fn match_offers_skips_when_buy_price_too_low() {
        let mut world = GameWorld::new(ContentPack::default());
        world.players.insert(PlayerId(Uuid::from_u128(1)), test_player(1, "buyer"));
        world.players.insert(PlayerId(Uuid::from_u128(2)), test_player(2, "seller"));
        world.economy.market_offers.push(openmmo_common::MarketOffer {
            id: Uuid::new_v4(),
            player_name: "buyer".into(),
            item_id: ItemId(2),
            quantity: 1,
            price_per: 5,
            is_buy: true,
            created_tick: 0,
        });
        world.economy.market_offers.push(openmmo_common::MarketOffer {
            id: Uuid::new_v4(),
            player_name: "seller".into(),
            item_id: ItemId(2),
            quantity: 1,
            price_per: 10,
            is_buy: false,
            created_tick: 0,
        });
        match_offers(&mut world);
        assert_eq!(world.economy.market_offers.len(), 2);
    }

    #[test]
    fn sell_offer_debits_inventory_and_match_transfers_to_buyer() {
        let mut world = GameWorld::new(ContentPack::default());
        let buyer_id = PlayerId(Uuid::from_u128(1));
        let seller_id = PlayerId(Uuid::from_u128(2));
        let mut buyer = test_player(1, "buyer");
        buyer.inventory.slots[0] = Some(openmmo_common::InventorySlot {
            item_id: ItemId(1),
            quantity: 100,
        });
        let mut seller = test_player(2, "seller");
        seller.inventory.slots[0] = Some(openmmo_common::InventorySlot {
            item_id: ItemId(2),
            quantity: 10,
        });
        world.players.insert(buyer_id, buyer);
        world.players.insert(seller_id, seller);

        let msgs = handle_market_offer(&mut world, seller_id, ItemId(2), 5, 15, false);
        assert!(!msgs.is_empty());
        let seller_inv_qty: u32 = world.players.get(&seller_id).unwrap().inventory.slots[0]
            .as_ref()
            .map(|s| s.quantity)
            .unwrap_or(0);
        assert_eq!(seller_inv_qty, 5);

        let _ = handle_market_offer(&mut world, buyer_id, ItemId(2), 5, 20, true);
        match_offers(&mut world);

        assert!(world.economy.market_offers.is_empty());
        let buyer = world.players.get(&buyer_id).unwrap();
        assert!(
            buyer
                .inventory
                .slots
                .iter()
                .flatten()
                .any(|s| s.item_id == ItemId(2) && s.quantity == 5)
        );
    }
}
