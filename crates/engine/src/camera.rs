use openmmo_common::TilePos;

use crate::math::{self, Mat4, Vec3, DEFAULT_FOV_Y};

#[derive(Debug, Clone)]
pub struct Camera {
    pub target_x: f32,
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
            target_z: 32.0,
            yaw: 0.7,
            pitch: 0.55,
            distance: 45.0,
        }
    }

    pub fn center_on_tile(&mut self, tile: TilePos) {
        self.target_x = tile.x as f32 + 0.5;
        self.target_z = tile.y as f32 + 0.5;
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
        let y = self.distance * self.pitch.sin();
        let z = self.target_z + horizontal * self.yaw.cos();
        Vec3::new(x, y, z)
    }

    pub fn target_position(&self) -> Vec3 {
        Vec3::new(self.target_x, 0.0, self.target_z)
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
        self.projection_matrix(width, height).mul(self.view_matrix())
    }

    pub fn inverse_view_projection(&self, width: u32, height: u32) -> Mat4 {
        self.view_projection(width, height)
            .inverse()
            .unwrap_or_else(Mat4::identity)
    }

    pub fn pick_tile(&self, mouse_x: f32, mouse_y: f32, width: u32, height: u32) -> Option<TilePos> {
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
}
