use openmmo_common::{
    EntityId, PlayerAction, PlayerId, Skill, TilePos,
};
use openmmo_protocol::{ChatChannel, ClientMessage, LedgerContract, ServerMessage};
use rand::Rng;

use crate::combat::{cast_spell, npc_attack_player, player_attack_npc};
use crate::pathfinding::find_path;
use crate::state::GameWorld;

pub fn process_tick(world: &mut GameWorld) -> Vec<(PlayerId, ServerMessage)> {
    world.tick += 1;
    let mut messages = Vec::new();

    crate::anticheat::PluginApi::on_tick(world);

    for player in world.players.values_mut() {
        if let PlayerAction::Walking { path, index } = &mut player.action {
            if *index + 1 < path.len() {
                *index += 1;
                player.position = path[*index];
            } else {
                player.action = PlayerAction::Idle;
            }
        }
    }

    tick_harvest(world, &mut messages);
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
) -> Vec<ServerMessage> {
    let mut out = Vec::new();
    match msg {
        ClientMessage::Login { .. } => {}
        ClientMessage::WalkIntent { target } => {
            if let Some(resp) = handle_walk(world, player_id, target) {
                out.push(resp);
            }
        }
        ClientMessage::Harvest { object_entity } => {
            out.extend(handle_harvest(world, player_id, object_entity));
        }
        ClientMessage::InteractObject { object_entity } => {
            out.extend(handle_harvest(world, player_id, object_entity));
        }
        ClientMessage::Refine { recipe_id } => {
            out.extend(handle_refine(world, player_id, &recipe_id));
        }
        ClientMessage::Attack { target, style } => {
            out.extend(handle_attack(world, player_id, target, style));
        }
        ClientMessage::CastSpell { target, spell_id } => {
            out.extend(handle_spell(world, player_id, target, &spell_id));
        }
        ClientMessage::PickupItem { ground_entity } => {
            out.extend(handle_pickup(world, player_id, ground_entity));
        }
        ClientMessage::DropItem { slot, quantity } => {
            out.extend(handle_drop(world, player_id, slot, quantity));
        }
        ClientMessage::BankDeposit { inv_slot, quantity } => {
            out.extend(handle_bank_deposit(world, player_id, inv_slot, quantity));
        }
        ClientMessage::BankWithdraw { bank_slot, quantity } => {
            out.extend(handle_bank_withdraw(world, player_id, bank_slot, quantity));
        }
        ClientMessage::Chat { channel, message } => {
            out.extend(handle_chat(world, player_id, channel, message));
        }
        ClientMessage::TradeRequest { target_player } => {
            out.extend(crate::economy::handle_trade_request(world, player_id, target_player));
        }
        ClientMessage::TradeOffer { target, items } => {
            out.extend(crate::economy::handle_trade_offer(world, player_id, target, items));
        }
        ClientMessage::TradeAccept { target } => {
            out.extend(crate::economy::handle_trade_accept(world, player_id, target));
        }
        ClientMessage::GePlaceOffer {
            item_id,
            quantity,
            price_per,
            is_buy,
        } => {
            out.extend(crate::economy::handle_ge_offer(
                world, player_id, item_id, quantity, price_per, is_buy,
            ));
        }
        ClientMessage::GeCancelOffer { offer_id } => {
            out.extend(crate::economy::handle_ge_cancel(world, player_id, offer_id));
        }
        ClientMessage::FriendAdd { name } => {
            out.extend(crate::social::handle_friend_add(world, player_id, name));
        }
        ClientMessage::PrivateMessage { to, message } => {
            out.extend(crate::social::handle_private_message(world, player_id, to, message));
        }
        ClientMessage::DialogueSelect {
            npc_entity,
            option_index,
        } => {
            out.extend(crate::quest::handle_dialogue_select(
                world, player_id, npc_entity, option_index,
            ));
        }
        ClientMessage::SelectSpecialization { skill, branch } => {
            if let Some(p) = world.players.get_mut(&player_id) {
                p.specialization.insert(skill, branch);
            }
        }
        ClientMessage::JoinMinigame { minigame_id } => {
            out.extend(crate::minigame::handle_join(world, player_id, minigame_id));
        }
        ClientMessage::ModeratorCommand { command } => {
            out.extend(crate::anticheat::handle_mod_command(world, player_id, command));
        }
        ClientMessage::Ping => out.push(ServerMessage::Pong),
    }
    out
}

fn handle_walk(world: &mut GameWorld, player_id: PlayerId, target: TilePos) -> Option<ServerMessage> {
    let walkable = world.walkable.clone();
    let player = world.players.get_mut(&player_id)?;
    if !crate::anticheat::validate_walk(player, target) {
        return Some(ServerMessage::Error {
            message: "Invalid movement".into(),
        });
    }
    let path = find_path(player.position, target, &walkable);
    if path.len() > 1 {
        player.action = PlayerAction::Walking { path, index: 0 };
    }
    None
}

fn handle_harvest(
    world: &mut GameWorld,
    player_id: PlayerId,
    object_entity: EntityId,
) -> Vec<ServerMessage> {
    let mut out = Vec::new();
    let obj = match world.objects.get(&object_entity) {
        Some(o) if !o.depleted => o.clone(),
        _ => {
            return vec![ServerMessage::Error {
                message: "Nothing to harvest".into(),
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
    if player.skills.level(Skill::Harvest) < def.harvest_level {
        return vec![ServerMessage::Error {
            message: format!("Need Harvest level {}", def.harvest_level),
        }];
    }
    player.action = PlayerAction::Harvesting {
        object_entity,
        ticks_remaining: def.harvest_ticks,
    };
    out
}

fn handle_refine(world: &mut GameWorld, player_id: PlayerId, recipe_id: &str) -> Vec<ServerMessage> {
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
    if player.skills.level(Skill::Refinement) < recipe.refinement_level {
        return vec![ServerMessage::Error {
            message: format!("Need Refinement level {}", recipe.refinement_level),
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
    player.action = PlayerAction::Refining {
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
    if player.skills.level(Skill::Arcana) < spell.arcana_level {
        return vec![ServerMessage::Error {
            message: format!("Need Arcana level {}", spell.arcana_level),
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
    let stackable = world.content.item(item_id).map(|i| i.stackable).unwrap_or(true);
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
    let stackable = world.content.item(item_id).map(|i| i.stackable).unwrap_or(true);
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

fn tick_harvest(world: &mut GameWorld, messages: &mut Vec<(PlayerId, ServerMessage)>) {
    let player_ids: Vec<PlayerId> = world.players.keys().copied().collect();
    for pid in player_ids {
        let action = world.players.get(&pid).map(|p| p.action.clone());
        if let Some(PlayerAction::Harvesting {
            object_entity,
            ticks_remaining,
        }) = action
        {
            if ticks_remaining > 1 {
                if let Some(p) = world.players.get_mut(&pid) {
                    p.action = PlayerAction::Harvesting {
                        object_entity,
                        ticks_remaining: ticks_remaining - 1,
                    };
                }
                continue;
            }
            complete_harvest(world, pid, object_entity, messages);
        }
    }
}

fn complete_harvest(
    world: &mut GameWorld,
    player_id: PlayerId,
    object_entity: EntityId,
    messages: &mut Vec<(PlayerId, ServerMessage)>,
) {
    let obj_state = match world.objects.get(&object_entity) {
        Some(o) => o.clone(),
        None => return,
    };
    let def = match world.content.object(obj_state.object_id) {
        Some(d) => d.clone(),
        None => return,
    };
    let player = match world.players.get_mut(&player_id) {
        Some(p) => p,
        None => return,
    };
    player.action = PlayerAction::Idle;
    let levels = player.skills.grant_xp(Skill::Harvest, def.harvest_xp);
    if let Some(item_id) = def.harvest_item {
        let stackable = world.content.item(item_id).map(|i| i.stackable).unwrap_or(true);
        let _ = player.inventory.add_item(item_id, 1, stackable);
        messages.push((
            player_id,
            ServerMessage::CollectionLogEntry {
                item_id,
                source: def.name.clone(),
            },
        ));
    }
    messages.push((player_id, inventory_update(player)));
    messages.push((
        player_id,
        ServerMessage::XpDrop {
            skill: Skill::Harvest,
            amount: def.harvest_xp,
        },
    ));
    for lvl in levels {
        messages.push((
            player_id,
            ServerMessage::SkillUpdate {
                skills: player.skills.clone(),
                levels_gained: vec![(Skill::Harvest, lvl)],
            },
        ));
    }
    crate::quest::on_harvest(world, player_id, def.harvest_tag);
    if def.depletes {
        if let Some(o) = world.objects.get_mut(&object_entity) {
            o.depleted = true;
            o.respawn_ticks = 5;
        }
    }
}

fn tick_refine(world: &mut GameWorld, messages: &mut Vec<(PlayerId, ServerMessage)>) {
    let player_ids: Vec<PlayerId> = world.players.keys().copied().collect();
    for pid in player_ids {
        let action = world.players.get(&pid).map(|p| p.action.clone());
        if let Some(PlayerAction::Refining {
            recipe_id,
            ticks_remaining,
        }) = action
        {
            if ticks_remaining > 1 {
                if let Some(p) = world.players.get_mut(&pid) {
                    p.action = PlayerAction::Refining {
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
    messages: &mut Vec<(PlayerId, ServerMessage)>,
) {
    let recipe = match world.content.recipe(recipe_id) {
        Some(r) => r.clone(),
        None => return,
    };
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
    let levels = player.skills.grant_xp(Skill::Refinement, recipe.refinement_xp);
    player.action = PlayerAction::Idle;
    messages.push((player_id, inventory_update(player)));
    messages.push((
        player_id,
        ServerMessage::XpDrop {
            skill: Skill::Refinement,
            amount: recipe.refinement_xp,
        },
    ));
    for lvl in levels {
        messages.push((
            player_id,
            ServerMessage::SkillUpdate {
                skills: player.skills.clone(),
                levels_gained: vec![(Skill::Refinement, lvl)],
            },
        ));
    }
    crate::quest::on_refine(world, player_id, recipe_id);
}

fn tick_combat(world: &mut GameWorld, messages: &mut Vec<(PlayerId, ServerMessage)>) {
    let player_actions: Vec<(PlayerId, PlayerAction)> = world
        .players
        .iter()
        .map(|(id, p)| (*id, p.action.clone()))
        .collect();

    for (pid, action) in player_actions {
        match action {
            PlayerAction::Combat { target, style } => {
                let mut rng = rand::thread_rng();
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
                            if let Some(dmg) = player_attack_npc(player, npc, style) {
                                if let Some(eid) = player_eid {
                                    messages.push((
                                        pid,
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
                                        pid,
                                        ServerMessage::Death {
                                            entity: target,
                                            killer: player_eid,
                                        },
                                    ));
                                    handle_npc_loot(world, pid, target, messages);
                                    crate::quest::on_kill(world, pid, npc_id_copy);
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
                            if let Some(dmg) = npc_attack_player(npc, player) {
                                messages.push((
                                    pid,
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
                let Some(player_eid) = player_eid else { continue };
                if let (Some(player), Some(npc)) =
                    (world.players.get_mut(&pid), world.npcs.get_mut(&target))
                {
                    let npc_dead = if let Some(dmg) =
                        cast_spell(player, npc, spell.max_hit, spell.xp)
                    {
                        messages.push((
                            pid,
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
    messages: &mut Vec<(PlayerId, ServerMessage)>,
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
        messages.push((player_id, ServerMessage::LootSpawn { items: loot }));
    }
    if let Some(npc) = world.npcs.get_mut(&npc_entity) {
        npc.respawn_ticks = def.respawn_ticks;
    }
}

fn respawn_player(
    world: &mut GameWorld,
    player_id: PlayerId,
    messages: &mut Vec<(PlayerId, ServerMessage)>,
) {
    let spawn = world
        .content
        .regions
        .first()
        .map(|r| r.spawn)
        .unwrap_or(TilePos::new(5, 5));
    if let Some(player) = world.players.get_mut(&player_id) {
        messages.push((
            player_id,
            ServerMessage::Death {
                entity: player.entity_id,
                killer: None,
            },
        ));
        player.position = spawn;
        player.hp = player.max_hp;
        player.action = PlayerAction::Idle;
        player.combat_target = None;
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

fn tick_minigame(world: &mut GameWorld, messages: &mut Vec<(PlayerId, ServerMessage)>) {
    crate::minigame::tick(world, messages);
}

fn tick_boss(world: &mut GameWorld, messages: &mut Vec<(PlayerId, ServerMessage)>) {
    crate::minigame::tick_boss(world, messages);
}

fn tick_economy(world: &mut GameWorld) {
    crate::economy::match_offers(world);
}

fn tick_ledger_contracts(world: &mut GameWorld, messages: &mut Vec<(PlayerId, ServerMessage)>) {
    for player in world.players.values() {
        if let Some(contract) = world.economy.ledger_contracts.get(&player.id) {
            messages.push((
                player.id,
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
