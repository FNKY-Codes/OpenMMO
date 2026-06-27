use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use openmmo_common::{EntityId, RegionDef, TilePos, TICK_MS};

use crate::entity_bounds;

#[derive(Debug, Clone, Copy)]
struct Entry {
    from: TilePos,
    to: TilePos,
    started_at: Instant,
    facing_yaw: f32,
}

#[derive(Debug, Default)]
pub struct EntityMovementInterp {
    entries: HashMap<EntityId, Entry>,
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn tile_world_xz(tile: TilePos) -> (f32, f32) {
    (tile.x as f32 + 0.5, tile.y as f32 + 0.5)
}

fn progress(started_at: Instant, now: Instant) -> f32 {
    let elapsed = now.saturating_duration_since(started_at);
    let tick = Duration::from_millis(TICK_MS);
    if tick.is_zero() {
        return 1.0;
    }
    (elapsed.as_secs_f32() / tick.as_secs_f32()).clamp(0.0, 1.0)
}

fn tile_movement_yaw(from: TilePos, to: TilePos) -> Option<f32> {
    let dx = (to.x - from.x) as f32;
    let dz = (to.y - from.y) as f32;
    if dx.abs() < f32::EPSILON && dz.abs() < f32::EPSILON {
        None
    } else {
        Some(dx.atan2(dz))
    }
}

impl EntityMovementInterp {
    pub fn on_position_change(&mut self, id: EntityId, new_pos: TilePos, now: Instant) {
        let entry = match self.entries.get(&id) {
            Some(existing) => {
                let old = existing.to;
                if old == new_pos {
                    return;
                }
                let facing_yaw = tile_movement_yaw(old, new_pos).unwrap_or(existing.facing_yaw);
                if old.chebyshev_distance(&new_pos) > 1 {
                    Entry {
                        from: new_pos,
                        to: new_pos,
                        started_at: now,
                        facing_yaw,
                    }
                } else {
                    Entry {
                        from: old,
                        to: new_pos,
                        started_at: now,
                        facing_yaw,
                    }
                }
            }
            None => Entry {
                from: new_pos,
                to: new_pos,
                started_at: now,
                facing_yaw: 0.0,
            },
        };
        self.entries.insert(id, entry);
    }

    pub fn remove(&mut self, id: EntityId) {
        self.entries.remove(&id);
    }

    pub fn prune(&mut self, live_ids: &HashSet<EntityId>) {
        self.entries.retain(|id, _| live_ids.contains(id));
    }

    pub fn visual_center(
        &self,
        id: EntityId,
        now: Instant,
        region: Option<&RegionDef>,
    ) -> Option<[f32; 3]> {
        let entry = self.entries.get(&id)?;
        let t = progress(entry.started_at, now);
        let (from_x, from_z) = tile_world_xz(entry.from);
        let (to_x, to_z) = tile_world_xz(entry.to);
        let cx = lerp_f32(from_x, to_x, t);
        let cz = lerp_f32(from_z, to_z, t);
        let from_y = entity_bounds::tile_surface_height(entry.from, region);
        let to_y = entity_bounds::tile_surface_height(entry.to, region);
        let surface_y = lerp_f32(from_y, to_y, t);
        Some([cx, surface_y, cz])
    }

    pub fn is_moving(&self, id: EntityId, now: Instant) -> bool {
        self.entries.get(&id).is_some_and(|entry| {
            entry.from != entry.to && progress(entry.started_at, now) < 1.0
        })
    }

    pub fn visual_facing_yaw(&self, id: EntityId, now: Instant) -> Option<f32> {
        let entry = self.entries.get(&id)?;
        if entry.from != entry.to && progress(entry.started_at, now) < 1.0 {
            tile_movement_yaw(entry.from, entry.to).or(Some(entry.facing_yaw))
        } else {
            Some(entry.facing_yaw)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmmo_common::EntityId;

    fn now() -> Instant {
        Instant::now()
    }

    #[test]
    fn first_sighting_snaps_to_tile_center() {
        let mut interp = EntityMovementInterp::default();
        let id = EntityId(1);
        let pos = TilePos::new(3, 4);
        interp.on_position_change(id, pos, now());

        let center = interp.visual_center(id, now(), None).unwrap();
        assert!((center[0] - 3.5).abs() < f32::EPSILON);
        assert!((center[2] - 4.5).abs() < f32::EPSILON);
    }

    #[test]
    fn single_tile_move_interpolates_halfway() {
        let mut interp = EntityMovementInterp::default();
        let id = EntityId(1);
        let start = now();
        interp.on_position_change(id, TilePos::new(0, 0), start);
        interp.on_position_change(id, TilePos::new(1, 0), start);

        let mid = start + Duration::from_millis(TICK_MS / 2);
        let center = interp.visual_center(id, mid, None).unwrap();
        assert!((center[0] - 1.0).abs() < 0.01);
        assert!((center[2] - 0.5).abs() < 0.01);
    }

    #[test]
    fn teleport_snaps_without_slide() {
        let mut interp = EntityMovementInterp::default();
        let id = EntityId(1);
        let start = now();
        interp.on_position_change(id, TilePos::new(0, 0), start);
        interp.on_position_change(id, TilePos::new(5, 5), start);

        let mid = start + Duration::from_millis(TICK_MS / 2);
        let center = interp.visual_center(id, mid, None).unwrap();
        assert!((center[0] - 5.5).abs() < f32::EPSILON);
        assert!((center[2] - 5.5).abs() < f32::EPSILON);
    }

    #[test]
    fn movement_sets_facing_yaw() {
        let mut interp = EntityMovementInterp::default();
        let id = EntityId(1);
        let start = now();
        interp.on_position_change(id, TilePos::new(0, 0), start);
        interp.on_position_change(id, TilePos::new(1, 0), start);

        let yaw = interp.visual_facing_yaw(id, start).unwrap();
        assert!((yaw - std::f32::consts::FRAC_PI_2).abs() < 0.01);
    }

    #[test]
    fn progress_clamps_after_tick_duration() {
        let mut interp = EntityMovementInterp::default();
        let id = EntityId(1);
        let start = now();
        interp.on_position_change(id, TilePos::new(0, 0), start);
        interp.on_position_change(id, TilePos::new(1, 0), start);

        let late = start + Duration::from_millis(TICK_MS * 2);
        let center = interp.visual_center(id, late, None).unwrap();
        assert!((center[0] - 1.5).abs() < f32::EPSILON);
    }
}
