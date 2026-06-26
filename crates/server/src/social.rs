use std::collections::HashMap;

use openmmo_common::PlayerId;
use openmmo_protocol::{ChatChannel, ServerMessage};

use crate::state::GameWorld;

#[derive(Default)]
pub struct SocialState {
    pub friend_graph: HashMap<String, Vec<String>>,
    pub mutes: HashMap<String, Vec<String>>,
    pub clan_members: HashMap<String, Vec<String>>,
}

pub fn handle_friend_add(
    world: &mut GameWorld,
    player_id: PlayerId,
    name: String,
) -> Vec<ServerMessage> {
    if let Some(player) = world.players.get_mut(&player_id) {
        if !player.friends.contains(&name) {
            player.friends.push(name.clone());
        }
        let player_name = player.name.clone();
        world
            .social
            .friend_graph
            .entry(player_name.clone())
            .or_default()
            .push(name);
        return vec![friends_update(world, &player_name)];
    }
    Vec::new()
}

pub fn handle_private_message(
    world: &GameWorld,
    player_id: PlayerId,
    to: String,
    message: String,
) -> Vec<ServerMessage> {
    let from = world
        .players
        .get(&player_id)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    vec![ServerMessage::ChatMessage {
        channel: ChatChannel::Private,
        from,
        message: format!("@{to}: {message}"),
    }]
}

pub fn broadcast_chat(
    world: &GameWorld,
    channel: ChatChannel,
    from: String,
    message: String,
) -> Vec<ServerMessage> {
    world.audit_log.len();
    vec![ServerMessage::ChatMessage {
        channel,
        from,
        message,
    }]
}

pub fn friends_update(world: &GameWorld, name: &str) -> ServerMessage {
    let friends = world
        .players
        .values()
        .find(|p| p.name == name)
        .map(|p| p.friends.clone())
        .unwrap_or_default();
    let online: Vec<String> = world.players.values().map(|p| p.name.clone()).collect();
    ServerMessage::FriendsUpdate { friends, online }
}
