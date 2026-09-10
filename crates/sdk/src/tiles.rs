//! Ground tile kinds and the ASCII region layout format.
//!
//! A region's ground is a byte per tile (`RegionDef::tiles`). Rather than
//! author 6400 numbers by hand, regions can be written as text art:
//!
//! ```yaml
//! layout:
//!   - "^^^^^^^^^^"
//!   - "^..T..r..^"
//!   - "^..==S...^"
//!   - "^^^^^^^^^^"
//! legend:
//!   "^": { tile: cliff }
//!   ".": { tile: grass }
//!   "=": { tile: road }
//!   "T": { tile: grass, object: 1 }
//!   "r": { tile: grass, npc: 1 }
//!   "S": { tile: road, spawn: true }
//! ```
//!
//! [`RegionDef::expand_layout`] turns that into `tiles`, `objects`, `npcs`,
//! `transitions` and `spawn`. Row 0 is `y = 0`; column 0 is `x = 0`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{NpcId, ObjectId, RegionId, TilePos};

/// What a ground tile is made of. Stored as a byte in `RegionDef::tiles`.
///
/// Walkability is a property of the kind, shared by the server (movement,
/// pathing), the client (rendering, click targets) and the validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum TileKind {
    Grass = 0,
    /// Solid rock / boulder. Blocks movement.
    Rock = 1,
    /// Deep water. Blocks movement.
    Water = 2,
    Dirt = 3,
    Road = 4,
    Sand = 5,
    /// Broken masonry, walkable.
    Rubble = 6,
    CaveFloor = 7,
    /// Cliff face. Blocks movement.
    Cliff = 8,
    /// Building wall. Blocks movement.
    Wall = 9,
    /// Interior floor (plank / stone).
    Floor = 10,
    Mud = 11,
    Ash = 12,
    Snow = 13,
    /// Shallow water, walkable.
    Shallows = 14,
    /// Lava / toxic pool. Blocks movement.
    Hazard = 15,
}

impl TileKind {
    pub const ALL: &'static [TileKind] = &[
        TileKind::Grass,
        TileKind::Rock,
        TileKind::Water,
        TileKind::Dirt,
        TileKind::Road,
        TileKind::Sand,
        TileKind::Rubble,
        TileKind::CaveFloor,
        TileKind::Cliff,
        TileKind::Wall,
        TileKind::Floor,
        TileKind::Mud,
        TileKind::Ash,
        TileKind::Snow,
        TileKind::Shallows,
        TileKind::Hazard,
    ];

    pub fn from_byte(b: u8) -> Option<TileKind> {
        TileKind::ALL.iter().copied().find(|k| *k as u8 == b)
    }

    pub fn byte(self) -> u8 {
        self as u8
    }

    /// Can players and NPCs stand on this tile?
    pub fn walkable(self) -> bool {
        !matches!(
            self,
            TileKind::Rock | TileKind::Water | TileKind::Cliff | TileKind::Wall | TileKind::Hazard
        )
    }

    /// Whether a byte in `RegionDef::tiles` is walkable. Unknown bytes are
    /// treated as blocked so a typo can't punch a hole through a wall.
    pub fn byte_walkable(b: u8) -> bool {
        TileKind::from_byte(b).is_some_and(TileKind::walkable)
    }

    /// Does this tile block line of sight / count as a solid volume when
    /// rendered (walls, rock, cliffs)?
    pub fn solid(self) -> bool {
        matches!(self, TileKind::Rock | TileKind::Cliff | TileKind::Wall)
    }
}

/// How a region transition is presented in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    /// Generic glowing tile (legacy).
    #[default]
    Portal,
    /// Dark cave mouth in a rock face.
    Cave,
    /// A door in a wall.
    Door,
    /// Stairs going up.
    StairsUp,
    /// Stairs / ladder going down.
    StairsDown,
    /// An open path off the edge of the region.
    Path,
}

/// One character of a region layout.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LegendEntry {
    /// Ground under this cell. Required.
    pub tile: Option<TileKind>,
    /// Place this object here.
    #[serde(default)]
    pub object: Option<ObjectId>,
    /// Spawn this NPC here.
    #[serde(default)]
    pub npc: Option<NpcId>,
    /// New players (and respawns) start here. At most one cell per region.
    #[serde(default)]
    pub spawn: bool,
    /// Walking onto this cell moves the player to another region.
    #[serde(default)]
    pub portal: Option<LegendPortal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegendPortal {
    pub region: RegionId,
    /// `[x, y]` in the target region.
    pub spawn: [i32; 2],
    #[serde(default)]
    pub kind: TransitionKind,
    /// Hover / examine text, e.g. "Cave entrance".
    #[serde(default)]
    pub label: String,
}

/// Errors from expanding a layout. Reported by the validator, and fatal at
/// load time.
#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    #[error("region {region}: layout row {row} has {got} columns, expected {expected}")]
    RaggedRow {
        region: String,
        row: usize,
        got: usize,
        expected: usize,
    },
    #[error("region {region}: layout char {ch:?} at ({x},{y}) has no legend entry")]
    UnknownChar {
        region: String,
        ch: char,
        x: i32,
        y: i32,
    },
    #[error("region {region}: legend entry {ch:?} has no `tile`")]
    MissingTile { region: String, ch: char },
    #[error("region {region}: more than one spawn cell in layout")]
    MultipleSpawns { region: String },
    #[error("region {region}: width/height {w}x{h} do not match layout {lw}x{lh}")]
    SizeMismatch {
        region: String,
        w: u32,
        h: u32,
        lw: u32,
        lh: u32,
    },
    #[error("region {region}: legend entry {ch:?} is unused")]
    UnusedLegend { region: String, ch: char },
}

/// Result of expanding a layout into flat region data.
pub struct ExpandedLayout {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<u8>,
    pub objects: Vec<(ObjectId, TilePos)>,
    pub npcs: Vec<(NpcId, TilePos)>,
    pub spawn: Option<TilePos>,
    pub transitions: Vec<(TilePos, LegendPortal)>,
}

pub fn expand_layout(
    region_name: &str,
    layout: &[String],
    legend: &BTreeMap<char, LegendEntry>,
) -> Result<ExpandedLayout, LayoutError> {
    let height = layout.len() as u32;
    let width = layout.first().map(|r| r.chars().count()).unwrap_or(0) as u32;
    let mut tiles = Vec::with_capacity((width * height) as usize);
    let mut objects = Vec::new();
    let mut npcs = Vec::new();
    let mut transitions = Vec::new();
    let mut spawn = None;
    let mut used = std::collections::BTreeSet::new();

    for (y, row) in layout.iter().enumerate() {
        let cols: Vec<char> = row.chars().collect();
        if cols.len() as u32 != width {
            return Err(LayoutError::RaggedRow {
                region: region_name.to_string(),
                row: y,
                got: cols.len(),
                expected: width as usize,
            });
        }
        for (x, ch) in cols.into_iter().enumerate() {
            let pos = TilePos::new(x as i32, y as i32);
            let Some(entry) = legend.get(&ch) else {
                return Err(LayoutError::UnknownChar {
                    region: region_name.to_string(),
                    ch,
                    x: pos.x,
                    y: pos.y,
                });
            };
            used.insert(ch);
            let Some(tile) = entry.tile else {
                return Err(LayoutError::MissingTile {
                    region: region_name.to_string(),
                    ch,
                });
            };
            tiles.push(tile.byte());
            if let Some(id) = entry.object {
                objects.push((id, pos));
            }
            if let Some(id) = entry.npc {
                npcs.push((id, pos));
            }
            if entry.spawn {
                if spawn.is_some() {
                    return Err(LayoutError::MultipleSpawns {
                        region: region_name.to_string(),
                    });
                }
                spawn = Some(pos);
            }
            if let Some(portal) = &entry.portal {
                transitions.push((pos, portal.clone()));
            }
        }
    }

    if let Some(ch) = legend.keys().find(|c| !used.contains(c)) {
        return Err(LayoutError::UnusedLegend {
            region: region_name.to_string(),
            ch: *ch,
        });
    }

    Ok(ExpandedLayout {
        width,
        height,
        tiles,
        objects,
        npcs,
        spawn,
        transitions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legend() -> BTreeMap<char, LegendEntry> {
        let mut l = BTreeMap::new();
        l.insert(
            '.',
            LegendEntry {
                tile: Some(TileKind::Grass),
                ..Default::default()
            },
        );
        l.insert(
            '#',
            LegendEntry {
                tile: Some(TileKind::Wall),
                ..Default::default()
            },
        );
        l.insert(
            'S',
            LegendEntry {
                tile: Some(TileKind::Road),
                spawn: true,
                ..Default::default()
            },
        );
        l.insert(
            'T',
            LegendEntry {
                tile: Some(TileKind::Grass),
                object: Some(ObjectId(1)),
                ..Default::default()
            },
        );
        l
    }

    #[test]
    fn expands_rows_into_tiles_and_entities() {
        let rows = vec![
            "####".to_string(),
            "#ST#".to_string(),
            "#..#".to_string(),
            "####".to_string(),
        ];
        let e = expand_layout("t", &rows, &legend()).unwrap();
        assert_eq!((e.width, e.height), (4, 4));
        assert_eq!(e.tiles.len(), 16);
        assert_eq!(e.tiles[0], TileKind::Wall.byte());
        assert_eq!(e.tiles[5], TileKind::Road.byte());
        assert_eq!(e.spawn, Some(TilePos::new(1, 1)));
        assert_eq!(e.objects, vec![(ObjectId(1), TilePos::new(2, 1))]);
        assert!(TileKind::byte_walkable(e.tiles[5]));
        assert!(!TileKind::byte_walkable(e.tiles[0]));
    }

    #[test]
    fn ragged_rows_and_unknown_chars_are_errors() {
        let rows = vec!["###".to_string(), "#S".to_string()];
        assert!(matches!(
            expand_layout("t", &rows, &legend()),
            Err(LayoutError::RaggedRow { row: 1, .. })
        ));
        let rows = vec!["#S?".to_string()];
        assert!(matches!(
            expand_layout("t", &rows, &legend()),
            Err(LayoutError::UnknownChar { ch: '?', .. })
        ));
    }

    #[test]
    fn unused_legend_entry_is_an_error() {
        let rows = vec!["S.".to_string()];
        assert!(matches!(
            expand_layout("t", &rows, &legend()),
            Err(LayoutError::UnusedLegend { .. })
        ));
    }

    #[test]
    fn unknown_tile_bytes_block_movement() {
        assert!(!TileKind::byte_walkable(200));
        assert!(!TileKind::byte_walkable(255));
        assert!(TileKind::byte_walkable(TileKind::Shallows.byte()));
    }
}
