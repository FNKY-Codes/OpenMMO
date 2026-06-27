pub use openmmo_sdk::math::TilePos;

/// Screen/camera space to world tile (isometric projection inverse).
pub fn screen_to_tile(screen_x: f32, screen_y: f32, camera_x: f32, camera_y: f32) -> TilePos {
    let wx = (screen_x + camera_x) / 32.0;
    let wy = (screen_y + camera_y) / 16.0;
    let tile_x = ((wx + wy) / 2.0).floor() as i32;
    let tile_y = ((wy - wx) / 2.0).floor() as i32;
    TilePos::new(tile_x, tile_y)
}

/// World tile to isometric screen position.
pub fn tile_to_screen(tile: TilePos, camera_x: f32, camera_y: f32) -> (f32, f32) {
    let screen_x = (tile.x - tile.y) as f32 * 32.0 - camera_x;
    let screen_y = (tile.x + tile.y) as f32 * 16.0 - camera_y;
    (screen_x, screen_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_roundtrip() {
        let tile = TilePos::new(5, 3);
        let (sx, sy) = tile_to_screen(tile, 0.0, 0.0);
        let back = screen_to_tile(sx, sy, 0.0, 0.0);
        assert_eq!(back.x, tile.x);
        assert_eq!(back.y, tile.y);
    }
}
