use openmmo_common::{ItemId, PlayerId, PlayerState, TilePos};
use openmmo_protocol::ModCommand;

use crate::state::GameWorld;

const MAX_WALK_PER_TICK: i32 = 1;

pub fn validate_walk(player: &PlayerState, target: TilePos) -> bool {
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

pub fn handle_mod_command(
    world: &mut GameWorld,
    _moderator: PlayerId,
    command: ModCommand,
) -> Vec<openmmo_protocol::ServerMessage> {
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

impl PluginApi {
    pub fn on_tick(_world: &mut GameWorld) {
        // WASM/Lua plugin hook point (Phase 4)
    }
}
