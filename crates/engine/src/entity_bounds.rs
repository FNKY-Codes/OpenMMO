use openmmo_common::{
    ContentPack, EntityKind, NpcFootprint, NpcId, ObjectId, RegionDef, TilePos, WorldEntity,
};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, OnceLock};

use crate::math::Vec3;

static PLAYER_MODEL_DIMS: OnceLock<(f32, f32)> = OnceLock::new();
static NPC_MODEL_DIMS: LazyLock<Mutex<HashMap<NpcId, (f32, f32)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static OBJECT_DIMS: LazyLock<Mutex<HashMap<ObjectId, (f32, f32)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn set_player_model_dims(width: f32, height: f32) {
    let _ = PLAYER_MODEL_DIMS.set((width, height));
}

pub fn set_npc_model_dims(npc_id: NpcId, width: f32, height: f32) {
    if let Ok(mut dims) = NPC_MODEL_DIMS.lock() {
        dims.insert(npc_id, (width, height));
    }
}

/// Register hover/pick bounds for every object's prop so clicking a tree
/// hits the canopy, not a generic crate-sized box.
pub fn register_object_dims(content: &ContentPack) {
    if let Ok(mut dims) = OBJECT_DIMS.lock() {
        for def in &content.objects {
            dims.insert(def.id, crate::props::prop_dims(def.model_or_default()));
        }
    }
}

pub fn entity_cube_dims(kind: &EntityKind) -> (f32, f32) {
    match kind {
        EntityKind::Player { .. } => PLAYER_MODEL_DIMS.get().copied().unwrap_or((0.9, 1.8)),
        EntityKind::Npc { npc_id, .. } => NPC_MODEL_DIMS
            .lock()
            .ok()
            .and_then(|dims| dims.get(npc_id).copied())
            .unwrap_or((0.8, 1.6)),
        EntityKind::Boss { .. } => (1.4, 3.0),
        EntityKind::Object {
            object_id,
            depleted,
            ..
        } => {
            let full = OBJECT_DIMS
                .lock()
                .ok()
                .and_then(|dims| dims.get(object_id).copied())
                .unwrap_or((0.7, 1.2));
            if *depleted {
                (full.0.min(0.6), 0.4)
            } else {
                full
            }
        }
        EntityKind::GroundItem { .. } => (0.35, 0.35),
    }
}

pub fn tile_surface_height(tile: TilePos, region: Option<&RegionDef>) -> f32 {
    let Some(region) = region else {
        // Test map (no region loaded).
        return 0.15;
    };
    let byte = if tile.x >= 0
        && tile.y >= 0
        && (tile.x as u32) < region.width
        && (tile.y as u32) < region.height
    {
        let idx = (tile.y as u32 * region.width + tile.x as u32) as usize;
        region.tiles.get(idx).copied().unwrap_or(0)
    } else {
        0
    };
    crate::world_style::tile_height(byte, tile.x, tile.y)
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

pub fn entity_aabb(
    entity: &WorldEntity,
    region: Option<&RegionDef>,
    footprint: Option<NpcFootprint>,
) -> Option<(Vec3, Vec3)> {
    let tile = entity_tile(entity)?;
    let (width, height) = entity_cube_dims(&entity.kind);
    let surface_y = tile_surface_height(tile, region);
    match &entity.kind {
        EntityKind::Npc { .. } => {
            let fp = footprint.unwrap_or_default();
            let center = Vec3::new(
                tile.x as f32 + fp.width as f32 * 0.5,
                surface_y + height * 0.5,
                tile.y as f32 + fp.height as f32 * 0.5,
            );
            let half = Vec3::new(fp.width as f32 * 0.5, height * 0.5, fp.height as f32 * 0.5);
            Some((center, half))
        }
        _ => {
            let cx = tile.x as f32 + 0.5;
            let cz = tile.y as f32 + 0.5;
            let center = Vec3::new(cx, surface_y + height * 0.5, cz);
            let half = Vec3::new(width * 0.5, height * 0.5, width * 0.5);
            Some((center, half))
        }
    }
}
