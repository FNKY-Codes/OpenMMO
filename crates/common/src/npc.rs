use crate::{NpcDef, TilePos};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcFootprint {
    pub width: u32,
    pub height: u32,
}

impl Default for NpcFootprint {
    fn default() -> Self {
        Self {
            width: 1,
            height: 1,
        }
    }
}

impl NpcFootprint {
    pub fn from_def(def: &NpcDef) -> Self {
        Self {
            width: def.footprint_w.max(1),
            height: def.footprint_h.max(1),
        }
    }

    pub fn is_multi_tile(self) -> bool {
        self.width > 1 || self.height > 1
    }

    /// South-west anchor tile used for line-of-sight and grid placement.
    pub fn los_tile(self, sw: TilePos) -> TilePos {
        let _ = self;
        sw
    }

    pub fn occupied_tiles(self, sw: TilePos) -> Vec<TilePos> {
        let mut tiles = Vec::new();
        for dx in 0..self.width {
            for dy in 0..self.height {
                tiles.push(TilePos::new(sw.x + dx as i32, sw.y + dy as i32));
            }
        }
        tiles
    }

    pub fn los_distance(self, from: TilePos, npc_sw: TilePos) -> i32 {
        from.chebyshev_distance(&self.los_tile(npc_sw))
    }

    pub fn player_adjacent(self, player: TilePos, sw: TilePos) -> bool {
        self.occupied_tiles(sw).into_iter().any(|tile| {
            player.chebyshev_distance(&tile) <= 1
        })
    }

    pub fn world_center(self, sw: TilePos, surface_y: f32) -> [f32; 3] {
        [
            sw.x as f32 + self.width as f32 * 0.5,
            surface_y,
            sw.y as f32 + self.height as f32 * 0.5,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TilePos;

    #[test]
    fn los_tile_is_south_west_anchor() {
        let fp = NpcFootprint {
            width: 2,
            height: 2,
        };
        let sw = TilePos::new(3, 4);
        assert_eq!(fp.los_tile(sw), sw);
    }

    #[test]
    fn occupied_tiles_cover_footprint() {
        let fp = NpcFootprint {
            width: 2,
            height: 2,
        };
        let sw = TilePos::new(1, 2);
        let tiles = fp.occupied_tiles(sw);
        assert_eq!(tiles.len(), 4);
        assert!(tiles.contains(&TilePos::new(1, 2)));
        assert!(tiles.contains(&TilePos::new(2, 2)));
        assert!(tiles.contains(&TilePos::new(1, 3)));
        assert!(tiles.contains(&TilePos::new(2, 3)));
    }

    #[test]
    fn player_adjacent_when_next_to_any_footprint_tile() {
        let fp = NpcFootprint {
            width: 2,
            height: 2,
        };
        let sw = TilePos::new(5, 5);
        assert!(fp.player_adjacent(TilePos::new(4, 5), sw));
        assert!(fp.player_adjacent(TilePos::new(7, 6), sw));
        assert!(!fp.player_adjacent(TilePos::new(3, 3), sw));
    }

    #[test]
    fn los_distance_uses_south_west_tile() {
        let fp = NpcFootprint {
            width: 2,
            height: 2,
        };
        let sw = TilePos::new(10, 10);
        assert_eq!(fp.los_distance(TilePos::new(12, 10), sw), 2);
        assert_eq!(fp.los_distance(TilePos::new(11, 11), sw), 1);
    }
}
