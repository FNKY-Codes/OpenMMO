use openmmo_common::TilePos;

use crate::entity_bounds;
use crate::math::{self, Mat4, Vec3, DEFAULT_FOV_Y};

#[derive(Debug, Clone)]
pub struct Camera {
    pub target_x: f32,
    pub target_y: f32,
    pub target_z: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self::new()
    }
}

impl Camera {
    pub fn new() -> Self {
        Self {
            target_x: 32.0,
            target_y: 0.9,
            target_z: 32.0,
            yaw: 0.7,
            pitch: 0.55,
            distance: 45.0,
        }
    }

    pub fn center_on_tile(&mut self, tile: TilePos) {
        self.target_x = tile.x as f32 + 0.5;
        self.target_z = tile.y as f32 + 0.5;
        self.target_y = 0.9;
    }

    pub fn center_on_world(&mut self, x: f32, z: f32) {
        self.target_x = x;
        self.target_z = z;
        self.target_y = 0.9;
    }

    pub fn rotate(&mut self, dyaw: f32, dpitch: f32) {
        self.yaw += dyaw;
        self.pitch = (self.pitch + dpitch).clamp(0.15, 1.35);
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta).clamp(8.0, 120.0);
    }

    pub fn eye_position(&self) -> Vec3 {
        let horizontal = self.distance * self.pitch.cos();
        let x = self.target_x + horizontal * self.yaw.sin();
        let y = self.target_y + self.distance * self.pitch.sin();
        let z = self.target_z + horizontal * self.yaw.cos();
        Vec3::new(x, y, z)
    }

    pub fn target_position(&self) -> Vec3 {
        Vec3::new(self.target_x, self.target_y, self.target_z)
    }

    /// Horizontal direction the camera looks toward (radians, Y-up).
    pub fn facing_yaw(&self) -> f32 {
        let eye = self.eye_position();
        let target = self.target_position();
        let dx = target.x - eye.x;
        let dz = target.z - eye.z;
        if dx.abs() < 1e-6 && dz.abs() < 1e-6 {
            self.yaw + std::f32::consts::PI
        } else {
            dx.atan2(dz)
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at(
            self.eye_position(),
            self.target_position(),
            Vec3::new(0.0, 1.0, 0.0),
        )
    }

    pub fn projection_matrix(&self, width: u32, height: u32) -> Mat4 {
        let aspect = width.max(1) as f32 / height.max(1) as f32;
        Mat4::perspective(DEFAULT_FOV_Y, aspect, 0.1, 500.0)
    }

    pub fn view_projection(&self, width: u32, height: u32) -> Mat4 {
        self.projection_matrix(width, height)
            .mul(self.view_matrix())
    }

    pub fn inverse_view_projection(&self, width: u32, height: u32) -> Mat4 {
        self.view_projection(width, height)
            .inverse()
            .unwrap_or_else(Mat4::identity)
    }

    pub fn pick_tile(
        &self,
        mouse_x: f32,
        mouse_y: f32,
        width: u32,
        height: u32,
    ) -> Option<TilePos> {
        let inv_vp = self.inverse_view_projection(width, height);
        let (origin, dir) = math::screen_to_world_ray(
            mouse_x,
            mouse_y,
            width.max(1) as f32,
            height.max(1) as f32,
            inv_vp,
        );
        let hit = math::ray_plane_y_intersection(origin, dir)?;
        Some(TilePos::new(hit.x.floor() as i32, hit.z.floor() as i32))
    }

    pub fn pick_entity(
        &self,
        mouse_x: f32,
        mouse_y: f32,
        width: u32,
        height: u32,
        entities: &[openmmo_common::WorldEntity],
        region: Option<&openmmo_common::RegionDef>,
        local_player: Option<openmmo_common::PlayerId>,
    ) -> Option<openmmo_common::EntityId> {
        let inv_vp = self.inverse_view_projection(width, height);
        let (origin, dir) = math::screen_to_world_ray(
            mouse_x,
            mouse_y,
            width.max(1) as f32,
            height.max(1) as f32,
            inv_vp,
        );

        let mut best_mesh: Option<(openmmo_common::EntityId, f32)> = None;
        for entity in entities {
            if is_local_player(entity, local_player) {
                continue;
            }
            let (center, half) = entity_bounds::entity_aabb(entity, region)?;
            let Some(t) = math::ray_aabb_intersection(origin, dir, center, half) else {
                continue;
            };
            if best_mesh.is_none() || t < best_mesh.unwrap().1 {
                best_mesh = Some((entity.entity_id, t));
            }
        }
        if let Some((id, _)) = best_mesh {
            return Some(id);
        }

        pick_entity_on_tile(origin, dir, entities, local_player)
    }
}

fn is_local_player(
    entity: &openmmo_common::WorldEntity,
    local_player: Option<openmmo_common::PlayerId>,
) -> bool {
    use openmmo_common::EntityKind;
    match (&entity.kind, local_player) {
        (EntityKind::Player { player_id, .. }, Some(local)) => *player_id == local,
        _ => false,
    }
}

fn pick_entity_on_tile(
    origin: Vec3,
    dir: Vec3,
    entities: &[openmmo_common::WorldEntity],
    local_player: Option<openmmo_common::PlayerId>,
) -> Option<openmmo_common::EntityId> {
    let hit = math::ray_plane_y_intersection(origin, dir)?;
    let hit_tile = TilePos::new(hit.x.floor() as i32, hit.z.floor() as i32);
    let mut best: Option<(openmmo_common::EntityId, f32)> = None;
    for entity in entities {
        if is_local_player(entity, local_player) {
            continue;
        }
        let pos = entity_bounds::entity_tile(entity)?;
        if pos != hit_tile {
            continue;
        }
        let cx = pos.x as f32 + 0.5;
        let cz = pos.y as f32 + 0.5;
        let dx = hit.x - cx;
        let dz = hit.z - cz;
        let dist = (dx * dx + dz * dz).sqrt();
        if best.is_none() || dist < best.unwrap().1 {
            best = Some((entity.entity_id, dist));
        }
    }
    best.map(|(id, _)| id)
}

#[cfg(test)]
mod tests {
    use openmmo_common::{EntityId, EntityKind, PlayerId, TilePos, WorldEntity};

    use super::Camera;

    fn object_at(tile: TilePos) -> WorldEntity {
        WorldEntity {
            entity_id: EntityId(1),
            kind: EntityKind::Object {
                object_id: openmmo_common::ObjectId(1),
                position: tile,
            },
        }
    }

    fn local_player_at(tile: TilePos, player_id: PlayerId) -> WorldEntity {
        WorldEntity {
            entity_id: EntityId(2),
            kind: EntityKind::Player {
                player_id,
                name: "Hero".into(),
                position: tile,
                hp: 10,
                max_hp: 10,
            },
        }
    }

    #[test]
    fn pick_entity_hits_object_mesh_not_just_ground_tile() {
        let camera = Camera {
            target_x: 5.5,
            target_y: 0.9,
            target_z: 5.5,
            yaw: 0.7,
            pitch: 0.55,
            distance: 12.0,
        };
        let object_tile = TilePos::new(5, 5);
        let entities = vec![object_at(object_tile)];
        let width = 800;
        let height = 600;
        let (center, half) = crate::entity_bounds::entity_aabb(&entities[0], None).unwrap();

        let mut found = false;
        'search: for y in (0..height).step_by(8) {
            for x in (0..width).step_by(8) {
                let inv_vp = camera.inverse_view_projection(width, height);
                let (origin, dir) = crate::math::screen_to_world_ray(
                    x as f32,
                    y as f32,
                    width as f32,
                    height as f32,
                    inv_vp,
                );
                let Some(t) = crate::math::ray_aabb_intersection(origin, dir, center, half) else {
                    continue;
                };
                if t <= 0.0 {
                    continue;
                }
                let ground_hit = crate::math::ray_plane_y_intersection(origin, dir).expect("ground hit");
                let ground_tile =
                    TilePos::new(ground_hit.x.floor() as i32, ground_hit.z.floor() as i32);
                if ground_tile == object_tile {
                    continue;
                }

                let picked = camera
                    .pick_entity(x as f32, y as f32, width, height, &entities, None, None)
                    .expect("object should be picked from its mesh");
                assert_eq!(picked, EntityId(1));
                found = true;
                break 'search;
            }
        }
        assert!(
            found,
            "expected a screen point that hits the object mesh but projects to a different ground tile"
        );
    }

    #[test]
    fn pick_entity_ignores_local_player_blocking_object() {
        let camera = Camera::new();
        let player_tile = TilePos::new(32, 32);
        let object_tile = TilePos::new(33, 32);
        let local_id = PlayerId::new();
        let entities = vec![local_player_at(player_tile, local_id), object_at(object_tile)];
        let width = 1280;
        let height = 720;

        let mut found = false;
        'search: for y in (0..height).step_by(8) {
            for x in (0..width).step_by(8) {
                let picked = camera.pick_entity(
                    x as f32,
                    y as f32,
                    width,
                    height,
                    &entities,
                    None,
                    Some(local_id),
                );
                if picked == Some(EntityId(1)) {
                    found = true;
                    break 'search;
                }
            }
        }
        assert!(
            found,
            "object should be pickable even when local player is in the scene"
        );
    }

    #[test]
    fn pick_entity_falls_back_to_tile_under_cursor() {
        let camera = Camera {
            target_x: 10.5,
            target_y: 0.9,
            target_z: 10.5,
            yaw: 0.7,
            pitch: 0.55,
            distance: 12.0,
        };
        let object_tile = TilePos::new(10, 10);
        let entities = vec![object_at(object_tile)];
        let width = 800;
        let height = 600;

        let mut found = false;
        'search: for y in (0..height).step_by(8) {
            for x in (0..width).step_by(8) {
                let inv_vp = camera.inverse_view_projection(width, height);
                let (origin, dir) = crate::math::screen_to_world_ray(
                    x as f32,
                    y as f32,
                    width as f32,
                    height as f32,
                    inv_vp,
                );
                let ground_hit = crate::math::ray_plane_y_intersection(origin, dir).unwrap();
                let ground_tile =
                    TilePos::new(ground_hit.x.floor() as i32, ground_hit.z.floor() as i32);
                if ground_tile != object_tile {
                    continue;
                }
                let (center, half) = crate::entity_bounds::entity_aabb(&entities[0], None).unwrap();
                if crate::math::ray_aabb_intersection(origin, dir, center, half).is_some() {
                    continue;
                }

                let picked = camera
                    .pick_entity(x as f32, y as f32, width, height, &entities, None, None)
                    .expect("tile fallback should pick object on clicked tile");
                assert_eq!(picked, EntityId(1));
                found = true;
                break 'search;
            }
        }
        assert!(found, "expected a tile click that misses mesh but hits object tile");
    }
}
