use openmmo_common::{EntityKind, NpcId, RegionDef, TilePos, WorldEntity};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, OnceLock};

use crate::math::Vec3;

static PLAYER_MODEL_DIMS: OnceLock<(f32, f32)> = OnceLock::new();
static NPC_MODEL_DIMS: LazyLock<Mutex<HashMap<NpcId, (f32, f32)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn set_player_model_dims(width: f32, height: f32) {
    let _ = PLAYER_MODEL_DIMS.set((width, height));
}

pub fn set_npc_model_dims(npc_id: NpcId, width: f32, height: f32) {
    if let Ok(mut dims) = NPC_MODEL_DIMS.lock() {
        dims.insert(npc_id, (width, height));
    }
}

pub fn entity_cube_dims(kind: &EntityKind) -> (f32, f32) {
    match kind {
        EntityKind::Player { .. } => {
            PLAYER_MODEL_DIMS.get().copied().unwrap_or((0.9, 1.8))
        }
        EntityKind::Npc { npc_id, .. } => NPC_MODEL_DIMS
            .lock()
            .ok()
            .and_then(|dims| dims.get(npc_id).copied())
            .unwrap_or((0.8, 1.6)),
        EntityKind::Boss { .. } => (1.4, 3.0),
        EntityKind::Object { .. } => (0.7, 1.2),
        EntityKind::GroundItem { .. } => (0.35, 0.35),
    }
}

pub fn tile_surface_height(tile: TilePos, region: Option<&RegionDef>) -> f32 {
    let tile_type = if let Some(region) = region {
        if tile.x >= 0
            && tile.y >= 0
            && (tile.x as u32) < region.width
            && (tile.y as u32) < region.height
        {
            let idx = (tile.y as u32 * region.width + tile.x as u32) as usize;
            region.tiles.get(idx).copied().unwrap_or(0)
        } else {
            0
        }
    } else {
        0
    };

    tile_type_height(tile_type, region.is_none())
}

fn tile_type_height(tile_type: u8, test_map: bool) -> f32 {
    if test_map {
        0.15
    } else {
        match tile_type {
            0 => 0.12,
            1 => 0.2,
            2 => 0.05,
            _ => 0.1,
        }
    }
}

pub fn entity_tile(entity: &WorldEntity) -> Option<TilePos> {
    match &entity.kind {
        EntityKind::Player { position, .. }
        | EntityKind::Npc { position, .. }
        | EntityKind::Boss { position, .. }
        | EntityKind::Object { position, .. }
        | EntityKind::GroundItem { position, .. } => Some(*position),
    }
}

pub fn entity_aabb(entity: &WorldEntity, region: Option<&RegionDef>) -> Option<(Vec3, Vec3)> {
    let tile = entity_tile(entity)?;
    let (width, height) = entity_cube_dims(&entity.kind);
    let cx = tile.x as f32 + 0.5;
    let cz = tile.y as f32 + 0.5;
    let surface_y = tile_surface_height(tile, region);
    let center = Vec3::new(cx, surface_y + height * 0.5, cz);
    let half = Vec3::new(width * 0.5, height * 0.5, width * 0.5);
    Some((center, half))
}
