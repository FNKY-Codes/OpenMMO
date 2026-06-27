use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct TilePos {
    pub x: i32,
    pub y: i32,
    pub plane: u8,
}

impl TilePos {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y, plane: 0 }
    }

    pub fn manhattan_distance(&self, other: &TilePos) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }

    pub fn chebyshev_distance(&self, other: &TilePos) -> i32 {
        (self.x - other.x).abs().max((self.y - other.y).abs())
    }
}
