use std::collections::HashMap;

use openmmo_common::{InventorySlot, ItemId, MarketOffer, PlayerId};
use openmmo_protocol::ServerMessage;
use uuid::Uuid;

use crate::state::GameWorld;

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
    Vec::new()
}

pub fn handle_trade_offer(
    world: &mut GameWorld,
    from: PlayerId,
    to: PlayerId,
    items: Vec<InventorySlot>,
) -> Vec<ServerMessage> {
    if let Some(session) = world.economy.trades.get_mut(&from) {
        session.items_a = items;
        session.accepted_a = false;
        let partner = world
            .players
            .get(&to)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        return vec![ServerMessage::TradeUpdate {
            partner,
            their_items: session.items_b.clone(),
            your_items: session.items_a.clone(),
            partner_accepted: session.accepted_b,
            you_accepted: session.accepted_a,
        }];
    }
    Vec::new()
}

pub fn handle_trade_accept(
    world: &mut GameWorld,
    from: PlayerId,
    to: PlayerId,
) -> Vec<ServerMessage> {
    let session = match world.economy.trades.get_mut(&from) {
        Some(s) => s,
        None => return Vec::new(),
    };
    if session.player_b != to {
        return Vec::new();
    }
    session.accepted_a = true;
    if session.accepted_a && session.accepted_b {
        execute_trade(world, from);
        return Vec::new();
    }
    Vec::new()
}

fn execute_trade(world: &mut GameWorld, from: PlayerId) {
    let session = match world.economy.trades.remove(&from) {
        Some(s) => s,
        None => return,
    };
    world.audit(&format!(
        "Trade executed between {:?} and {:?}",
        session.player_a, session.player_b
    ));
    let _ = session;
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
                world
                    .economy
                    .price_history
                    .entry(buy.item_id.0)
                    .or_default()
                    .push(sell.price_per);
                world.economy.market_offers[bi].quantity -= qty;
                world.economy.market_offers[si].quantity -= qty;
                if world.economy.market_offers[bi].quantity == 0 {
                    world.economy.market_offers.remove(bi);
                }
                if si < world.economy.market_offers.len()
                    && world.economy.market_offers[si].quantity == 0
                {
                    world.economy.market_offers.remove(si);
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
