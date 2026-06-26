use openmmo_common::{BossState, EntityId, PlayerId, TilePos};
use openmmo_protocol::ServerMessage;

use crate::state::GameWorld;

#[derive(Default)]
pub struct MinigameState {
    pub lobbies: Vec<openmmo_common::MinigameLobby>,
    pub active_wave: u32,
    pub arena_players: Vec<PlayerId>,
}

pub fn handle_join(
    world: &mut GameWorld,
    player_id: PlayerId,
    minigame_id: String,
) -> Vec<ServerMessage> {
    if minigame_id == "arena" {
        if !world.minigames.arena_players.contains(&player_id) {
            world.minigames.arena_players.push(player_id);
        }
        if world.minigames.arena_players.len() >= 1 && world.boss.is_none() {
            spawn_boss(world);
        }
        return vec![ServerMessage::MinigameStart {
            minigame_id,
            wave: world.minigames.active_wave,
        }];
    }
    Vec::new()
}

pub fn spawn_boss(world: &mut GameWorld) {
    let eid = world.alloc_entity();
    world.boss = Some(BossState {
        entity_id: eid,
        name: "Stone Guardian".into(),
        position: TilePos::new(25, 25),
        hp: 100,
        max_hp: 100,
        phase: 1,
        mechanics_active: vec!["ground_slam".into()],
    });
    world.minigames.active_wave = 1;
}

pub fn tick(world: &mut GameWorld, messages: &mut Vec<(PlayerId, ServerMessage)>) {
    if world.tick % 20 == 0 && !world.minigames.arena_players.is_empty() {
        world.minigames.active_wave += 1;
        for pid in &world.minigames.arena_players.clone() {
            messages.push((
                *pid,
                ServerMessage::MinigameStart {
                    minigame_id: "arena".into(),
                    wave: world.minigames.active_wave,
                },
            ));
        }
    }
}

pub fn tick_boss(world: &mut GameWorld, messages: &mut Vec<(PlayerId, ServerMessage)>) {
    let Some(boss) = world.boss.as_mut() else {
        return;
    };
    if world.tick % 5 == 0 {
        if boss.phase == 1 && boss.hp < boss.max_hp / 2 {
            boss.phase = 2;
            boss.mechanics_active.push("shockwave".into());
        }
        for pid in world.players.keys().copied().collect::<Vec<_>>() {
            if let Some(player) = world.players.get_mut(&pid) {
                if player.position.chebyshev_distance(&boss.position) <= 3 {
                    player.hp = player.hp.saturating_sub(2);
                }
            }
            messages.push((pid, ServerMessage::BossUpdate { boss: boss.clone() }));
        }
    }
}
