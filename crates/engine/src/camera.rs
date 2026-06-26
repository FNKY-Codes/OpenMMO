use openmmo_common::{TilePos, tile_to_screen};

#[derive(Debug, Clone, Default)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }

    pub fn center_on_tile(&mut self, tile: TilePos, screen_w: f32, screen_h: f32) {
        let (sx, sy) = tile_to_screen(tile, 0.0, 0.0);
        self.x = sx - screen_w / 2.0;
        self.y = sy - screen_h / 2.0;
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.x += dx;
        self.y += dy;
    }
}
