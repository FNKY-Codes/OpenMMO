use openmmo_common::{ItemId, PlayerId, TilePos};
use openmmo_protocol::ModCommand;

use crate::state::GameWorld;

const MAX_WALK_PER_TICK: i32 = 1;

pub fn validate_username(username: &str) -> bool {
    !username.is_empty() && username.len() <= 32 && username.chars().all(|c| c.is_alphanumeric() || c == '_')
}

pub fn validate_walk(player: &openmmo_common::PlayerState, target: TilePos) -> bool {
    player.position.chebyshev_distance(&target) <= 50
}

pub fn validate_chat(message: &str) -> bool {
    !message.is_empty() && message.len() <= 200 && !message.contains('\0')
}

pub fn validate_teleport(from: TilePos, to: TilePos) -> bool {
    from.chebyshev_distance(&to) <= 100
}

pub fn detect_speed_hack(old: TilePos, new: TilePos, ticks: u64) -> bool {
    let max_dist = (ticks as i32 + 1) * MAX_WALK_PER_TICK;
    old.chebyshev_distance(&new) > max_dist
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmmo_common::TilePos;

    #[test]
    fn detect_speed_hack_flags_large_jumps() {
        let old = TilePos::new(0, 0);
        let new = TilePos::new(5, 0);
        assert!(detect_speed_hack(old, new, 1));
    }

    #[test]
    fn detect_speed_hack_allows_single_tile_step() {
        let old = TilePos::new(0, 0);
        let new = TilePos::new(1, 0);
        assert!(!detect_speed_hack(old, new, 1));
    }
}

pub fn handle_mod_command(
    world: &mut GameWorld,
    moderator: PlayerId,
    command: ModCommand,
) -> Vec<openmmo_protocol::ServerMessage> {
    let is_mod = world
        .players
        .get(&moderator)
        .is_some_and(|p| p.is_moderator);
    if !is_mod {
        return vec![openmmo_protocol::ServerMessage::Error {
            message: "Moderator permission required".into(),
        }];
    }
    match command {
        ModCommand::Kick { player } => {
            world.remove_player(player);
            world.audit(&format!("Moderator kicked {player:?}"));
        }
        ModCommand::Ban { player } => {
            world.remove_player(player);
            world.audit(&format!("Moderator banned {player:?}"));
        }
        ModCommand::Teleport { player, target } => {
            if let Some(p) = world.players.get_mut(&player) {
                if validate_teleport(p.position, target) {
                    p.position = target;
                }
            }
            world.audit(&format!("Moderator teleported {player:?}"));
        }
        ModCommand::SpawnItem {
            player,
            item_id,
            qty,
        } => {
            if let Some(p) = world.players.get_mut(&player) {
                let stackable = world
                    .content
                    .item(item_id)
                    .map(|i| i.stackable)
                    .unwrap_or(true);
                let _ = p.inventory.add_item(item_id, qty, stackable);
            }
            world.audit(&format!(
                "Moderator spawned item {} for {:?}",
                item_id.0, player
            ));
        }
    }
    Vec::new()
}

pub struct PluginApi;

pub struct PluginRegistry {
    pub hooks: Vec<String>,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self {
            hooks: vec!["on_tick".into()],
        }
    }
}

impl PluginApi {
    pub fn on_tick(world: &mut GameWorld) {
        // Plugin hook point: WASM/Lua sandbox will register scripts here.
        let _ = world.tick;
    }

    pub fn registry() -> PluginRegistry {
        PluginRegistry::default()
    }
}
