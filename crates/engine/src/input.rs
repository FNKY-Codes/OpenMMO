use openmmo_common::{TilePos, screen_to_tile};

#[derive(Debug, Clone, Default)]
pub struct InputState {
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub left_clicked: bool,
    pub right_clicked: bool,
    pub middle_dragging: bool,
    pub last_drag_x: f32,
    pub last_drag_y: f32,
}

impl InputState {
    pub fn tile_under_cursor(&self, camera_x: f32, camera_y: f32) -> TilePos {
        screen_to_tile(self.mouse_x, self.mouse_y, camera_x, camera_y)
    }
}
