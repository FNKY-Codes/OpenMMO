use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use openmmo_common::{EntityId, NpcFootprint, RegionDef, TilePos, TICK_MS};

use crate::entity_bounds;

#[derive(Debug, Clone, Copy)]
struct Entry {
    from_world: [f32; 3],
    to_world: [f32; 3],
    from_tile: TilePos,
    to_tile: TilePos,
    /// Wall-clock instant when this tile step began on the client.
    segment_start: Instant,
    facing_yaw: f32,
}

#[derive(Debug, Default)]
pub struct EntityMovementInterp {
    entries: HashMap<EntityId, Entry>,
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        lerp_f32(a[0], b[0], t),
        lerp_f32(a[1], b[1], t),
        lerp_f32(a[2], b[2], t),
    ]
}

fn progress(segment_start: Instant, now: Instant) -> f32 {
    let elapsed = now.saturating_duration_since(segment_start);
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
    pub fn seed_position(
        &mut self,
        id: EntityId,
        pos: TilePos,
        region: Option<&RegionDef>,
        footprint: Option<NpcFootprint>,
    ) {
        let fp = footprint.unwrap_or_default();
        let world = footprint_world_center(pos, region, fp);
        let facing_yaw = self
            .entries
            .get(&id)
            .filter(|e| e.to_tile == pos)
            .map(|e| e.facing_yaw)
            .unwrap_or(0.0);
        self.entries.insert(
            id,
            Entry {
                from_world: world,
                to_world: world,
                from_tile: pos,
                to_tile: pos,
                segment_start: Instant::now(),
                facing_yaw,
            },
        );
    }

    pub fn on_position_change(
        &mut self,
        id: EntityId,
        new_pos: TilePos,
        now: Instant,
        region: Option<&RegionDef>,
        footprint: Option<NpcFootprint>,
    ) {
        let fp = footprint.unwrap_or_default();
        let new_world = footprint_world_center(new_pos, region, fp);

        let entry = match self.entries.get(&id) {
            Some(existing) => {
                if existing.to_tile == new_pos {
                    return;
                }
                let facing_yaw = tile_movement_yaw(existing.to_tile, new_pos)
                    .unwrap_or(existing.facing_yaw);

                if existing.to_tile.chebyshev_distance(&new_pos) > 1 {
                    Entry {
                        from_world: new_world,
                        to_world: new_world,
                        from_tile: new_pos,
                        to_tile: new_pos,
                        segment_start: now,
                        facing_yaw,
                    }
                } else {
                    let t = progress(existing.segment_start, now);
                    let from_world = if existing.from_tile != existing.to_tile && t < 1.0 {
                        lerp3(existing.from_world, existing.to_world, t)
                    } else {
                        existing.to_world
                    };
                    Entry {
                        from_world,
                        to_world: new_world,
                        from_tile: existing.to_tile,
                        to_tile: new_pos,
                        segment_start: now,
                        facing_yaw,
                    }
                }
            }
            None => Entry {
                from_world: new_world,
                to_world: new_world,
                from_tile: new_pos,
                to_tile: new_pos,
                segment_start: now,
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
        footprint: Option<NpcFootprint>,
    ) -> Option<[f32; 3]> {
        let entry = self.entries.get(&id)?;
        let _ = (region, footprint);
        let t = progress(entry.segment_start, now);
        Some(lerp3(entry.from_world, entry.to_world, t))
    }

    pub fn visual_facing_yaw(&self, id: EntityId, now: Instant) -> Option<f32> {
        let entry = self.entries.get(&id)?;
        if entry.from_tile != entry.to_tile && progress(entry.segment_start, now) < 1.0 {
            tile_movement_yaw(entry.from_tile, entry.to_tile).or(Some(entry.facing_yaw))
        } else {
            Some(entry.facing_yaw)
        }
    }

    /// Rotate an entity to face a target tile (e.g. when attacking while stationary).
    pub fn face_toward(&mut self, id: EntityId, from: TilePos, toward: TilePos) {
        let Some(yaw) = tile_movement_yaw(from, toward) else {
            return;
        };
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.facing_yaw = yaw;
        }
    }

    pub fn is_moving(&self, id: EntityId, now: Instant) -> bool {
        self.entries.get(&id).is_some_and(|entry| {
            entry.from_tile != entry.to_tile && progress(entry.segment_start, now) < 1.0
        })
    }

    pub fn movement_progress(&self, id: EntityId, now: Instant) -> Option<f32> {
        let entry = self.entries.get(&id)?;
        if entry.from_tile == entry.to_tile {
            return None;
        }
        Some(progress(entry.segment_start, now))
    }
}

fn footprint_world_center(
    tile: TilePos,
    region: Option<&RegionDef>,
    footprint: NpcFootprint,
) -> [f32; 3] {
    let surface_y = entity_bounds::tile_surface_height(tile, region);
    footprint.world_center(tile, surface_y)
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
        let t = now();
        interp.on_position_change(id, pos, t, None, None);

        let center = interp.visual_center(id, t, None, None).unwrap();
        assert!((center[0] - 3.5).abs() < f32::EPSILON);
        assert!((center[2] - 4.5).abs() < f32::EPSILON);
    }

    #[test]
    fn single_tile_move_interpolates_halfway() {
        let mut interp = EntityMovementInterp::default();
        let id = EntityId(1);
        let start = now();
        interp.on_position_change(id, TilePos::new(0, 0), start, None, None);
        interp.on_position_change(id, TilePos::new(1, 0), start, None, None);

        let mid = start + Duration::from_millis(TICK_MS / 2);
        let center = interp.visual_center(id, mid, None, None).unwrap();
        assert!((center[0] - 1.0).abs() < 0.01);
        assert!((center[2] - 0.5).abs() < 0.01);
    }

    #[test]
    fn teleport_snaps_without_slide() {
        let mut interp = EntityMovementInterp::default();
        let id = EntityId(1);
        let start = now();
        interp.on_position_change(id, TilePos::new(0, 0), start, None, None);
        interp.on_position_change(id, TilePos::new(5, 5), start, None, None);

        let mid = start + Duration::from_millis(TICK_MS / 2);
        let center = interp.visual_center(id, mid, None, None).unwrap();
        assert!((center[0] - 5.5).abs() < f32::EPSILON);
        assert!((center[2] - 5.5).abs() < f32::EPSILON);
    }

    #[test]
    fn movement_sets_facing_yaw() {
        let mut interp = EntityMovementInterp::default();
        let id = EntityId(1);
        let start = now();
        interp.on_position_change(id, TilePos::new(0, 0), start, None, None);
        interp.on_position_change(id, TilePos::new(1, 0), start, None, None);

        let yaw = interp.visual_facing_yaw(id, start).unwrap();
        assert!((yaw - std::f32::consts::FRAC_PI_2).abs() < 0.01);
    }

    #[test]
    fn seed_position_preserves_facing_at_same_tile() {
        let mut interp = EntityMovementInterp::default();
        let id = EntityId(1);
        let start = now();
        interp.on_position_change(id, TilePos::new(0, 0), start, None, None);
        interp.face_toward(id, TilePos::new(0, 0), TilePos::new(1, 0));

        let before = interp.visual_facing_yaw(id, start).unwrap();
        interp.seed_position(id, TilePos::new(0, 0), None, None);
        let after = interp.visual_facing_yaw(id, start).unwrap();
        assert!((after - before).abs() < 0.01);
    }

    #[test]
    fn face_toward_updates_stationary_yaw() {
        let mut interp = EntityMovementInterp::default();
        let id = EntityId(1);
        let start = now();
        interp.on_position_change(id, TilePos::new(0, 0), start, None, None);
        interp.face_toward(id, TilePos::new(0, 0), TilePos::new(1, 0));

        let yaw = interp.visual_facing_yaw(id, start).unwrap();
        assert!((yaw - std::f32::consts::FRAC_PI_2).abs() < 0.01);
    }

    #[test]
    fn progress_clamps_after_tick_duration() {
        let mut interp = EntityMovementInterp::default();
        let id = EntityId(1);
        let start = now();
        interp.on_position_change(id, TilePos::new(0, 0), start, None, None);
        interp.on_position_change(id, TilePos::new(1, 0), start, None, None);

        let late = start + Duration::from_millis(TICK_MS * 2);
        let center = interp.visual_center(id, late, None, None).unwrap();
        assert!((center[0] - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn chained_update_preserves_visual_position() {
        let mut interp = EntityMovementInterp::default();
        let id = EntityId(1);
        let start = now();
        interp.on_position_change(id, TilePos::new(0, 0), start, None, None);
        interp.on_position_change(id, TilePos::new(1, 0), start, None, None);

        let mid = start + Duration::from_millis(TICK_MS / 2);
        let before = interp.visual_center(id, mid, None, None).unwrap();
        interp.on_position_change(id, TilePos::new(2, 0), mid, None, None);

        let after = interp.visual_center(id, mid, None, None).unwrap();
        assert!((after[0] - before[0]).abs() < 0.01);
        assert!((after[2] - before[2]).abs() < 0.01);
    }

    #[test]
    fn simulated_walk_monotonic_in_move_direction() {
        let mut interp = EntityMovementInterp::default();
        let id = EntityId(1);
        let mut t = now();
        interp.on_position_change(id, TilePos::new(0, 0), t, None, None);

        let mut positions = Vec::new();
        for frame in 0..120 {
            if frame > 0 && frame % 38 == 0 {
                let tile_x = (frame / 38) as i32;
                interp.on_position_change(
                    id,
                    TilePos::new(tile_x, 0),
                    t,
                    None,
                    None,
                );
            }
            let center = interp.visual_center(id, t, None, None).unwrap();
            positions.push(center[0]);
            t += Duration::from_millis(16);
        }

        for window in positions.windows(2) {
            assert!(
                window[1] + 0.001 >= window[0],
                "visual x regressed from {} to {}",
                window[0],
                window[1]
            );
        }
    }
}
