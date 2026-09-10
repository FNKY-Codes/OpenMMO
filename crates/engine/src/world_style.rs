//! Visual palette for ground tiles and region ambience.
//!
//! One source of truth for how a `TileKind` looks: block height (also the
//! surface entities stand on), colours, and the minimap swatch.

use openmmo_common::{RegionAmbience, TileKind};

#[derive(Debug, Clone, Copy)]
pub struct TileStyle {
    /// Height of the tile block; entities stand on top of it.
    pub height: f32,
    pub color: [f32; 4],
}

/// Deterministic per-tile noise in `[-1, 1]` so ground doesn't look like a
/// spreadsheet. Cheap integer hash.
pub fn tile_noise(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B9) ^ (y as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    (h & 0xFFFF) as f32 / 32767.5 - 1.0
}

/// Base look of a tile kind, before per-tile variation.
pub fn base_style(kind: TileKind) -> TileStyle {
    let (height, color) = match kind {
        TileKind::Grass => (0.12, [0.33, 0.56, 0.27, 1.0]),
        TileKind::Dirt => (0.11, [0.47, 0.37, 0.25, 1.0]),
        TileKind::Road => (0.13, [0.58, 0.55, 0.47, 1.0]),
        TileKind::Sand => (0.11, [0.78, 0.71, 0.48, 1.0]),
        TileKind::Rubble => (0.14, [0.50, 0.48, 0.44, 1.0]),
        TileKind::CaveFloor => (0.10, [0.30, 0.28, 0.30, 1.0]),
        TileKind::Floor => (0.14, [0.60, 0.46, 0.30, 1.0]),
        TileKind::Mud => (0.09, [0.36, 0.28, 0.20, 1.0]),
        TileKind::Ash => (0.10, [0.42, 0.40, 0.40, 1.0]),
        TileKind::Snow => (0.13, [0.88, 0.90, 0.94, 1.0]),
        TileKind::Shallows => (0.06, [0.36, 0.58, 0.66, 1.0]),
        TileKind::Water => (0.03, [0.16, 0.34, 0.58, 1.0]),
        TileKind::Hazard => (0.05, [0.85, 0.42, 0.10, 1.0]),
        // Solid volumes: tall enough to read as obstacles from the orbit camera.
        TileKind::Rock => (0.85, [0.46, 0.43, 0.40, 1.0]),
        TileKind::Cliff => (1.35, [0.36, 0.33, 0.32, 1.0]),
        TileKind::Wall => (1.15, [0.55, 0.52, 0.47, 1.0]),
    };
    TileStyle { height, color }
}

/// Style with per-tile variation applied.
pub fn tile_style(kind: TileKind, x: i32, y: i32) -> TileStyle {
    let base = base_style(kind);
    let n = tile_noise(x, y);
    let amount = match kind {
        TileKind::Grass | TileKind::Sand | TileKind::Dirt | TileKind::Snow => 0.06,
        TileKind::Rock | TileKind::Cliff | TileKind::Rubble | TileKind::CaveFloor => 0.08,
        TileKind::Water | TileKind::Shallows => 0.03,
        _ => 0.04,
    };
    let f = 1.0 + n * amount;
    let mut color = base.color;
    for c in color.iter_mut().take(3) {
        *c = (*c * f).clamp(0.0, 1.0);
    }
    // Rough ground gets a little relief; solid volumes vary in height too.
    let height = match kind {
        TileKind::Rock => base.height + n * 0.18,
        TileKind::Cliff => base.height + n * 0.22,
        TileKind::Rubble => base.height + n.abs() * 0.05,
        _ => base.height,
    };
    TileStyle { height, color }
}

/// Surface height for a raw tile byte (unknown bytes are treated as rock).
pub fn tile_height(byte: u8, x: i32, y: i32) -> f32 {
    let kind = TileKind::from_byte(byte).unwrap_or(TileKind::Rock);
    tile_style(kind, x, y).height
}

/// Minimap swatch.
pub fn minimap_color(kind: TileKind) -> [u8; 3] {
    let c = base_style(kind).color;
    let f = if kind.solid() { 0.75 } else { 1.0 };
    [
        (c[0] * f * 255.0) as u8,
        (c[1] * f * 255.0) as u8,
        (c[2] * f * 255.0) as u8,
    ]
}

/// Sky / clear colour for a region's ambience.
pub fn clear_color(ambience: RegionAmbience) -> [f64; 3] {
    match ambience {
        RegionAmbience::Overworld => [0.62, 0.78, 0.90],
        RegionAmbience::Coast => [0.70, 0.82, 0.92],
        RegionAmbience::Mountain => [0.74, 0.80, 0.90],
        RegionAmbience::Cave => [0.05, 0.05, 0.07],
        RegionAmbience::Dungeon => [0.07, 0.05, 0.10],
    }
}

/// Brightness multiplier applied to props/entities in dim regions.
pub fn ambient_light(ambience: RegionAmbience) -> f32 {
    match ambience {
        RegionAmbience::Cave => 0.72,
        RegionAmbience::Dungeon => 0.68,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solid_kinds_are_taller_than_ground() {
        let ground = base_style(TileKind::Grass).height;
        for k in [TileKind::Rock, TileKind::Cliff, TileKind::Wall] {
            assert!(base_style(k).height > ground * 4.0, "{k:?}");
        }
    }

    #[test]
    fn noise_is_deterministic_and_bounded() {
        for x in -20..20 {
            for y in -20..20 {
                let n = tile_noise(x, y);
                assert!((-1.0..=1.0).contains(&n));
                assert_eq!(n, tile_noise(x, y));
            }
        }
    }

    #[test]
    fn every_kind_has_a_style() {
        for k in TileKind::ALL {
            let s = tile_style(*k, 3, 7);
            assert!(s.height > 0.0);
            assert!(s.color.iter().all(|c| (0.0..=1.0).contains(c)));
        }
    }
}
