use openmmo_common::{EntityId, EquipSlot, HarvestTag, PlayerAction, PlayerId, Skill, TilePos, ToolTag};
use openmmo_protocol::{ChatChannel, ClientMessage, LedgerContract, ServerMessage};
use rand::Rng;

use crate::combat::{cast_spell, npc_attack_player, player_attack_npc};
use crate::pathfinding::find_path;
use crate::state::GameWorld;

#[derive(Debug, Clone, Copy)]
pub enum MessageTarget {
    Player(PlayerId),
    Broadcast,
}

pub fn process_tick(world: &mut GameWorld) -> Vec<(MessageTarget, ServerMessage)> {
    world.tick += 1;
    let mut messages = Vec::new();

    crate::anticheat::PluginApi::on_tick(world);

    let mut moved_players = Vec::new();
    let occupied = world.entity_occupied_tiles();
    for player in world.players.values_mut() {
        if let PlayerAction::Walking { path, index } = &mut player.action {
            if *index + 1 < path.len() {
                let next_pos = path[*index + 1];
                if occupied.contains(&next_pos) && next_pos != player.position {
                    player.action = PlayerAction::Idle;
                } else {
                    *index += 1;
                    player.position = path[*index];
                    player.last_position = player.position;
                    player.ticks_stationary = 1;
                    moved_players.push((player.id, player.position));
                }
            } else {
                player.action = PlayerAction::Idle;
            }
        } else {
            player.ticks_stationary = player.ticks_stationary.saturating_add(1);
        }
    }
    for (pid, pos) in moved_players {
        for msg in crate::quest::on_visit_tile(world, pid, pos) {
            messages.push((MessageTarget::Player(pid), msg));
        }
    }

    tick_npc_ai(world, &mut messages);

    tick_scavenge(world, &mut messages);
    tick_refine(world, &mut messages);
    tick_combat(world, &mut messages);
    tick_npc_respawn(world);
    tick_object_respawn(world);
    tick_ground_items(world);
    tick_minigame(world, &mut messages);
    tick_boss(world, &mut messages);
    tick_economy(world);
    tick_ledger_contracts(world, &mut messages);

    messages
}

pub fn handle_client_message(
    world: &mut GameWorld,
    player_id: PlayerId,
    msg: ClientMessage,
) -> Vec<(MessageTarget, ServerMessage)> {
    let mut out = Vec::new();
    let route = |msg: ServerMessage| (MessageTarget::Player(player_id), msg);
    let broadcast = |msg: ServerMessage| (MessageTarget::Broadcast, msg);
    match msg {
        ClientMessage::Login { .. } => {}
        ClientMessage::WalkIntent { target } => {
            if let Some(resp) = handle_walk(world, player_id, target) {
                out.push(route(resp));
            }
        }
        ClientMessage::Scavenge { object_entity } => {
            out.extend(
                handle_scavenge(world, player_id, object_entity)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::InteractObject { object_entity } => {
            out.extend(
                handle_scavenge(world, player_id, object_entity)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::Refine { recipe_id } => {
            out.extend(
                handle_refine(world, player_id, &recipe_id)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::Attack { target, style } => {
            out.extend(
                handle_attack(world, player_id, target, style)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::TalkToNpc { npc_entity } => {
            out.extend(
                crate::quest::handle_talk_to_npc(world, player_id, npc_entity)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::CastSpell { target, spell_id } => {
            out.extend(
                handle_spell(world, player_id, target, &spell_id)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::PickupItem { ground_entity } => {
            out.extend(
                handle_pickup(world, player_id, ground_entity)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::DropItem { slot, quantity } => {
            out.extend(
                handle_drop(world, player_id, slot, quantity)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::BankDeposit { inv_slot, quantity } => {
            out.extend(
                handle_bank_deposit(world, player_id, inv_slot, quantity)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::BankWithdraw {
            bank_slot,
            quantity,
        } => {
            out.extend(
                handle_bank_withdraw(world, player_id, bank_slot, quantity)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::EquipItem { inv_slot } => {
            out.extend(
                handle_equip(world, player_id, inv_slot)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::UnequipItem { slot } => {
            out.extend(
                handle_unequip(world, player_id, slot)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::ShopBuy {
            shop_id,
            item_id,
            quantity,
        } => {
            out.extend(
                handle_shop_buy(world, player_id, &shop_id, item_id, quantity)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::Chat { channel, message } => {
            out.extend(
                handle_chat(world, player_id, channel, message)
                    .into_iter()
                    .map(broadcast),
            );
        }
        ClientMessage::TradeRequest { target_player } => {
            out.extend(
                crate::economy::handle_trade_request(world, player_id, target_player)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::TradeOffer { target, items } => {
            out.extend(
                crate::economy::handle_trade_offer(world, player_id, target, items)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::TradeAccept { target } => {
            out.extend(
                crate::economy::handle_trade_accept(world, player_id, target)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::MarketPlaceOffer {
            item_id,
            quantity,
            price_per,
            is_buy,
        } => {
            out.extend(
                crate::economy::handle_market_offer(
                    world, player_id, item_id, quantity, price_per, is_buy,
                )
                .into_iter()
                .map(route),
            );
        }
        ClientMessage::MarketCancelOffer { offer_id } => {
            out.extend(
                crate::economy::handle_market_cancel(world, player_id, offer_id)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::FriendAdd { name } => {
            out.extend(
                crate::social::handle_friend_add(world, player_id, name)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::PrivateMessage { to, message } => {
            out.extend(
                crate::social::handle_private_message(world, player_id, to, message)
                    .into_iter()
                    .map(|(pid, msg)| (MessageTarget::Player(pid), msg)),
            );
        }
        ClientMessage::DialogueSelect {
            npc_entity,
            option_index,
        } => {
            out.extend(
                crate::quest::handle_dialogue_select(
                    world, player_id, npc_entity, option_index,
                )
                .into_iter()
                .map(route),
            );
        }
        ClientMessage::SelectSpecialization { skill, branch } => {
            if let Some(p) = world.players.get_mut(&player_id) {
                p.specialization.insert(skill, branch);
            }
        }
        ClientMessage::JoinMinigame { minigame_id } => {
            out.extend(
                crate::minigame::handle_join(world, player_id, minigame_id)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::ModeratorCommand { command } => {
            out.extend(
                crate::anticheat::handle_mod_command(world, player_id, command)
                    .into_iter()
                    .map(route),
            );
        }
        ClientMessage::Ping => out.push(route(ServerMessage::Pong)),
    }
    out
}

fn handle_walk(
    world: &mut GameWorld,
    player_id: PlayerId,
    target: TilePos,
) -> Option<ServerMessage> {
    let walkable = world.walkable_for_player(player_id);
    let player = world.players.get_mut(&player_id)?;
    if !crate::anticheat::validate_walk(player, target) {
        return Some(ServerMessage::Error {
            message: "Invalid movement".into(),
        });
    }
    let path = find_path(player.position, target, &walkable);
    if path.len() > 1 {
        player.action = PlayerAction::Walking { path, index: 0 };
        player.ticks_stationary = 0;
    }
    None
}

fn handle_scavenge(
    world: &mut GameWorld,
    player_id: PlayerId,
    object_entity: EntityId,
) -> Vec<ServerMessage> {
    let mut out = Vec::new();
    let obj = match world.objects.get(&object_entity) {
        Some(o) if !o.depleted => o.clone(),
        _ => {
            return vec![ServerMessage::Error {
                message: "Nothing to scavenge".into(),
            }];
        }
    };
    let def = match world.content.object(obj.object_id) {
        Some(d) => d.clone(),
        None => return out,
    };
    let player = match world.players.get_mut(&player_id) {
        Some(p) => p,
        None => return out,
    };
    if player.skills.level(Skill::Scavenging) < def.scavenging_level {
        return vec![ServerMessage::Error {
            message: format!("Need Scavenging level {}", def.scavenging_level),
        }];
    }
    if let Some(required_tool) = def.harvest_tag.and_then(|tag| tool_for_harvest(tag)) {
        let has_tool = player.inventory.slots.iter().flatten().any(|slot| {
            world
                .content
                .item(slot.item_id)
                .and_then(|i| i.tool_tag)
                .is_some_and(|t| t == required_tool)
        }) || player_has_equipped_tool(player, required_tool, &world.content);
        if !has_tool {
            return vec![ServerMessage::Error {
                message: format!("Need {:?} tool", required_tool),
            }];
        }
    }
    player.action = PlayerAction::Scavenging {
        object_entity,
        ticks_remaining: def.scavenging_ticks,
    };
    out
}

fn handle_refine(
    world: &mut GameWorld,
    player_id: PlayerId,
    recipe_id: &str,
) -> Vec<ServerMessage> {
    let recipe = match world.content.recipe(recipe_id) {
        Some(r) => r.clone(),
        None => {
            return vec![ServerMessage::Error {
                message: "Unknown recipe".into(),
            }];
        }
    };
    let player = match world.players.get_mut(&player_id) {
        Some(p) => p,
        None => return Vec::new(),
    };
    if player.skills.level(Skill::Fabrication) < recipe.fabrication_level {
        return vec![ServerMessage::Error {
            message: format!("Need Fabrication level {}", recipe.fabrication_level),
        }];
    }
    for input in &recipe.inputs {
        let has = player
            .inventory
            .slots
            .iter()
            .filter_map(|s| s.as_ref())
            .filter(|s| s.item_id == input.item_id)
            .map(|s| s.quantity)
            .sum::<u32>();
        if has < input.quantity {
            return vec![ServerMessage::Error {
                message: "Missing ingredients".into(),
            }];
        }
    }
    player.action = PlayerAction::Fabricating {
        recipe_id: recipe_id.to_string(),
        ticks_remaining: recipe.ticks,
    };
    Vec::new()
}

fn handle_attack(
    world: &mut GameWorld,
    player_id: PlayerId,
    target: EntityId,
    style: openmmo_common::CombatStyle,
) -> Vec<ServerMessage> {
    let player = match world.players.get_mut(&player_id) {
        Some(p) => p,
        None => return Vec::new(),
    };
    player.combat_target = Some(target);
    player.action = PlayerAction::Combat { target, style };
    Vec::new()
}

fn handle_spell(
    world: &mut GameWorld,
    player_id: PlayerId,
    target: EntityId,
    spell_id: &str,
) -> Vec<ServerMessage> {
    let spell = match world.content.spell(spell_id) {
        Some(s) => s.clone(),
        None => {
            return vec![ServerMessage::Error {
                message: "Unknown spell".into(),
            }];
        }
    };
    let player = match world.players.get(&player_id) {
        Some(p) => p,
        None => return Vec::new(),
    };
    if player.skills.level(Skill::Electrics) < spell.electrics_level {
        return vec![ServerMessage::Error {
            message: format!("Need Electrics level {}", spell.electrics_level),
        }];
    }
    let _ = player;
    let _ = target;
    let _ = spell;
    if let Some(p) = world.players.get_mut(&player_id) {
        p.action = PlayerAction::Casting {
            target,
            spell_id: spell_id.to_string(),
        };
    }
    Vec::new()
}

fn handle_pickup(
    world: &mut GameWorld,
    player_id: PlayerId,
    ground_entity: EntityId,
) -> Vec<ServerMessage> {
    let item = match world.ground_items.remove(&ground_entity) {
        Some(i) => i,
        None => return Vec::new(),
    };
    let player = match world.players.get_mut(&player_id) {
        Some(p) => p,
        None => return Vec::new(),
    };
    if player.position.chebyshev_distance(&item.position) > 2 {
        world.ground_items.insert(ground_entity, item);
        return Vec::new();
    }
    let stackable = world
        .content
        .item(item.item_id)
        .map(|i| i.stackable)
        .unwrap_or(true);
    let leftover = player
        .inventory
        .add_item(item.item_id, item.quantity, stackable);
    if leftover > 0 {
        world.ground_items.insert(
            ground_entity,
            openmmo_common::GroundItem {
                entity_id: ground_entity,
                item_id: item.item_id,
                quantity: leftover,
                position: item.position,
                despawn_ticks: 100,
            },
        );
    }
    vec![inventory_update(player)]
}

fn handle_drop(
    world: &mut GameWorld,
    player_id: PlayerId,
    slot: usize,
    quantity: u32,
) -> Vec<ServerMessage> {
    let player = match world.players.get_mut(&player_id) {
        Some(p) => p,
        None => return Vec::new(),
    };
    let slot_item = match player.inventory.slots.get_mut(slot) {
        Some(Some(s)) => s,
        _ => return Vec::new(),
    };
    let drop_qty = quantity.min(slot_item.quantity);
    let item_id = slot_item.item_id;
    slot_item.quantity -= drop_qty;
    if slot_item.quantity == 0 {
        player.inventory.slots[slot] = None;
    }
    let pos = player.position;
    drop(player);
    let eid = world.alloc_entity();
    world.ground_items.insert(
        eid,
        openmmo_common::GroundItem {
            entity_id: eid,
            item_id,
            quantity: drop_qty,
            position: pos,
            despawn_ticks: 200,
        },
    );
    if let Some(p) = world.players.get(&player_id) {
        vec![inventory_update(p)]
    } else {
        Vec::new()
    }
}

fn handle_bank_deposit(
    world: &mut GameWorld,
    player_id: PlayerId,
    inv_slot: usize,
    quantity: u32,
) -> Vec<ServerMessage> {
    let player = match world.players.get_mut(&player_id) {
        Some(p) => p,
        None => return Vec::new(),
    };
    let inv = match player.inventory.slots.get_mut(inv_slot) {
        Some(Some(s)) => s,
        _ => return Vec::new(),
    };
    let qty = quantity.min(inv.quantity);
    let item_id = inv.item_id;
    inv.quantity -= qty;
    if inv.quantity == 0 {
        player.inventory.slots[inv_slot] = None;
    }
    let stackable = world
        .content
        .item(item_id)
        .map(|i| i.stackable)
        .unwrap_or(true);
    let _ = player.bank.add_item(item_id, qty, stackable);
    if let Some(p) = world.players.get(&player_id) {
        vec![inventory_update(p)]
    } else {
        Vec::new()
    }
}

fn handle_bank_withdraw(
    world: &mut GameWorld,
    player_id: PlayerId,
    bank_slot: usize,
    quantity: u32,
) -> Vec<ServerMessage> {
    let player = match world.players.get_mut(&player_id) {
        Some(p) => p,
        None => return Vec::new(),
    };
    let bank = match player.bank.slots.get_mut(bank_slot) {
        Some(Some(s)) => s,
        _ => return Vec::new(),
    };
    let qty = quantity.min(bank.quantity);
    let item_id = bank.item_id;
    bank.quantity -= qty;
    if bank.quantity == 0 {
        player.bank.slots[bank_slot] = None;
    }
    let stackable = world
        .content
        .item(item_id)
        .map(|i| i.stackable)
        .unwrap_or(true);
    let _ = player.inventory.add_item(item_id, qty, stackable);
    if let Some(p) = world.players.get(&player_id) {
        vec![inventory_update(p)]
    } else {
        Vec::new()
    }
}

fn handle_chat(
    world: &mut GameWorld,
    player_id: PlayerId,
    channel: ChatChannel,
    message: String,
) -> Vec<ServerMessage> {
    if !crate::anticheat::validate_chat(&message) {
        return vec![ServerMessage::Error {
            message: "Invalid chat message".into(),
        }];
    }
    let name = world
        .players
        .get(&player_id)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    crate::social::broadcast_chat(world, channel, name, message)
}

fn tick_scavenge(world: &mut GameWorld, messages: &mut Vec<(MessageTarget, ServerMessage)>) {
    let player_ids: Vec<PlayerId> = world.players.keys().copied().collect();
    for pid in player_ids {
        let action = world.players.get(&pid).map(|p| p.action.clone());
        if let Some(PlayerAction::Scavenging {
            object_entity,
            ticks_remaining,
        }) = action
        {
            if ticks_remaining > 1 {
                if let Some(p) = world.players.get_mut(&pid) {
                    p.action = PlayerAction::Scavenging {
                        object_entity,
                        ticks_remaining: ticks_remaining - 1,
                    };
                }
                continue;
            }
            complete_scavenge(world, pid, object_entity, messages);
        }
    }
}

fn complete_scavenge(
    world: &mut GameWorld,
    player_id: PlayerId,
    object_entity: EntityId,
    messages: &mut Vec<(MessageTarget, ServerMessage)>,
) {
    let obj_state = match world.objects.get(&object_entity) {
        Some(o) => o.clone(),
        None => return,
    };
    let def = match world.content.object(obj_state.object_id) {
        Some(d) => d.clone(),
        None => return,
    };
    let levels = {
        let player = match world.players.get_mut(&player_id) {
            Some(p) => p,
            None => return,
        };
        player.action = PlayerAction::Idle;
        let levels = player.skills.grant_xp(Skill::Scavenging, def.scavenging_xp);
        if let Some(item_id) = def.harvest_item {
            let stackable = world
                .content
                .item(item_id)
                .map(|i| i.stackable)
                .unwrap_or(true);
            let _ = player.inventory.add_item(item_id, 1, stackable);
            messages.push((
                MessageTarget::Player(player_id),
                ServerMessage::CollectionLogEntry {
                    item_id,
                    source: def.name.clone(),
                },
            ));
        }
        messages.push((MessageTarget::Player(player_id), inventory_update(player)));
        messages.push((
            MessageTarget::Player(player_id),
            ServerMessage::XpDrop {
                skill: Skill::Scavenging,
                amount: def.scavenging_xp,
            },
        ));
        levels
    };
    for lvl in levels {
        if let Some(player) = world.players.get(&player_id) {
            messages.push((
                MessageTarget::Player(player_id),
                ServerMessage::SkillUpdate {
                    skills: player.skills.clone(),
                    levels_gained: vec![(Skill::Scavenging, lvl)],
                },
            ));
        }
        crate::quest::on_skill_level(world, player_id, Skill::Scavenging, lvl)
            .into_iter()
            .for_each(|msg| messages.push((MessageTarget::Player(player_id), msg)));
    }
    crate::quest::on_harvest(world, player_id, def.harvest_tag)
        .into_iter()
        .for_each(|msg| messages.push((MessageTarget::Player(player_id), msg)));
    if def.depletes {
        if let Some(o) = world.objects.get_mut(&object_entity) {
            o.depleted = true;
            o.respawn_ticks = 5;
        }
    }
}

fn tick_refine(world: &mut GameWorld, messages: &mut Vec<(MessageTarget, ServerMessage)>) {
    let player_ids: Vec<PlayerId> = world.players.keys().copied().collect();
    for pid in player_ids {
        let action = world.players.get(&pid).map(|p| p.action.clone());
        if let Some(PlayerAction::Fabricating {
            recipe_id,
            ticks_remaining,
        }) = action
        {
            if ticks_remaining > 1 {
                if let Some(p) = world.players.get_mut(&pid) {
                    p.action = PlayerAction::Fabricating {
                        recipe_id: recipe_id.clone(),
                        ticks_remaining: ticks_remaining - 1,
                    };
                }
                continue;
            }
            complete_refine(world, pid, &recipe_id, messages);
        }
    }
}

fn complete_refine(
    world: &mut GameWorld,
    player_id: PlayerId,
    recipe_id: &str,
    messages: &mut Vec<(MessageTarget, ServerMessage)>,
) {
    let recipe = match world.content.recipe(recipe_id) {
        Some(r) => r.clone(),
        None => return,
    };
    let levels = {
        let player = match world.players.get_mut(&player_id) {
            Some(p) => p,
            None => return,
        };
        for input in &recipe.inputs {
            let mut remaining = input.quantity;
            for slot in &mut player.inventory.slots {
                if let Some(s) = slot {
                    if s.item_id == input.item_id {
                        let take = remaining.min(s.quantity);
                        s.quantity -= take;
                        remaining -= take;
                        if s.quantity == 0 {
                            *slot = None;
                        }
                        if remaining == 0 {
                            break;
                        }
                    }
                }
            }
        }
        let stackable = world
            .content
            .item(recipe.output)
            .map(|i| i.stackable)
            .unwrap_or(true);
        let _ = player
            .inventory
            .add_item(recipe.output, recipe.output_qty, stackable);
        let levels = player
            .skills
            .grant_xp(Skill::Fabrication, recipe.fabrication_xp);
        player.action = PlayerAction::Idle;
        messages.push((MessageTarget::Player(player_id), inventory_update(player)));
        messages.push((
            MessageTarget::Player(player_id),
            ServerMessage::XpDrop {
                skill: Skill::Fabrication,
                amount: recipe.fabrication_xp,
            },
        ));
        levels
    };
    for lvl in levels {
        if let Some(player) = world.players.get(&player_id) {
            messages.push((
                MessageTarget::Player(player_id),
                ServerMessage::SkillUpdate {
                    skills: player.skills.clone(),
                    levels_gained: vec![(Skill::Fabrication, lvl)],
                },
            ));
        }
        crate::quest::on_skill_level(world, player_id, Skill::Fabrication, lvl)
            .into_iter()
            .for_each(|msg| messages.push((MessageTarget::Player(player_id), msg)));
    }
    crate::quest::on_refine(world, player_id, recipe_id)
        .into_iter()
        .for_each(|msg| messages.push((MessageTarget::Player(player_id), msg)));
}

fn tick_combat(world: &mut GameWorld, messages: &mut Vec<(MessageTarget, ServerMessage)>) {
    let player_actions: Vec<(PlayerId, PlayerAction)> = world
        .players
        .iter()
        .map(|(id, p)| (*id, p.action.clone()))
        .collect();

    for (pid, action) in player_actions {
        let content = world.content.clone();
        match action {
            PlayerAction::Combat { target, style } => {
                let mut rng = rand::thread_rng();
                let boss_target = world.boss.as_ref().map(|b| b.entity_id) == Some(target);
                if boss_target {
                    let in_range = world
                        .players
                        .get(&pid)
                        .zip(world.boss.as_ref())
                        .map(|(p, b)| p.position.chebyshev_distance(&b.position) <= 1)
                        .unwrap_or(false);
                    if in_range {
                        let player_eid = world.players.get(&pid).map(|p| p.entity_id);
                        if let (Some(player), Some(boss)) =
                            (world.players.get_mut(&pid), world.boss.as_mut())
                        {
                            let max_hit = crate::combat::player_max_hit(player, style, &content);
                            let dmg = crate::combat::roll_damage(max_hit);
                            boss.hp = boss.hp.saturating_sub(dmg);
                            if let Some(eid) = player_eid {
                                messages.push((
                                    MessageTarget::Player(pid),
                                    ServerMessage::Damage {
                                        source: eid,
                                        target,
                                        amount: dmg,
                                        style,
                                    },
                                ));
                            }
                            if boss.hp == 0 {
                                world.boss = None;
                                messages.push((
                                    MessageTarget::Player(pid),
                                    ServerMessage::Death {
                                        entity: target,
                                        killer: player_eid,
                                    },
                                ));
                            }
                        }
                    }
                    continue;
                }
                if rng.gen_bool(0.5) {
                    let in_range = world
                        .players
                        .get(&pid)
                        .zip(world.npcs.get(&target))
                        .map(|(p, n)| p.position.chebyshev_distance(&n.position) <= 1)
                        .unwrap_or(false);
                    if in_range {
                        let player_eid = world.players.get(&pid).map(|p| p.entity_id);
                        if let (Some(player), Some(npc)) =
                            (world.players.get_mut(&pid), world.npcs.get_mut(&target))
                        {
                            if let Some(dmg) = player_attack_npc(player, npc, style, &content) {
                                if let Some(eid) = player_eid {
                                    messages.push((
                                        MessageTarget::Player(pid),
                                        ServerMessage::Damage {
                                            source: eid,
                                            target,
                                            amount: dmg,
                                            style,
                                        },
                                    ));
                                }
                                let npc_dead = !npc.alive;
                                let npc_id_copy = npc.npc_id;
                                drop(npc);
                                drop(player);
                                if npc_dead {
                                    messages.push((
                                        MessageTarget::Player(pid),
                                        ServerMessage::Death {
                                            entity: target,
                                            killer: player_eid,
                                        },
                                    ));
                                    handle_npc_loot(world, pid, target, messages);
                                    crate::quest::on_kill(world, pid, npc_id_copy)
                                        .into_iter()
                                        .for_each(|msg| messages.push((MessageTarget::Player(pid), msg)));
                                }
                            }
                        }
                    }
                } else {
                    let in_range = world
                        .players
                        .get(&pid)
                        .zip(world.npcs.get(&target))
                        .map(|(p, n)| n.alive && p.position.chebyshev_distance(&n.position) <= 1)
                        .unwrap_or(false);
                    if in_range {
                        if let (Some(npc), Some(player)) =
                            (world.npcs.get(&target), world.players.get_mut(&pid))
                        {
                            let player_eid = player.entity_id;
                            if let Some(dmg) = npc_attack_player(npc, player, &content) {
                                messages.push((
                                    MessageTarget::Player(pid),
                                    ServerMessage::Damage {
                                        source: target,
                                        target: player_eid,
                                        amount: dmg,
                                        style: openmmo_common::CombatStyle::Melee,
                                    },
                                ));
                                if player.hp == 0 {
                                    respawn_player(world, pid, messages);
                                }
                            }
                        }
                    }
                }
            }
            PlayerAction::Casting { target, spell_id } => {
                let spell = world.content.spell(&spell_id).cloned();
                let in_range = world
                    .players
                    .get(&pid)
                    .zip(world.npcs.get(&target))
                    .map(|(p, n)| p.position.chebyshev_distance(&n.position) <= 5)
                    .unwrap_or(false);
                if !in_range {
                    continue;
                }
                let Some(spell) = spell else { continue };
                let player_eid = world.players.get(&pid).map(|p| p.entity_id);
                let Some(player_eid) = player_eid else {
                    continue;
                };
                if let (Some(player), Some(npc)) =
                    (world.players.get_mut(&pid), world.npcs.get_mut(&target))
                {
                    let npc_dead =
                        if let Some(dmg) = cast_spell(player, npc, spell.max_hit, spell.xp) {
                            messages.push((
                                MessageTarget::Player(pid),
                                ServerMessage::Damage {
                                    source: player_eid,
                                    target,
                                    amount: dmg,
                                    style: openmmo_common::CombatStyle::Magic,
                                },
                            ));
                            !npc.alive
                        } else {
                            false
                        };
                    player.action = PlayerAction::Idle;
                    if npc_dead {
                        drop(player);
                        handle_npc_loot(world, pid, target, messages);
                    }
                }
            }
            _ => {}
        }
    }
}

fn handle_npc_loot(
    world: &mut GameWorld,
    player_id: PlayerId,
    npc_entity: EntityId,
    messages: &mut Vec<(MessageTarget, ServerMessage)>,
) {
    let npc = match world.npcs.get(&npc_entity) {
        Some(n) => n.clone(),
        None => return,
    };
    let def = match world.content.npc(npc.npc_id) {
        Some(d) => d.clone(),
        None => return,
    };
    let mut rng = rand::thread_rng();
    let mut loot = Vec::new();
    for entry in &def.loot_table {
        if rng.gen::<f32>() <= entry.rate {
            let qty = rng.gen_range(entry.min_qty..=entry.max_qty);
            let eid = world.alloc_entity();
            let pos = npc.position;
            let ground = openmmo_common::GroundItem {
                entity_id: eid,
                item_id: entry.item_id,
                quantity: qty,
                position: pos,
                despawn_ticks: 300,
            };
            world.ground_items.insert(eid, ground.clone());
            loot.push(ground);
        }
    }
    if !loot.is_empty() {
        messages.push((MessageTarget::Player(player_id), ServerMessage::LootSpawn { items: loot }));
    }
    if let Some(npc) = world.npcs.get_mut(&npc_entity) {
        npc.respawn_ticks = def.respawn_ticks;
    }
}

fn clear_npc_aggro_for_player(world: &mut GameWorld, player_id: PlayerId) {
    for npc in world.npcs.values_mut() {
        if npc.aggro_target == Some(player_id) {
            npc.aggro_target = None;
        }
    }
}

fn respawn_player(
    world: &mut GameWorld,
    player_id: PlayerId,
    messages: &mut Vec<(MessageTarget, ServerMessage)>,
) {
    let spawn = world
        .content
        .regions
        .first()
        .map(|r| r.spawn)
        .unwrap_or(TilePos::new(5, 5));
    if let Some(player) = world.players.get_mut(&player_id) {
        messages.push((
            MessageTarget::Player(player_id),
            ServerMessage::Death {
                entity: player.entity_id,
                killer: None,
            },
        ));
        player.position = spawn;
        player.hp = player.max_hp;
        player.action = PlayerAction::Idle;
        player.combat_target = None;
        clear_npc_aggro_for_player(world, player_id);
    }
}

fn tick_npc_respawn(world: &mut GameWorld) {
    for npc in world.npcs.values_mut() {
        if !npc.alive {
            if npc.respawn_ticks > 0 {
                npc.respawn_ticks -= 1;
            }
            if npc.respawn_ticks == 0 {
                let def = world.content.npc(npc.npc_id).cloned();
                if let Some(def) = def {
                    npc.alive = true;
                    npc.hp = def.max_hp;
                    npc.aggro_target = None;
                    npc.position = npc.home_position;
                }
            }
        }
    }
}

fn tick_object_respawn(world: &mut GameWorld) {
    for obj in world.objects.values_mut() {
        if obj.depleted && obj.respawn_ticks > 0 {
            obj.respawn_ticks -= 1;
            if obj.respawn_ticks == 0 {
                obj.depleted = false;
            }
        }
    }
}

fn tick_ground_items(world: &mut GameWorld) {
    let expired: Vec<EntityId> = world
        .ground_items
        .iter()
        .filter_map(|(id, g)| {
            if g.despawn_ticks == 0 {
                Some(*id)
            } else {
                None
            }
        })
        .collect();
    for id in expired {
        world.ground_items.remove(&id);
    }
    for item in world.ground_items.values_mut() {
        if item.despawn_ticks > 0 {
            item.despawn_ticks -= 1;
        }
    }
}

fn tick_minigame(world: &mut GameWorld, messages: &mut Vec<(MessageTarget, ServerMessage)>) {
    let mut inner = Vec::new();
    crate::minigame::tick(world, &mut inner);
    for (pid, msg) in inner {
        messages.push((MessageTarget::Player(pid), msg));
    }
}

fn tick_boss(world: &mut GameWorld, messages: &mut Vec<(MessageTarget, ServerMessage)>) {
    let mut inner = Vec::new();
    crate::minigame::tick_boss(world, &mut inner);
    for (pid, msg) in inner {
        messages.push((MessageTarget::Player(pid), msg));
    }
}

fn tick_economy(world: &mut GameWorld) {
    crate::economy::match_offers(world);
}

fn tick_ledger_contracts(world: &mut GameWorld, messages: &mut Vec<(MessageTarget, ServerMessage)>) {
    for player in world.players.values() {
        if let Some(contract) = world.economy.ledger_contracts.get(&player.id) {
            messages.push((
                MessageTarget::Player(player.id),
                ServerMessage::LedgerUpdate {
                    rank: player.ledger_rank,
                    points: player.ledger_points,
                    active_contract: Some(LedgerContract {
                        npc_id: contract.npc_id,
                        name: contract.name.clone(),
                        remaining: contract.remaining,
                        reward_points: contract.reward_points,
                    }),
                },
            ));
        }
    }
}

pub fn inventory_update(player: &openmmo_common::PlayerState) -> ServerMessage {
    ServerMessage::InventoryUpdate {
        inventory: player.inventory.clone(),
        bank: player.bank.clone(),
        equipment: player.equipment.clone(),
    }
}

pub fn snapshot_message(world: &GameWorld, local: Option<PlayerId>) -> ServerMessage {
    ServerMessage::WorldSnapshot {
        tick: world.tick,
        entities: world.entities_snapshot(),
        local_player: local,
    }
}

fn tool_for_harvest(tag: HarvestTag) -> Option<ToolTag> {
    match tag {
        HarvestTag::Timber => Some(ToolTag::Axe),
        HarvestTag::Ore => Some(ToolTag::Pick),
        HarvestTag::Water => Some(ToolTag::Rod),
        HarvestTag::Flora => Some(ToolTag::Knife),
    }
}

fn player_has_equipped_tool(
    player: &openmmo_common::PlayerState,
    tool: ToolTag,
    content: &openmmo_common::ContentPack,
) -> bool {
    let slots = [
        player.equipment.weapon.as_ref(),
        player.equipment.shield.as_ref(),
    ];
    slots.into_iter().flatten().any(|slot| {
        content
            .item(slot.item_id)
            .and_then(|i| i.tool_tag)
            .is_some_and(|t| t == tool)
    })
}

fn handle_equip(
    world: &mut GameWorld,
    player_id: PlayerId,
    inv_slot: usize,
) -> Vec<ServerMessage> {
    let item_id = {
        let player = match world.players.get(&player_id) {
            Some(p) => p,
            None => return Vec::new(),
        };
        let slot = match player.inventory.slots.get(inv_slot) {
            Some(Some(s)) => s,
            _ => return Vec::new(),
        };
        slot.item_id
    };
    let def = match world.content.item(item_id) {
        Some(d) => d.clone(),
        None => return Vec::new(),
    };
    let equip_slot = match def.equip_slot {
        Some(s) => s,
        None => {
            return vec![ServerMessage::Error {
                message: "Item is not equippable".into(),
            }];
        }
    };
    let player = match world.players.get_mut(&player_id) {
        Some(p) => p,
        None => return Vec::new(),
    };
    let removed = player.inventory.slots[inv_slot].take();
    let Some(removed) = removed else {
        return Vec::new();
    };
    let target = match equip_slot {
        EquipSlot::Head => &mut player.equipment.head,
        EquipSlot::Body => &mut player.equipment.body,
        EquipSlot::Legs => &mut player.equipment.legs,
        EquipSlot::Weapon => &mut player.equipment.weapon,
        EquipSlot::Shield => &mut player.equipment.shield,
    };
    if let Some(prev) = target.take() {
        let stackable = world
            .content
            .item(prev.item_id)
            .map(|i| i.stackable)
            .unwrap_or(true);
        let _ = player.inventory.add_item(prev.item_id, prev.quantity, stackable);
    }
    *target = Some(removed);
    vec![inventory_update(player)]
}

fn handle_unequip(
    world: &mut GameWorld,
    player_id: PlayerId,
    slot: EquipSlot,
) -> Vec<ServerMessage> {
    let mut ground_item = None;
    {
        let player = match world.players.get_mut(&player_id) {
            Some(p) => p,
            None => return Vec::new(),
        };
        let equipped = match slot {
            EquipSlot::Head => player.equipment.head.take(),
            EquipSlot::Body => player.equipment.body.take(),
            EquipSlot::Legs => player.equipment.legs.take(),
            EquipSlot::Weapon => player.equipment.weapon.take(),
            EquipSlot::Shield => player.equipment.shield.take(),
        };
        let Some(item) = equipped else {
            return Vec::new();
        };
        let stackable = world
            .content
            .item(item.item_id)
            .map(|i| i.stackable)
            .unwrap_or(true);
        let leftover = player
            .inventory
            .add_item(item.item_id, item.quantity, stackable);
        if leftover > 0 {
            ground_item = Some((item.item_id, leftover, player.position));
        }
    }
    if let Some((item_id, quantity, position)) = ground_item {
        let eid = world.alloc_entity();
        world.ground_items.insert(
            eid,
            openmmo_common::GroundItem {
                entity_id: eid,
                item_id,
                quantity,
                position,
                despawn_ticks: 200,
            },
        );
    }
    world
        .players
        .get(&player_id)
        .map(inventory_update)
        .into_iter()
        .collect()
}

fn handle_shop_buy(
    world: &mut GameWorld,
    player_id: PlayerId,
    shop_id: &str,
    item_id: openmmo_common::ItemId,
    quantity: u32,
) -> Vec<ServerMessage> {
    let shop = match world.content.shops.iter().find(|s| s.id == shop_id) {
        Some(s) => s.clone(),
        None => {
            return vec![ServerMessage::Error {
                message: "Unknown shop".into(),
            }];
        }
    };
    let stock = match shop.stock.iter().find(|s| s.item_id == item_id) {
        Some(s) => s.clone(),
        None => {
            return vec![ServerMessage::Error {
                message: "Item not in stock".into(),
            }];
        }
    };
    let total_cost = stock.price.saturating_mul(quantity);
    let player = match world.players.get_mut(&player_id) {
        Some(p) => p,
        None => return Vec::new(),
    };
    let coins = player
        .inventory
        .slots
        .iter()
        .flatten()
        .filter(|s| s.item_id.0 == 1)
        .map(|s| s.quantity)
        .sum::<u32>();
    if coins < total_cost {
        return vec![ServerMessage::Error {
            message: "Not enough currency".into(),
        }];
    }
    let mut remaining_cost = total_cost;
    for slot in player.inventory.slots.iter_mut() {
        if let Some(s) = slot {
            if s.item_id.0 == 1 && remaining_cost > 0 {
                let take = remaining_cost.min(s.quantity);
                s.quantity -= take;
                remaining_cost -= take;
                if s.quantity == 0 {
                    *slot = None;
                }
            }
        }
    }
    let stackable = world
        .content
        .item(item_id)
        .map(|i| i.stackable)
        .unwrap_or(true);
    let _ = player.inventory.add_item(item_id, quantity, stackable);
    vec![inventory_update(player)]
}

fn move_npc_toward(world: &mut GameWorld, entity_id: EntityId, from: TilePos, to: TilePos) -> bool {
    if from == to {
        return false;
    }
    let dx = (to.x - from.x).clamp(-1, 1);
    let dy = (to.y - from.y).clamp(-1, 1);
    let next = TilePos::new(from.x + dx, from.y + dy);
    if !world.is_walkable(next) {
        return false;
    }
    if let Some(npc) = world.npcs.get_mut(&entity_id) {
        npc.position = next;
    }
    true
}

fn tick_npc_ai(world: &mut GameWorld, messages: &mut Vec<(MessageTarget, ServerMessage)>) {
    let npc_snapshots: Vec<_> = world
        .npcs
        .iter()
        .filter(|(_, n)| n.alive)
        .map(|(eid, n)| {
            (
                *eid,
                n.npc_id,
                n.position,
                n.home_position,
                n.aggro_target,
            )
        })
        .collect();
    for (entity_id, npc_id, npc_pos, home_pos, aggro_target) in npc_snapshots {
        let def = match world.content.npc(npc_id) {
            Some(d) => d.clone(),
            None => continue,
        };
        if def.aggro_range == 0 {
            continue;
        }
        let mut target_player = aggro_target.filter(|pid| world.players.contains_key(pid));
        if target_player.is_none() && aggro_target.is_some() {
            if let Some(npc) = world.npcs.get_mut(&entity_id) {
                npc.aggro_target = None;
            }
        }
        if target_player.is_none() {
            for (pid, player) in &world.players {
                if player.position.chebyshev_distance(&npc_pos) <= def.aggro_range {
                    target_player = Some(*pid);
                    break;
                }
            }
        }
        if let Some(target_pid) = target_player {
            if let Some(npc) = world.npcs.get_mut(&entity_id) {
                npc.aggro_target = Some(target_pid);
            }
            let player_pos = world
                .players
                .get(&target_pid)
                .map(|p| p.position)
                .unwrap_or(npc_pos);
            if npc_pos.chebyshev_distance(&player_pos) > 1 {
                move_npc_toward(world, entity_id, npc_pos, player_pos);
            } else if let (Some(npc), Some(player)) = (
                world.npcs.get(&entity_id),
                world.players.get_mut(&target_pid),
            ) {
                let content = world.content.clone();
                let player_eid = player.entity_id;
                if let Some(dmg) = crate::combat::npc_attack_player(npc, player, &content) {
                    messages.push((
                        MessageTarget::Player(target_pid),
                        ServerMessage::Damage {
                            source: entity_id,
                            target: player_eid,
                            amount: dmg,
                            style: openmmo_common::CombatStyle::Melee,
                        },
                    ));
                    if player.hp == 0 {
                        drop(player);
                        respawn_player(world, target_pid, messages);
                    }
                }
            }
            continue;
        }

        if npc_pos != home_pos {
            move_npc_toward(world, entity_id, npc_pos, home_pos);
        }
    }
}

#[cfg(test)]
mod routing_tests {
    use super::*;
    use openmmo_common::{Inventory, InventorySlot, ItemId, PlayerState, SkillBook, TilePos};
    use uuid::Uuid;

    fn test_player(id: u128) -> PlayerState {
        let pid = PlayerId(Uuid::from_u128(id));
        let mut player = PlayerState {
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
            quest_progress: Default::default(),
            quest_counters: Default::default(),
            friends: Vec::new(),
            ledger_rank: 0,
            ledger_points: 0,
            specialization: Default::default(),
            is_moderator: false,
            last_position: TilePos::new(0, 0),
            ticks_stationary: 1,
        };
        player.inventory.slots[0] = Some(InventorySlot {
            item_id: ItemId(2),
            quantity: 5,
        });
        player
    }

    #[test]
    fn private_actions_route_to_initiating_player() {
        let mut world = GameWorld::new(openmmo_common::ContentPack::default());
        let pid = PlayerId(Uuid::from_u128(1));
        world.players.insert(pid, test_player(1));
        let msgs = handle_client_message(
            &mut world,
            pid,
            ClientMessage::BankDeposit {
                inv_slot: 0,
                quantity: 2,
            },
        );
        assert!(!msgs.is_empty());
        assert!(msgs
            .iter()
            .all(|(target, _)| matches!(target, MessageTarget::Player(p) if *p == pid)));
        assert_eq!(
            world.players.get(&pid).unwrap().inventory.slots[0]
                .as_ref()
                .map(|s| s.quantity),
            Some(3)
        );
    }

    #[test]
    fn process_tick_routes_refine_completion_to_player() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let content = openmmo_common::load_content(&path).expect("content dir");
        let mut world = GameWorld::new(content);
        let pid = PlayerId(Uuid::from_u128(1));
        let mut player = test_player(1);
        player.inventory.slots[0] = Some(InventorySlot {
            item_id: ItemId(2),
            quantity: 5,
        });
        world.players.insert(pid, player);
        world.players.get_mut(&pid).unwrap().action = openmmo_common::PlayerAction::Fabricating {
            recipe_id: "timber_to_plank".into(),
            ticks_remaining: 0,
        };
        let msgs = process_tick(&mut world);
        assert!(msgs.iter().any(|(target, msg)| {
            matches!(target, MessageTarget::Player(p) if *p == pid)
                && matches!(msg, ServerMessage::InventoryUpdate { .. })
        }));
    }

    #[test]
    fn walk_intent_to_distant_tile_does_not_remove_player() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let content = openmmo_common::load_content(&path).expect("content dir");
        let mut world = GameWorld::new(content);
        let pid = PlayerId(Uuid::from_u128(1));
        let mut player = test_player(1);
        player.position = TilePos::new(5, 5);
        player.last_position = TilePos::new(5, 5);
        player.ticks_stationary = 1;
        world.players.insert(pid, player);

        let msgs = handle_client_message(
            &mut world,
            pid,
            ClientMessage::WalkIntent {
                target: TilePos::new(10, 10),
            },
        );

        assert!(world.players.contains_key(&pid));
        assert!(!msgs.iter().any(|(_, msg)| matches!(
            msg,
            ServerMessage::Error { message } if message == "Movement rejected"
        )));
        assert!(matches!(
            world.players.get(&pid).unwrap().action,
            openmmo_common::PlayerAction::Walking { .. }
        ));
    }

    #[test]
    fn walk_intent_avoids_npc_tile_but_reaches_adjacent_tile() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let content = openmmo_common::load_content(&path).expect("content dir");
        let mut world = GameWorld::new(content);
        let pid = PlayerId(Uuid::from_u128(1));
        let mut player = test_player(1);
        player.position = TilePos::new(5, 5);
        world.players.insert(pid, player);

        let msgs = handle_client_message(
            &mut world,
            pid,
            ClientMessage::WalkIntent {
                target: TilePos::new(7, 5),
            },
        );

        assert!(msgs.is_empty());
        let action = &world.players.get(&pid).unwrap().action;
        let openmmo_common::PlayerAction::Walking { path, .. } = action else {
            panic!("expected walking action");
        };
        assert!(!path.contains(&TilePos::new(6, 5)));
        assert_eq!(*path.last().unwrap(), TilePos::new(7, 5));
    }

    #[test]
    fn npc_returns_home_after_combat() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let content = openmmo_common::load_content(&path).expect("content dir");
        let mut world = GameWorld::new(content);

        let entity_id = world
            .npcs
            .iter()
            .find(|(_, n)| {
                world
                    .content
                    .npc(n.npc_id)
                    .map(|d| d.aggro_range > 0)
                    .unwrap_or(false)
            })
            .map(|(eid, _)| *eid)
            .expect("hostile npc");

        let home = world.npcs.get(&entity_id).unwrap().home_position;
        {
            let npc = world.npcs.get_mut(&entity_id).unwrap();
            npc.position = TilePos::new(home.x + 5, home.y + 5);
            npc.aggro_target = Some(PlayerId(Uuid::from_u128(99)));
        }

        let start_dist = world
            .npcs
            .get(&entity_id)
            .unwrap()
            .position
            .chebyshev_distance(&home);
        process_tick(&mut world);
        let after_one = world.npcs.get(&entity_id).unwrap().position;
        assert!(
            after_one.chebyshev_distance(&home) < start_dist,
            "npc should move toward home after combat ends"
        );

        for _ in 0..20 {
            process_tick(&mut world);
            if world.npcs.get(&entity_id).unwrap().position == home {
                break;
            }
        }
        assert_eq!(world.npcs.get(&entity_id).unwrap().position, home);
    }

    #[test]
    fn npc_respawns_at_home_tile() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let content = openmmo_common::load_content(&path).expect("content dir");
        let mut world = GameWorld::new(content);

        let entity_id = world
            .npcs
            .iter()
            .find(|(_, n)| {
                world
                    .content
                    .npc(n.npc_id)
                    .map(|d| d.aggro_range > 0)
                    .unwrap_or(false)
            })
            .map(|(eid, _)| *eid)
            .expect("hostile npc");

        let home = world.npcs.get(&entity_id).unwrap().home_position;
        {
            let npc = world.npcs.get_mut(&entity_id).unwrap();
            npc.position = TilePos::new(home.x + 5, home.y + 5);
            npc.alive = false;
            npc.hp = 0;
            npc.respawn_ticks = 1;
        }

        process_tick(&mut world);
        let npc = world.npcs.get(&entity_id).unwrap();
        assert!(npc.alive);
        assert_eq!(npc.position, home);
    }
}
