use openmmo_common::TilePos;

use crate::camera::Camera;

#[derive(Debug, Clone, Default)]
pub struct InputState {
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub left_clicked: bool,
    pub right_clicked: bool,
    pub middle_dragging: bool,
    pub last_drag_x: f32,
    pub last_drag_y: f32,
    pub scroll_delta: f32,
}

impl InputState {
    pub fn entity_under_cursor(
        &self,
        camera: &Camera,
        width: u32,
        height: u32,
        entities: &[openmmo_common::WorldEntity],
        region: Option<&openmmo_common::RegionDef>,
    ) -> Option<openmmo_common::EntityId> {
        camera.pick_entity(
            self.mouse_x,
            self.mouse_y,
            width,
            height,
            entities,
            region,
        )
    }

    pub fn tile_under_cursor(&self, camera: &Camera, width: u32, height: u32) -> Option<TilePos> {
        camera.pick_tile(self.mouse_x, self.mouse_y, width, height)
    }

    pub fn begin_drag(&mut self) {
        self.last_drag_x = self.mouse_x;
        self.last_drag_y = self.mouse_y;
    }

    pub fn drag_delta(&mut self) -> (f32, f32) {
        let dx = self.mouse_x - self.last_drag_x;
        let dy = self.mouse_y - self.last_drag_y;
        self.last_drag_x = self.mouse_x;
        self.last_drag_y = self.mouse_y;
        (dx, dy)
    }

    pub fn take_scroll(&mut self) -> f32 {
        let delta = self.scroll_delta;
        self.scroll_delta = 0.0;
        delta
    }
}
