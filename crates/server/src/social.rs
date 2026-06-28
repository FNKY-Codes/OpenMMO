use std::collections::HashMap;

use openmmo_common::{PlayerId, RegionId};
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
) -> Vec<(PlayerId, ServerMessage)> {
    let from = world
        .players
        .get(&player_id)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    let recipient = world.players.values().find(|p| p.name == to).map(|p| p.id);
    let msg = ServerMessage::ChatMessage {
        channel: ChatChannel::Private,
        from: from.clone(),
        message: message.clone(),
    };
    if let Some(target_id) = recipient {
        vec![
            (target_id, msg),
            (player_id, msg_for_sender(&from, &to, &message)),
        ]
    } else {
        vec![(
            player_id,
            ServerMessage::Error {
                message: format!("Player '{to}' not found"),
            },
        )]
    }
}

fn msg_for_sender(from: &str, to: &str, message: &str) -> ServerMessage {
    ServerMessage::ChatMessage {
        channel: ChatChannel::Private,
        from: from.to_string(),
        message: format!("To {to}: {message}"),
    }
}

pub fn route_chat(
    world: &GameWorld,
    sender_id: PlayerId,
    channel: ChatChannel,
    from: String,
    message: String,
) -> Vec<(PlayerId, ServerMessage)> {
    let msg = ServerMessage::ChatMessage {
        channel,
        from,
        message,
    };
    match channel {
        ChatChannel::Private => Vec::new(),
        ChatChannel::Global => world
            .players
            .keys()
            .copied()
            .map(|pid| (pid, msg.clone()))
            .collect(),
        ChatChannel::Local => {
            let sender_region = world
                .players
                .get(&sender_id)
                .map(|p| p.region_id)
                .unwrap_or(RegionId(1));
            world
                .players
                .iter()
                .filter(|(_, p)| p.region_id == sender_region)
                .map(|(&pid, _)| (pid, msg.clone()))
                .collect()
        }
        ChatChannel::Clan => {
            let sender_name = world
                .players
                .get(&sender_id)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            let members = world
                .social
                .clan_members
                .get(&sender_name)
                .cloned()
                .unwrap_or_default();
            if members.is_empty() {
                return vec![(sender_id, msg)];
            }
            world
                .players
                .values()
                .filter(|p| members.contains(&p.name))
                .map(|p| (p.id, msg.clone()))
                .collect()
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use openmmo_common::{PlayerAction, PlayerState, RegionId, SkillBook, TilePos};
    use uuid::Uuid;

    fn test_player(id: u128, name: &str, region_id: RegionId) -> PlayerState {
        let pid = PlayerId(Uuid::from_u128(id));
        PlayerState {
            id: pid,
            name: name.to_string(),
            entity_id: openmmo_common::EntityId(id as u32),
            position: TilePos::new(0, 0),
            hp: 10,
            max_hp: 10,
            skills: SkillBook::new_mvp(),
            inventory: openmmo_common::Inventory::new(28),
            bank: openmmo_common::Inventory::new(200),
            equipment: Default::default(),
            combat_target: None,
            action: PlayerAction::Idle,
            quest_progress: Default::default(),
            quest_counters: Default::default(),
            friends: Vec::new(),
            ledger_rank: 0,
            ledger_points: 0,
            specialization: Default::default(),
            is_moderator: false,
            last_position: TilePos::new(0, 0),
            ticks_stationary: 1,
            region_id,
        }
    }

    #[test]
    fn local_chat_only_same_region() {
        let mut world = GameWorld::new(openmmo_common::ContentPack::default());
        let a = PlayerId(Uuid::from_u128(1));
        let b = PlayerId(Uuid::from_u128(2));
        let c = PlayerId(Uuid::from_u128(3));
        world.players.insert(a, test_player(1, "Alice", RegionId(1)));
        world.players.insert(b, test_player(2, "Bob", RegionId(1)));
        world.players.insert(c, test_player(3, "Carol", RegionId(2)));

        let routed = route_chat(
            &world,
            a,
            ChatChannel::Local,
            "Alice".into(),
            "hello".into(),
        );
        let recipients: Vec<_> = routed.iter().map(|(pid, _)| *pid).collect();
        assert!(recipients.contains(&a));
        assert!(recipients.contains(&b));
        assert!(!recipients.contains(&c));
    }

    #[test]
    fn global_chat_all_players() {
        let mut world = GameWorld::new(openmmo_common::ContentPack::default());
        let a = PlayerId(Uuid::from_u128(1));
        let b = PlayerId(Uuid::from_u128(2));
        world.players.insert(a, test_player(1, "Alice", RegionId(1)));
        world.players.insert(b, test_player(2, "Bob", RegionId(2)));

        let routed = route_chat(
            &world,
            a,
            ChatChannel::Global,
            "Alice".into(),
            "hi all".into(),
        );
        assert_eq!(routed.len(), 2);
    }

    #[test]
    fn clan_chat_only_clan_members() {
        let mut world = GameWorld::new(openmmo_common::ContentPack::default());
        let a = PlayerId(Uuid::from_u128(1));
        let b = PlayerId(Uuid::from_u128(2));
        let c = PlayerId(Uuid::from_u128(3));
        world.players.insert(a, test_player(1, "Alice", RegionId(1)));
        world.players.insert(b, test_player(2, "Bob", RegionId(1)));
        world.players.insert(c, test_player(3, "Carol", RegionId(1)));
        world
            .social
            .clan_members
            .insert("Alice".into(), vec!["Alice".into(), "Bob".into()]);

        let routed = route_chat(
            &world,
            a,
            ChatChannel::Clan,
            "Alice".into(),
            "clan msg".into(),
        );
        let recipients: Vec<_> = routed.iter().map(|(pid, _)| *pid).collect();
        assert!(recipients.contains(&a));
        assert!(recipients.contains(&b));
        assert!(!recipients.contains(&c));
    }
}
