//! Procedural props: trees, rocks, stations, portals and placeholder bodies
//! for NPCs without a model. Everything is flat-shaded boxes and prisms in
//! the same vertex-colour style as the ground, so the world reads as one
//! piece.
//!
//! Builders append triangles to a `Vec<Vertex>`; `base` is the world-space
//! centre of the tile's top surface.

use openmmo_common::{NpcFootprint, ObjectDef, PropModel, TilePos, TransitionKind};

use crate::renderer::{add_box, push_tri, Vertex};
use crate::world_style::tile_noise;

type Color = [f32; 4];

// --- palette ---------------------------------------------------------------
const BARK_BIRCH: Color = [0.88, 0.87, 0.80, 1.0];
const BARK_OAK: Color = [0.40, 0.29, 0.18, 1.0];
const BARK_DEAD: Color = [0.24, 0.20, 0.16, 1.0];
const LEAF_BIRCH: Color = [0.58, 0.76, 0.40, 1.0];
const LEAF_OAK: Color = [0.24, 0.48, 0.24, 1.0];
const LEAF_PINE: Color = [0.17, 0.40, 0.27, 1.0];
const LEAF_BUSH: Color = [0.34, 0.58, 0.30, 1.0];
const STONE: Color = [0.50, 0.48, 0.45, 1.0];
const STONE_DARK: Color = [0.34, 0.33, 0.31, 1.0];
const WOOD: Color = [0.52, 0.36, 0.20, 1.0];
const WOOD_DARK: Color = [0.36, 0.25, 0.14, 1.0];
const IRON: Color = [0.28, 0.28, 0.30, 1.0];
const FLAME: Color = [1.0, 0.55, 0.12, 1.0];
const FLAME_CORE: Color = [1.0, 0.85, 0.35, 1.0];
const GLOW_TEAL: Color = [0.55, 0.95, 0.90, 1.0];
const WATER: Color = [0.24, 0.45, 0.66, 1.0];
const CANVAS: Color = [0.68, 0.60, 0.44, 1.0];
const PAPER: Color = [0.92, 0.90, 0.82, 1.0];
const GOLD: Color = [0.90, 0.75, 0.25, 1.0];

fn scale(c: Color, f: f32) -> Color {
    [
        (c[0] * f).clamp(0.0, 1.0),
        (c[1] * f).clamp(0.0, 1.0),
        (c[2] * f).clamp(0.0, 1.0),
        c[3],
    ]
}

/// Box at `base + offset` with the given size, rotated by `yaw` around the
/// vertical axis through `base`.
fn boxed(
    base: [f32; 3],
    yaw: f32,
    offset: [f32; 3],
    size: [f32; 3],
    color: Color,
    out: &mut Vec<Vertex>,
) {
    if yaw == 0.0 {
        add_box(
            [
                base[0] + offset[0],
                base[1] + offset[1] + size[1] * 0.5,
                base[2] + offset[2],
            ],
            size,
            color,
            out,
            false,
        );
        return;
    }
    let start = out.len();
    add_box(
        [offset[0], offset[1] + size[1] * 0.5, offset[2]],
        size,
        color,
        out,
        false,
    );
    let (s, c) = yaw.sin_cos();
    for v in &mut out[start..] {
        let [x, y, z] = v.position;
        v.position = [
            base[0] + x * c - z * s,
            base[1] + y,
            base[2] + x * s + z * c,
        ];
    }
}

/// Four-sided pyramid / cone approximation.
fn cone(
    base: [f32; 3],
    offset: [f32; 3],
    radius: f32,
    height: f32,
    color: Color,
    out: &mut Vec<Vertex>,
) {
    let [bx, by, bz] = [
        base[0] + offset[0],
        base[1] + offset[1],
        base[2] + offset[2],
    ];
    let apex = [bx, by + height, bz];
    let ring = [
        [bx - radius, by, bz - radius],
        [bx + radius, by, bz - radius],
        [bx + radius, by, bz + radius],
        [bx - radius, by, bz + radius],
    ];
    for i in 0..4 {
        let a = ring[i];
        let b = ring[(i + 1) % 4];
        let shade = 0.72 + 0.28 * ((i as f32 * 1.7).sin() * 0.5 + 0.5);
        push_tri(a, b, apex, scale(color, shade), out);
    }
    push_tri(ring[0], ring[2], ring[1], scale(color, 0.6), out);
    push_tri(ring[0], ring[3], ring[2], scale(color, 0.6), out);
}

/// A-frame prism along x (a tent).
fn prism(
    base: [f32; 3],
    offset: [f32; 3],
    length: f32,
    width: f32,
    height: f32,
    color: Color,
    out: &mut Vec<Vertex>,
) {
    let [cx, cy, cz] = [
        base[0] + offset[0],
        base[1] + offset[1],
        base[2] + offset[2],
    ];
    let hl = length * 0.5;
    let hw = width * 0.5;
    let a0 = [cx - hl, cy, cz - hw];
    let a1 = [cx + hl, cy, cz - hw];
    let b0 = [cx - hl, cy, cz + hw];
    let b1 = [cx + hl, cy, cz + hw];
    let r0 = [cx - hl, cy + height, cz];
    let r1 = [cx + hl, cy + height, cz];
    // two roof slopes
    push_tri(a0, r0, r1, scale(color, 0.95), out);
    push_tri(a0, r1, a1, scale(color, 0.95), out);
    push_tri(b1, r1, r0, scale(color, 0.8), out);
    push_tri(b1, r0, b0, scale(color, 0.8), out);
    // ends
    push_tri(a0, b0, r0, scale(color, 0.7), out);
    push_tri(a1, r1, b1, scale(color, 0.7), out);
}

/// Prop footprint used for hover outlines and picking: (width, height).
pub fn prop_dims(model: PropModel) -> (f32, f32) {
    match model {
        PropModel::Tree => (0.9, 2.2),
        PropModel::Oak => (1.3, 2.4),
        PropModel::Pine => (1.0, 2.6),
        PropModel::DeadTree => (0.8, 2.0),
        PropModel::Bush => (0.8, 0.6),
        PropModel::Stump => (0.5, 0.35),
        PropModel::Rock => (0.8, 0.6),
        PropModel::Boulder => (1.0, 0.9),
        PropModel::OreVein => (0.9, 0.8),
        PropModel::Crystal => (0.7, 1.3),
        PropModel::Pool => (0.9, 0.15),
        PropModel::Anvil => (0.7, 0.6),
        PropModel::Furnace => (0.9, 1.5),
        PropModel::Workbench => (1.0, 0.8),
        PropModel::BankBooth => (0.9, 0.7),
        PropModel::MarketStall => (1.0, 1.6),
        PropModel::Campfire => (0.8, 0.7),
        PropModel::Crate => (0.7, 0.7),
        PropModel::Barrel => (0.6, 0.8),
        PropModel::Sign => (0.8, 1.4),
        PropModel::Fence => (1.0, 0.9),
        PropModel::Lamp => (0.4, 2.0),
        PropModel::Well => (0.9, 1.6),
        PropModel::Tent => (1.4, 1.2),
        PropModel::Rubble => (0.9, 0.35),
        PropModel::Mushroom => (0.5, 0.5),
        PropModel::Cactus => (0.5, 1.4),
        PropModel::Statue => (0.8, 2.0),
    }
}

/// Fleck colour for ore veins, keyed off the node's name.
fn vein_fleck(def: &ObjectDef) -> Color {
    let n = def.name.to_ascii_lowercase();
    if n.contains("copper") {
        [0.35, 0.72, 0.50, 1.0]
    } else if n.contains("iron") {
        [0.72, 0.38, 0.22, 1.0]
    } else if n.contains("coal") {
        [0.10, 0.10, 0.11, 1.0]
    } else {
        [0.80, 0.70, 0.30, 1.0]
    }
}

/// Build a prop for an object. `seed` varies rotation/size between instances
/// of the same prop; `light` scales colours (caves are dim).
pub fn build_prop(
    def: &ObjectDef,
    base: [f32; 3],
    tile: TilePos,
    depleted: bool,
    light: f32,
    out: &mut Vec<Vertex>,
) {
    let model = def.model_or_default();
    let n = tile_noise(tile.x, tile.y);
    let yaw = n * 0.6;
    let s = 0.9 + n.abs() * 0.2;
    let start = out.len();

    if depleted {
        match model {
            PropModel::Tree | PropModel::Oak | PropModel::Pine | PropModel::DeadTree => {
                boxed(
                    base,
                    yaw,
                    [0.0, 0.0, 0.0],
                    [0.38 * s, 0.28, 0.38 * s],
                    BARK_OAK,
                    out,
                );
                boxed(
                    base,
                    yaw,
                    [0.0, 0.28, 0.0],
                    [0.34 * s, 0.03, 0.34 * s],
                    scale(BARK_BIRCH, 0.8),
                    out,
                );
            }
            PropModel::OreVein | PropModel::Rock | PropModel::Boulder => {
                boxed(
                    base,
                    yaw,
                    [0.0, 0.0, 0.0],
                    [0.8 * s, 0.35, 0.7 * s],
                    STONE_DARK,
                    out,
                );
            }
            PropModel::Mushroom | PropModel::Bush => {
                boxed(
                    base,
                    yaw,
                    [0.0, 0.0, 0.0],
                    [0.3, 0.08, 0.3],
                    scale(LEAF_BUSH, 0.5),
                    out,
                );
            }
            _ => build_model(model, def, base, yaw, s, out),
        }
    } else {
        build_model(model, def, base, yaw, s, out);
    }

    if light < 1.0 {
        for v in &mut out[start..] {
            v.color = scale(v.color, light);
        }
    }
}

fn build_model(
    model: PropModel,
    def: &ObjectDef,
    base: [f32; 3],
    yaw: f32,
    s: f32,
    out: &mut Vec<Vertex>,
) {
    match model {
        PropModel::Tree => {
            boxed(
                base,
                yaw,
                [0.0, 0.0, 0.0],
                [0.18, 1.1 * s, 0.18],
                BARK_BIRCH,
                out,
            );
            boxed(
                base,
                yaw,
                [0.0, 0.1, 0.0],
                [0.14, 0.5, 0.05],
                scale(BARK_BIRCH, 0.55),
                out,
            );
            boxed(
                base,
                yaw,
                [0.05, 0.9 * s, -0.05],
                [0.9 * s, 0.55, 0.9 * s],
                LEAF_BIRCH,
                out,
            );
            boxed(
                base,
                yaw + 0.5,
                [-0.1, 1.3 * s, 0.1],
                [0.7 * s, 0.5, 0.7 * s],
                scale(LEAF_BIRCH, 1.08),
                out,
            );
            boxed(
                base,
                yaw,
                [0.0, 1.7 * s, 0.0],
                [0.45 * s, 0.4, 0.45 * s],
                scale(LEAF_BIRCH, 1.15),
                out,
            );
        }
        PropModel::Oak => {
            boxed(
                base,
                yaw,
                [0.0, 0.0, 0.0],
                [0.32, 1.0 * s, 0.32],
                BARK_OAK,
                out,
            );
            boxed(
                base,
                yaw,
                [0.0, 0.8 * s, 0.0],
                [1.3 * s, 0.7, 1.3 * s],
                LEAF_OAK,
                out,
            );
            boxed(
                base,
                yaw + 0.6,
                [0.1, 1.35 * s, -0.1],
                [1.0 * s, 0.6, 1.0 * s],
                scale(LEAF_OAK, 1.1),
                out,
            );
            boxed(
                base,
                yaw,
                [0.0, 1.85 * s, 0.0],
                [0.6 * s, 0.45, 0.6 * s],
                scale(LEAF_OAK, 1.2),
                out,
            );
        }
        PropModel::Pine => {
            boxed(
                base,
                yaw,
                [0.0, 0.0, 0.0],
                [0.22, 0.9 * s, 0.22],
                BARK_OAK,
                out,
            );
            cone(base, [0.0, 0.6 * s, 0.0], 0.6 * s, 0.9, LEAF_PINE, out);
            cone(
                base,
                [0.0, 1.15 * s, 0.0],
                0.48 * s,
                0.8,
                scale(LEAF_PINE, 1.1),
                out,
            );
            cone(
                base,
                [0.0, 1.65 * s, 0.0],
                0.34 * s,
                0.7,
                scale(LEAF_PINE, 1.2),
                out,
            );
        }
        PropModel::DeadTree => {
            boxed(
                base,
                yaw,
                [0.0, 0.0, 0.0],
                [0.22, 1.5 * s, 0.22],
                BARK_DEAD,
                out,
            );
            boxed(
                base,
                yaw + 0.9,
                [0.3, 0.9 * s, 0.0],
                [0.7, 0.1, 0.1],
                BARK_DEAD,
                out,
            );
            boxed(
                base,
                yaw - 0.7,
                [-0.25, 1.2 * s, 0.05],
                [0.55, 0.09, 0.09],
                BARK_DEAD,
                out,
            );
        }
        PropModel::Bush => {
            boxed(
                base,
                yaw,
                [0.0, 0.0, 0.0],
                [0.7 * s, 0.4, 0.6 * s],
                LEAF_BUSH,
                out,
            );
            boxed(
                base,
                yaw + 0.8,
                [0.1, 0.25, 0.1],
                [0.5 * s, 0.35, 0.5 * s],
                scale(LEAF_BUSH, 1.12),
                out,
            );
        }
        PropModel::Stump => {
            boxed(base, yaw, [0.0, 0.0, 0.0], [0.42, 0.3, 0.42], BARK_OAK, out);
            boxed(
                base,
                yaw,
                [0.0, 0.3, 0.0],
                [0.36, 0.03, 0.36],
                scale(BARK_BIRCH, 0.85),
                out,
            );
        }
        PropModel::Rock => {
            boxed(
                base,
                yaw,
                [0.0, 0.0, 0.0],
                [0.75 * s, 0.45, 0.6 * s],
                STONE,
                out,
            );
            boxed(
                base,
                yaw + 0.4,
                [0.1, 0.4, -0.05],
                [0.4 * s, 0.25, 0.35 * s],
                scale(STONE, 1.1),
                out,
            );
        }
        PropModel::Boulder => {
            boxed(
                base,
                yaw,
                [0.0, 0.0, 0.0],
                [1.0 * s, 0.7, 0.85 * s],
                STONE,
                out,
            );
            boxed(
                base,
                yaw + 0.5,
                [0.12, 0.6, 0.05],
                [0.55 * s, 0.35, 0.5 * s],
                scale(STONE, 1.12),
                out,
            );
        }
        PropModel::OreVein => {
            let fleck = vein_fleck(def);
            boxed(
                base,
                yaw,
                [0.0, 0.0, 0.0],
                [0.9 * s, 0.6, 0.8 * s],
                STONE_DARK,
                out,
            );
            boxed(
                base,
                yaw + 0.5,
                [0.1, 0.5, -0.1],
                [0.5 * s, 0.3, 0.45 * s],
                STONE,
                out,
            );
            for (i, off) in [
                [0.28, 0.25, 0.42],
                [-0.3, 0.4, 0.2],
                [0.1, 0.62, -0.3],
                [-0.2, 0.15, -0.4],
            ]
            .iter()
            .enumerate()
            {
                let sz = 0.12 + (i as f32) * 0.02;
                boxed(base, yaw, *off, [sz, sz, sz], fleck, out);
            }
        }
        PropModel::Crystal => {
            for (i, (dx, dz, h)) in [(0.0, 0.0, 1.3), (0.22, -0.15, 0.9), (-0.2, 0.18, 0.7)]
                .iter()
                .enumerate()
            {
                let c = scale(GLOW_TEAL, 0.85 + i as f32 * 0.07);
                boxed(
                    base,
                    yaw + i as f32 * 0.5,
                    [*dx, 0.0, *dz],
                    [0.18, *h, 0.18],
                    c,
                    out,
                );
                cone(base, [*dx, *h, *dz], 0.09, 0.22, c, out);
            }
        }
        PropModel::Pool => {
            boxed(
                base,
                0.0,
                [0.0, -0.02, 0.0],
                [0.95, 0.03, 0.95],
                scale(WATER, 0.9),
                out,
            );
            boxed(
                base,
                0.0,
                [0.0, 0.01, 0.0],
                [0.5, 0.02, 0.5],
                scale(WATER, 1.25),
                out,
            );
            boxed(
                base,
                0.6,
                [0.25, 0.03, -0.2],
                [0.14, 0.04, 0.14],
                PAPER,
                out,
            );
        }
        PropModel::Anvil => {
            boxed(
                base,
                yaw,
                [0.0, 0.0, 0.0],
                [0.45, 0.3, 0.35],
                STONE_DARK,
                out,
            );
            boxed(base, yaw, [0.0, 0.3, 0.0], [0.7, 0.16, 0.32], IRON, out);
            boxed(base, yaw, [0.45, 0.32, 0.0], [0.22, 0.12, 0.18], IRON, out);
        }
        PropModel::Furnace => {
            boxed(base, yaw, [0.0, 0.0, 0.0], [0.9, 1.1, 0.9], STONE, out);
            boxed(
                base,
                yaw,
                [0.0, 1.1, 0.0],
                [0.4, 0.45, 0.4],
                STONE_DARK,
                out,
            );
            boxed(base, yaw, [0.0, 0.2, 0.47], [0.4, 0.35, 0.02], FLAME, out);
            boxed(
                base,
                yaw,
                [0.0, 0.28, 0.48],
                [0.2, 0.18, 0.02],
                FLAME_CORE,
                out,
            );
        }
        PropModel::Workbench => {
            for (dx, dz) in [(-0.4, -0.25), (0.4, -0.25), (-0.4, 0.25), (0.4, 0.25)] {
                boxed(base, yaw, [dx, 0.0, dz], [0.1, 0.65, 0.1], WOOD_DARK, out);
            }
            boxed(base, yaw, [0.0, 0.65, 0.0], [1.0, 0.08, 0.6], WOOD, out);
            boxed(
                base,
                yaw,
                [0.25, 0.73, -0.1],
                [0.3, 0.08, 0.15],
                scale(BARK_BIRCH, 0.9),
                out,
            );
            boxed(base, yaw, [-0.25, 0.73, 0.1], [0.18, 0.1, 0.18], IRON, out);
        }
        PropModel::BankBooth => {
            boxed(base, yaw, [0.0, 0.0, 0.0], [0.85, 0.45, 0.6], WOOD, out);
            boxed(
                base,
                yaw,
                [0.0, 0.45, 0.0],
                [0.87, 0.2, 0.62],
                WOOD_DARK,
                out,
            );
            boxed(base, yaw, [0.0, 0.3, 0.31], [0.14, 0.14, 0.03], GOLD, out);
            for dx in [-0.38, 0.38] {
                boxed(base, yaw, [dx, 0.0, 0.0], [0.06, 0.66, 0.64], IRON, out);
            }
        }
        PropModel::MarketStall => {
            for dx in [-0.45, 0.45] {
                boxed(base, yaw, [dx, 0.0, 0.0], [0.1, 1.5, 0.1], WOOD_DARK, out);
            }
            boxed(base, yaw, [0.0, 0.5, 0.0], [1.0, 0.8, 0.08], WOOD, out);
            for (i, (dx, dy)) in [(-0.3, 0.9), (0.05, 0.75), (0.32, 1.0), (-0.1, 0.6)]
                .iter()
                .enumerate()
            {
                boxed(
                    base,
                    yaw,
                    [*dx, *dy, 0.05],
                    [0.2, 0.22 + i as f32 * 0.02, 0.02],
                    PAPER,
                    out,
                );
            }
            boxed(base, yaw, [0.0, 1.5, 0.0], [1.2, 0.08, 0.5], CANVAS, out);
        }
        PropModel::Campfire => {
            for i in 0..3 {
                boxed(
                    base,
                    yaw + i as f32 * 1.05,
                    [0.0, 0.0, 0.0],
                    [0.7, 0.12, 0.12],
                    WOOD_DARK,
                    out,
                );
            }
            boxed(base, yaw, [0.0, 0.12, 0.0], [0.32, 0.4, 0.32], FLAME, out);
            boxed(
                base,
                yaw + 0.7,
                [0.0, 0.2, 0.0],
                [0.18, 0.45, 0.18],
                FLAME_CORE,
                out,
            );
        }
        PropModel::Crate => {
            boxed(
                base,
                yaw,
                [0.0, 0.0, 0.0],
                [0.7 * s, 0.7 * s, 0.7 * s],
                WOOD,
                out,
            );
            boxed(
                base,
                yaw,
                [0.0, 0.33 * s, 0.0],
                [0.72 * s, 0.06, 0.72 * s],
                WOOD_DARK,
                out,
            );
        }
        PropModel::Barrel => {
            boxed(base, yaw, [0.0, 0.0, 0.0], [0.6, 0.8, 0.6], WOOD, out);
            boxed(base, yaw, [0.0, 0.15, 0.0], [0.63, 0.06, 0.63], IRON, out);
            boxed(base, yaw, [0.0, 0.6, 0.0], [0.63, 0.06, 0.63], IRON, out);
        }
        PropModel::Sign => {
            boxed(base, yaw, [0.0, 0.0, 0.0], [0.1, 1.2, 0.1], WOOD_DARK, out);
            boxed(base, yaw, [0.0, 0.85, 0.0], [0.8, 0.4, 0.08], WOOD, out);
            boxed(
                base,
                yaw,
                [-0.15, 1.0, 0.05],
                [0.35, 0.05, 0.02],
                PAPER,
                out,
            );
            boxed(base, yaw, [0.05, 0.9, 0.05], [0.45, 0.05, 0.02], PAPER, out);
        }
        PropModel::Fence => {
            for dx in [-0.42, 0.42] {
                boxed(base, 0.0, [dx, 0.0, 0.0], [0.1, 0.9, 0.1], WOOD_DARK, out);
            }
            boxed(base, 0.0, [0.0, 0.35, 0.0], [1.0, 0.08, 0.06], WOOD, out);
            boxed(base, 0.0, [0.0, 0.7, 0.0], [1.0, 0.08, 0.06], WOOD, out);
        }
        PropModel::Lamp => {
            boxed(base, 0.0, [0.0, 0.0, 0.0], [0.12, 1.7, 0.12], IRON, out);
            boxed(base, 0.0, [0.0, 1.7, 0.0], [0.3, 0.3, 0.3], FLAME_CORE, out);
            boxed(base, 0.0, [0.0, 2.0, 0.0], [0.38, 0.06, 0.38], IRON, out);
        }
        PropModel::Well => {
            boxed(base, 0.0, [0.0, 0.0, 0.0], [0.9, 0.5, 0.9], STONE, out);
            boxed(
                base,
                0.0,
                [0.0, 0.45, 0.0],
                [0.6, 0.08, 0.6],
                scale(WATER, 0.7),
                out,
            );
            for dx in [-0.4, 0.4] {
                boxed(base, 0.0, [dx, 0.5, 0.0], [0.1, 0.9, 0.1], WOOD_DARK, out);
            }
            prism(base, [0.0, 1.35, 0.0], 1.1, 0.9, 0.35, WOOD, out);
        }
        PropModel::Tent => {
            prism(base, [0.0, 0.0, 0.0], 1.4, 1.1, 1.1, CANVAS, out);
        }
        PropModel::Rubble => {
            for (i, (dx, dz)) in [(-0.25, -0.2), (0.2, 0.1), (-0.05, 0.3), (0.3, -0.3)]
                .iter()
                .enumerate()
            {
                let sz = 0.2 + i as f32 * 0.05;
                boxed(
                    base,
                    yaw + i as f32 * 0.4,
                    [*dx, 0.0, *dz],
                    [sz, sz * 0.7, sz * 0.8],
                    if i % 2 == 0 { STONE } else { STONE_DARK },
                    out,
                );
            }
        }
        PropModel::Mushroom => {
            for (dx, dz, h) in [(0.0, 0.0, 0.35), (0.18, 0.12, 0.22), (-0.15, 0.1, 0.18)] {
                boxed(base, 0.0, [dx, 0.0, dz], [0.1, h, 0.1], PAPER, out);
                boxed(base, 0.0, [dx, h, dz], [0.32, 0.12, 0.32], GLOW_TEAL, out);
            }
        }
        PropModel::Cactus => {
            boxed(base, yaw, [0.0, 0.0, 0.0], [0.3, 1.3, 0.3], LEAF_OAK, out);
            boxed(
                base,
                yaw,
                [0.3, 0.6, 0.0],
                [0.35, 0.18, 0.18],
                LEAF_OAK,
                out,
            );
            boxed(
                base,
                yaw,
                [0.45, 0.6, 0.0],
                [0.18, 0.55, 0.18],
                LEAF_OAK,
                out,
            );
        }
        PropModel::Statue => {
            boxed(base, yaw, [0.0, 0.0, 0.0], [0.8, 0.5, 0.8], STONE_DARK, out);
            boxed(base, yaw, [0.0, 0.5, 0.0], [0.28, 0.55, 0.2], STONE, out);
            boxed(base, yaw, [0.0, 1.05, 0.0], [0.4, 0.55, 0.26], STONE, out);
            boxed(
                base,
                yaw,
                [0.0, 1.6, 0.0],
                [0.26, 0.28, 0.26],
                scale(STONE, 1.08),
                out,
            );
        }
    }
}

/// Static portal dressing, built with the tile cache. `open_dir` is the
/// direction (dx, dz) players approach from; the backdrop sits opposite it.
pub fn build_portal(
    kind: TransitionKind,
    tile: TilePos,
    surface: f32,
    open_dir: (f32, f32),
    out: &mut Vec<Vertex>,
) {
    let base = [tile.x as f32 + 0.5, surface, tile.y as f32 + 0.5];
    let (ox, oz) = open_dir;
    // Yaw so the opening faces `open_dir` (local +z is the open side).
    let yaw = oz.atan2(ox) - std::f32::consts::FRAC_PI_2;
    match kind {
        TransitionKind::Cave => {
            boxed(
                base,
                yaw,
                [0.0, 0.0, -0.3],
                [0.96, 1.3, 0.35],
                [0.04, 0.03, 0.04, 1.0],
                out,
            );
            for dx in [-0.42, 0.42] {
                boxed(
                    base,
                    yaw,
                    [dx, 0.0, -0.1],
                    [0.16, 1.25, 0.5],
                    STONE_DARK,
                    out,
                );
            }
            boxed(
                base,
                yaw,
                [0.0, 1.2, -0.1],
                [1.0, 0.25, 0.5],
                STONE_DARK,
                out,
            );
            boxed(
                base,
                yaw,
                [0.0, -0.02, 0.0],
                [0.9, 0.03, 0.9],
                [0.22, 0.20, 0.20, 1.0],
                out,
            );
        }
        TransitionKind::Door => {
            boxed(
                base,
                yaw,
                [0.0, 0.0, -0.3],
                [0.9, 1.5, 0.3],
                [0.05, 0.04, 0.05, 1.0],
                out,
            );
            for dx in [-0.4, 0.4] {
                boxed(
                    base,
                    yaw,
                    [dx, 0.0, -0.15],
                    [0.12, 1.5, 0.35],
                    WOOD_DARK,
                    out,
                );
            }
            boxed(
                base,
                yaw,
                [0.0, 1.45, -0.15],
                [0.92, 0.12, 0.35],
                WOOD_DARK,
                out,
            );
            boxed(base, yaw, [0.0, 0.0, -0.32], [0.62, 1.4, 0.06], IRON, out);
            boxed(
                base,
                yaw,
                [0.0, 0.5, -0.32],
                [0.66, 0.06, 0.08],
                scale(IRON, 0.7),
                out,
            );
            boxed(
                base,
                yaw,
                [0.0, 1.0, -0.32],
                [0.66, 0.06, 0.08],
                scale(IRON, 0.7),
                out,
            );
        }
        TransitionKind::StairsUp => {
            for i in 0..4 {
                let h = 0.14 * (i + 1) as f32;
                boxed(
                    base,
                    yaw,
                    [0.0, 0.0, 0.3 - i as f32 * 0.22],
                    [0.9, h, 0.22],
                    scale(STONE, 1.0 + i as f32 * 0.04),
                    out,
                );
            }
        }
        TransitionKind::StairsDown => {
            boxed(
                base,
                yaw,
                [0.0, -0.05, 0.0],
                [0.9, 0.04, 0.9],
                [0.03, 0.03, 0.04, 1.0],
                out,
            );
            for i in 0..3 {
                boxed(
                    base,
                    yaw,
                    [0.0, -0.05, 0.35 - i as f32 * 0.25],
                    [0.9, 0.06, 0.16],
                    scale(STONE_DARK, 1.0 - i as f32 * 0.15),
                    out,
                );
            }
        }
        TransitionKind::Path => {
            boxed(
                base,
                0.0,
                [0.0, -0.02, 0.0],
                [0.95, 0.03, 0.95],
                [0.70, 0.66, 0.55, 1.0],
                out,
            );
            boxed(base, yaw, [0.35, 0.0, 0.0], [0.14, 0.5, 0.14], STONE, out);
            boxed(base, yaw, [-0.35, 0.0, 0.0], [0.14, 0.5, 0.14], STONE, out);
        }
        TransitionKind::Portal => {
            boxed(
                base,
                0.0,
                [0.0, -0.02, 0.0],
                [0.9, 0.05, 0.9],
                [0.58, 0.22, 0.78, 1.0],
                out,
            );
        }
    }
}

/// Placeholder body for NPCs without a model. `archetype` comes from the
/// NPC definition; unknown archetypes get a neutral humanoid.
pub fn build_npc_placeholder(
    archetype: Option<&str>,
    footprint: NpcFootprint,
    base: [f32; 3],
    yaw: f32,
    alive: bool,
    light: f32,
    out: &mut Vec<Vertex>,
) {
    let start = out.len();
    let fp = (footprint.width.max(footprint.height) as f32).max(1.0);
    let s = 0.8 + (fp - 1.0) * 0.5; // 2x2 NPCs are big
    match archetype.unwrap_or("") {
        "beast" => quadruped(
            base,
            yaw,
            s,
            [0.55, 0.36, 0.22, 1.0],
            [0.72, 0.62, 0.50, 1.0],
            out,
        ),
        "vermin" => {
            quadruped(
                base,
                yaw,
                0.45,
                [0.42, 0.40, 0.40, 1.0],
                [0.55, 0.5, 0.5, 1.0],
                out,
            );
            boxed(
                base,
                yaw,
                [0.0, 0.08, -0.5 * 0.45],
                [0.05, 0.05, 0.4],
                [0.7, 0.55, 0.55, 1.0],
                out,
            );
        }
        "lurker" => {
            boxed(
                base,
                yaw,
                [0.0, 0.0, 0.0],
                [1.1 * s, 0.35, 1.3 * s],
                [0.78, 0.76, 0.70, 1.0],
                out,
            );
            boxed(
                base,
                yaw,
                [0.0, 0.3, 0.45 * s],
                [0.5, 0.3, 0.5],
                [0.85, 0.82, 0.75, 1.0],
                out,
            );
            for i in 0..4 {
                let z = -0.45 + i as f32 * 0.3;
                for dx in [-0.6 * s, 0.6 * s] {
                    boxed(
                        base,
                        yaw,
                        [dx, 0.0, z],
                        [0.08, 0.4, 0.08],
                        [0.6, 0.58, 0.5, 1.0],
                        out,
                    );
                }
            }
            for dx in [-0.12, 0.12] {
                boxed(
                    base,
                    yaw,
                    [dx, 0.42, 0.68 * s],
                    [0.06, 0.06, 0.04],
                    [0.9, 0.2, 0.2, 1.0],
                    out,
                );
            }
        }
        "wisp" => {
            for (i, (dx, dy, dz)) in [(0.0, 0.5, 0.0), (0.25, 0.9, 0.1), (-0.2, 0.75, -0.15)]
                .iter()
                .enumerate()
            {
                let sz = 0.5 - i as f32 * 0.12;
                boxed(
                    base,
                    yaw,
                    [*dx * s, *dy * s, *dz * s],
                    [sz * s, sz * s, sz * s],
                    scale(GLOW_TEAL, 0.9 + i as f32 * 0.1),
                    out,
                );
            }
        }
        "brute" => humanoid(
            base,
            yaw,
            s * 1.25,
            [0.42, 0.30, 0.22, 1.0],
            [0.75, 0.62, 0.50, 1.0],
            Some([0.5, 0.42, 0.32, 1.0]),
            out,
        ),
        "warlord" => {
            humanoid(
                base,
                yaw,
                s * 1.3,
                [0.30, 0.28, 0.30, 1.0],
                [0.72, 0.60, 0.48, 1.0],
                Some([0.55, 0.12, 0.12, 1.0]),
                out,
            );
            boxed(
                base,
                yaw,
                [0.0, 2.05 * s * 1.3 * 0.85, 0.0],
                [0.34, 0.16, 0.34],
                GOLD,
                out,
            );
        }
        "bandit" => humanoid(
            base,
            yaw,
            s,
            [0.40, 0.30, 0.20, 1.0],
            [0.75, 0.62, 0.50, 1.0],
            Some([0.62, 0.15, 0.12, 1.0]),
            out,
        ),
        "guide" => humanoid(
            base,
            yaw,
            s,
            [0.45, 0.45, 0.42, 1.0],
            [0.78, 0.66, 0.55, 1.0],
            Some([0.85, 0.85, 0.85, 1.0]),
            out,
        ),
        "trader" => humanoid(
            base,
            yaw,
            s,
            [0.48, 0.30, 0.52, 1.0],
            [0.78, 0.66, 0.55, 1.0],
            None,
            out,
        ),
        "smith" => humanoid(
            base,
            yaw,
            s,
            [0.25, 0.25, 0.27, 1.0],
            [0.70, 0.58, 0.48, 1.0],
            Some([0.35, 0.33, 0.30, 1.0]),
            out,
        ),
        "fisher" => humanoid(
            base,
            yaw,
            s,
            [0.28, 0.40, 0.55, 1.0],
            [0.75, 0.62, 0.50, 1.0],
            Some([0.80, 0.72, 0.40, 1.0]),
            out,
        ),
        _ => humanoid(
            base,
            yaw,
            s,
            [0.55, 0.30, 0.30, 1.0],
            [0.75, 0.62, 0.50, 1.0],
            None,
            out,
        ),
    }
    let f = if alive { light } else { light * 0.5 };
    if f < 1.0 {
        for v in &mut out[start..] {
            v.color = scale(v.color, f);
        }
    }
}

fn humanoid(
    base: [f32; 3],
    yaw: f32,
    s: f32,
    cloth: Color,
    skin: Color,
    hat: Option<Color>,
    out: &mut Vec<Vertex>,
) {
    // legs, torso, arms, head — proportions of a 1.8-unit figure at s = 1.
    let h = 1.8 * s;
    let legs = h * 0.45;
    let torso = h * 0.35;
    let head = h * 0.2;
    for dx in [-0.12 * s, 0.12 * s] {
        boxed(
            base,
            yaw,
            [dx, 0.0, 0.0],
            [0.2 * s, legs, 0.22 * s],
            scale(cloth, 0.75),
            out,
        );
    }
    boxed(
        base,
        yaw,
        [0.0, legs, 0.0],
        [0.5 * s, torso, 0.28 * s],
        cloth,
        out,
    );
    for dx in [-0.34 * s, 0.34 * s] {
        boxed(
            base,
            yaw,
            [dx, legs + torso * 0.15, 0.0],
            [0.14 * s, torso * 0.85, 0.16 * s],
            skin,
            out,
        );
    }
    boxed(
        base,
        yaw,
        [0.0, legs + torso, 0.0],
        [0.3 * s, head, 0.3 * s],
        skin,
        out,
    );
    if let Some(c) = hat {
        boxed(
            base,
            yaw,
            [0.0, legs + torso + head * 0.8, 0.0],
            [0.34 * s, head * 0.3, 0.34 * s],
            c,
            out,
        );
    }
}

fn quadruped(base: [f32; 3], yaw: f32, s: f32, body: Color, belly: Color, out: &mut Vec<Vertex>) {
    boxed(
        base,
        yaw,
        [0.0, 0.35 * s, 0.0],
        [0.5 * s, 0.45 * s, 1.1 * s],
        body,
        out,
    );
    boxed(
        base,
        yaw,
        [0.0, 0.55 * s, 0.6 * s],
        [0.4 * s, 0.35 * s, 0.4 * s],
        body,
        out,
    );
    boxed(
        base,
        yaw,
        [0.0, 0.5 * s, 0.85 * s],
        [0.25 * s, 0.2 * s, 0.25 * s],
        belly,
        out,
    );
    for (dx, dz) in [(-0.18, -0.4), (0.18, -0.4), (-0.18, 0.35), (0.18, 0.35)] {
        boxed(
            base,
            yaw,
            [dx * s, 0.0, dz * s],
            [0.12 * s, 0.38 * s, 0.14 * s],
            scale(body, 0.8),
            out,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmmo_common::{ObjectId, PropModel};

    fn def(model: PropModel) -> ObjectDef {
        ObjectDef {
            id: ObjectId(1),
            name: "Copper Vein".into(),
            model: Some(model),
            ..Default::default()
        }
    }

    #[test]
    fn every_prop_model_builds_geometry() {
        for m in [
            PropModel::Tree,
            PropModel::Oak,
            PropModel::Pine,
            PropModel::DeadTree,
            PropModel::Bush,
            PropModel::Stump,
            PropModel::Rock,
            PropModel::Boulder,
            PropModel::OreVein,
            PropModel::Crystal,
            PropModel::Pool,
            PropModel::Anvil,
            PropModel::Furnace,
            PropModel::Workbench,
            PropModel::BankBooth,
            PropModel::MarketStall,
            PropModel::Campfire,
            PropModel::Crate,
            PropModel::Barrel,
            PropModel::Sign,
            PropModel::Fence,
            PropModel::Lamp,
            PropModel::Well,
            PropModel::Tent,
            PropModel::Rubble,
            PropModel::Mushroom,
            PropModel::Cactus,
            PropModel::Statue,
        ] {
            let mut out = Vec::new();
            build_prop(
                &def(m),
                [0.5, 0.1, 0.5],
                TilePos::new(3, 4),
                false,
                1.0,
                &mut out,
            );
            assert!(!out.is_empty(), "{m:?} produced no geometry");
            assert_eq!(out.len() % 3, 0, "{m:?} not triangles");
            let mut depleted = Vec::new();
            build_prop(
                &def(m),
                [0.5, 0.1, 0.5],
                TilePos::new(3, 4),
                true,
                1.0,
                &mut depleted,
            );
            assert!(!depleted.is_empty(), "{m:?} depleted produced no geometry");
        }
    }

    #[test]
    fn prop_geometry_is_placed_at_base() {
        let mut out = Vec::new();
        build_prop(
            &def(PropModel::Tree),
            [10.5, 0.2, 20.5],
            TilePos::new(10, 20),
            false,
            1.0,
            &mut out,
        );
        let min_y = out.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min);
        assert!(
            (min_y - 0.2).abs() < 1e-4,
            "prop should start at the surface, got {min_y}"
        );
        let xs: Vec<f32> = out.iter().map(|v| v.position[0]).collect();
        assert!(xs.iter().all(|x| (*x - 10.5).abs() < 1.5));
    }

    #[test]
    fn portals_and_placeholders_build() {
        for k in [
            TransitionKind::Cave,
            TransitionKind::Door,
            TransitionKind::StairsUp,
            TransitionKind::StairsDown,
            TransitionKind::Path,
            TransitionKind::Portal,
        ] {
            let mut out = Vec::new();
            build_portal(k, TilePos::new(1, 1), 0.1, (0.0, 1.0), &mut out);
            assert!(!out.is_empty(), "{k:?}");
        }
        for a in [
            "beast", "vermin", "lurker", "wisp", "brute", "warlord", "bandit", "guide", "nope",
        ] {
            let mut out = Vec::new();
            build_npc_placeholder(
                Some(a),
                NpcFootprint::default(),
                [0.5, 0.0, 0.5],
                0.3,
                true,
                1.0,
                &mut out,
            );
            assert!(!out.is_empty(), "{a}");
        }
    }

    #[test]
    fn dim_light_darkens_colours() {
        let mut bright = Vec::new();
        let mut dim = Vec::new();
        build_prop(
            &def(PropModel::Crate),
            [0.5, 0.0, 0.5],
            TilePos::new(0, 0),
            false,
            1.0,
            &mut bright,
        );
        build_prop(
            &def(PropModel::Crate),
            [0.5, 0.0, 0.5],
            TilePos::new(0, 0),
            false,
            0.5,
            &mut dim,
        );
        assert!(dim[0].color[0] < bright[0].color[0]);
    }
}
