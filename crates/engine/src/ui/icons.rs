//! Procedural item and tab icons. There is no icon art in the repo, so each
//! item is drawn from a handful of shapes chosen by its category and tinted
//! by material tier. Good enough to tell a bronze helm from an iron blade at
//! a glance, and cheap to extend.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use openmmo_common::{EquipSlot, ItemDef, MaterialTier, ToolTag};

use super::panels::PanelTab;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Blade,
    Club,
    Helm,
    Plate,
    Legs,
    Shield,
    Axe,
    Pick,
    Rod,
    Fish,
    CookedFish,
    Mushroom,
    Ore,
    Coal,
    Ingot,
    Log,
    Plank,
    Coins,
    Orb,
    Crown,
    Mask,
    Generic,
}

pub fn icon_kind(def: &ItemDef) -> IconKind {
    let n = def.name.to_ascii_lowercase();
    match def.equip_slot {
        Some(EquipSlot::Head) if n.contains("mask") => return IconKind::Mask,
        Some(EquipSlot::Head) => return IconKind::Helm,
        Some(EquipSlot::Body) => return IconKind::Plate,
        Some(EquipSlot::Legs) => return IconKind::Legs,
        Some(EquipSlot::Shield) => return IconKind::Shield,
        Some(EquipSlot::Weapon) if n.contains("club") => return IconKind::Club,
        Some(EquipSlot::Weapon) => return IconKind::Blade,
        None => {}
    }
    match def.tool_tag {
        Some(ToolTag::Axe) => return IconKind::Axe,
        Some(ToolTag::Pick) => return IconKind::Pick,
        Some(ToolTag::Rod) => return IconKind::Rod,
        Some(ToolTag::Knife) => return IconKind::Blade,
        None => {}
    }
    if n.contains("mushroom") {
        IconKind::Mushroom
    } else if n.contains("cooked") {
        IconKind::CookedFish
    } else if n.contains("raw")
        || n.contains("fish")
        || n.contains("minnow")
        || n.contains("trout")
        || n.contains("pike")
    {
        IconKind::Fish
    } else if n.contains("coal") {
        IconKind::Coal
    } else if n.contains("ore") {
        IconKind::Ore
    } else if n.contains("ingot") {
        IconKind::Ingot
    } else if n.contains("plank") {
        IconKind::Plank
    } else if n.contains("timber") || n.contains("log") {
        IconKind::Log
    } else if n == "scrap" || n.contains("coin") {
        IconKind::Coins
    } else if n.contains("essence") {
        IconKind::Orb
    } else if n.contains("trophy") || n.contains("crown") {
        IconKind::Crown
    } else {
        IconKind::Generic
    }
}

pub fn tier_color(tier: Option<MaterialTier>) -> Color32 {
    match tier {
        Some(MaterialTier::Scrap) => Color32::from_rgb(140, 120, 100),
        Some(MaterialTier::Bronze) => Color32::from_rgb(196, 122, 70),
        Some(MaterialTier::Iron) => Color32::from_rgb(150, 152, 158),
        Some(MaterialTier::Steel) => Color32::from_rgb(200, 210, 225),
        Some(MaterialTier::Mithral) => Color32::from_rgb(90, 130, 220),
        Some(MaterialTier::Wood) => Color32::from_rgb(180, 130, 76),
        Some(MaterialTier::Stone) => Color32::from_rgb(130, 128, 120),
        Some(MaterialTier::Cloth) => Color32::from_rgb(200, 170, 140),
        Some(MaterialTier::Leather) => Color32::from_rgb(130, 90, 50),
        Some(MaterialTier::Food) => Color32::from_rgb(230, 150, 110),
        Some(MaterialTier::Misc) => Color32::from_rgb(170, 120, 220),
        None => Color32::from_rgb(170, 160, 140),
    }
}

fn darker(c: Color32, f: f32) -> Color32 {
    Color32::from_rgb(
        (c.r() as f32 * f) as u8,
        (c.g() as f32 * f) as u8,
        (c.b() as f32 * f) as u8,
    )
}

fn poly(painter: &Painter, pts: &[(f32, f32)], rect: Rect, fill: Color32, outline: Color32) {
    let points: Vec<Pos2> = pts
        .iter()
        .map(|(x, y)| {
            Pos2::new(
                rect.min.x + x * rect.width(),
                rect.min.y + y * rect.height(),
            )
        })
        .collect();
    painter.add(Shape::convex_polygon(
        points,
        fill,
        Stroke::new(1.0, outline),
    ));
}

fn seg(painter: &Painter, a: (f32, f32), b: (f32, f32), rect: Rect, width: f32, color: Color32) {
    let p = |(x, y): (f32, f32)| {
        Pos2::new(
            rect.min.x + x * rect.width(),
            rect.min.y + y * rect.height(),
        )
    };
    painter.line_segment([p(a), p(b)], Stroke::new(width, color));
}

fn dot(painter: &Painter, c: (f32, f32), r: f32, rect: Rect, color: Color32) {
    let p = Pos2::new(
        rect.min.x + c.0 * rect.width(),
        rect.min.y + c.1 * rect.height(),
    );
    painter.circle_filled(p, r * rect.width(), color);
}

/// Draw an item icon filling `rect` (square).
pub fn paint_item_icon(painter: &Painter, rect: Rect, def: &ItemDef) {
    let rect = rect.shrink(rect.width() * 0.12);
    let tint = tier_color(def.tier);
    let dark = darker(tint, 0.55);
    let wood = Color32::from_rgb(150, 105, 60);
    let s = rect.width();
    match icon_kind(def) {
        IconKind::Blade => {
            poly(
                painter,
                &[(0.62, 0.05), (0.78, 0.2), (0.35, 0.62), (0.2, 0.47)],
                rect,
                tint,
                dark,
            );
            seg(painter, (0.16, 0.6), (0.44, 0.88), rect, s * 0.09, dark);
            seg(painter, (0.12, 0.7), (0.4, 0.97), rect, s * 0.14, wood);
            seg(painter, (0.05, 0.52), (0.3, 0.77), rect, s * 0.08, dark);
        }
        IconKind::Club => {
            seg(painter, (0.2, 0.9), (0.55, 0.45), rect, s * 0.14, wood);
            poly(
                painter,
                &[(0.5, 0.15), (0.85, 0.35), (0.7, 0.62), (0.4, 0.5)],
                rect,
                tint,
                dark,
            );
        }
        IconKind::Helm => {
            poly(
                painter,
                &[
                    (0.15, 0.55),
                    (0.25, 0.22),
                    (0.5, 0.1),
                    (0.75, 0.22),
                    (0.85, 0.55),
                    (0.85, 0.85),
                    (0.15, 0.85),
                ],
                rect,
                tint,
                dark,
            );
            painter.rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::new(0.3 * s, 0.5 * s),
                    Vec2::new(0.4 * s, 0.12 * s),
                ),
                0.0,
                dark,
            );
        }
        IconKind::Plate => {
            poly(
                painter,
                &[
                    (0.1, 0.15),
                    (0.35, 0.08),
                    (0.65, 0.08),
                    (0.9, 0.15),
                    (0.8, 0.5),
                    (0.8, 0.9),
                    (0.2, 0.9),
                    (0.2, 0.5),
                ],
                rect,
                tint,
                dark,
            );
            seg(painter, (0.5, 0.15), (0.5, 0.88), rect, s * 0.05, dark);
        }
        IconKind::Legs => {
            poly(
                painter,
                &[
                    (0.2, 0.1),
                    (0.8, 0.1),
                    (0.85, 0.9),
                    (0.6, 0.9),
                    (0.5, 0.4),
                    (0.4, 0.9),
                    (0.15, 0.9),
                ],
                rect,
                tint,
                dark,
            );
        }
        IconKind::Shield => {
            poly(
                painter,
                &[
                    (0.15, 0.1),
                    (0.85, 0.1),
                    (0.85, 0.5),
                    (0.5, 0.92),
                    (0.15, 0.5),
                ],
                rect,
                tint,
                dark,
            );
            seg(painter, (0.5, 0.15), (0.5, 0.85), rect, s * 0.05, dark);
            seg(painter, (0.2, 0.35), (0.8, 0.35), rect, s * 0.05, dark);
        }
        IconKind::Axe => {
            seg(painter, (0.25, 0.92), (0.65, 0.3), rect, s * 0.12, wood);
            poly(
                painter,
                &[(0.55, 0.1), (0.9, 0.2), (0.85, 0.55), (0.5, 0.42)],
                rect,
                tint,
                dark,
            );
        }
        IconKind::Pick => {
            seg(painter, (0.3, 0.92), (0.65, 0.35), rect, s * 0.12, wood);
            poly(
                painter,
                &[
                    (0.3, 0.25),
                    (0.6, 0.12),
                    (0.92, 0.3),
                    (0.72, 0.42),
                    (0.6, 0.32),
                    (0.42, 0.42),
                ],
                rect,
                tint,
                dark,
            );
        }
        IconKind::Rod => {
            seg(painter, (0.15, 0.9), (0.8, 0.12), rect, s * 0.09, wood);
            seg(
                painter,
                (0.8, 0.12),
                (0.85, 0.6),
                rect,
                s * 0.04,
                Color32::from_rgb(220, 220, 220),
            );
            dot(
                painter,
                (0.85, 0.65),
                0.07,
                rect,
                Color32::from_rgb(200, 60, 60),
            );
        }
        IconKind::Fish | IconKind::CookedFish => {
            let body = if matches!(icon_kind(def), IconKind::CookedFish) {
                Color32::from_rgb(220, 150, 90)
            } else {
                Color32::from_rgb(140, 170, 190)
            };
            poly(
                painter,
                &[
                    (0.1, 0.5),
                    (0.35, 0.25),
                    (0.7, 0.3),
                    (0.85, 0.5),
                    (0.7, 0.7),
                    (0.35, 0.75),
                ],
                rect,
                body,
                darker(body, 0.6),
            );
            poly(
                painter,
                &[(0.85, 0.5), (0.98, 0.3), (0.98, 0.7)],
                rect,
                darker(body, 0.85),
                darker(body, 0.6),
            );
            dot(painter, (0.3, 0.45), 0.05, rect, Color32::BLACK);
        }
        IconKind::Mushroom => {
            seg(
                painter,
                (0.5, 0.9),
                (0.5, 0.5),
                rect,
                s * 0.16,
                Color32::from_rgb(230, 225, 210),
            );
            poly(
                painter,
                &[(0.12, 0.55), (0.3, 0.2), (0.7, 0.2), (0.88, 0.55)],
                rect,
                Color32::from_rgb(120, 220, 210),
                Color32::from_rgb(60, 140, 130),
            );
        }
        IconKind::Ore => {
            let rock = Color32::from_rgb(110, 105, 98);
            poly(
                painter,
                &[
                    (0.1, 0.8),
                    (0.25, 0.4),
                    (0.55, 0.25),
                    (0.9, 0.5),
                    (0.8, 0.85),
                ],
                rect,
                rock,
                darker(rock, 0.6),
            );
            let fleck = if def.name.to_ascii_lowercase().contains("iron") {
                Color32::from_rgb(190, 100, 60)
            } else {
                Color32::from_rgb(90, 190, 130)
            };
            dot(painter, (0.4, 0.55), 0.07, rect, fleck);
            dot(painter, (0.62, 0.45), 0.06, rect, fleck);
            dot(painter, (0.55, 0.7), 0.05, rect, fleck);
        }
        IconKind::Coal => {
            let c = Color32::from_rgb(35, 35, 40);
            poly(
                painter,
                &[
                    (0.1, 0.8),
                    (0.25, 0.4),
                    (0.55, 0.25),
                    (0.9, 0.5),
                    (0.8, 0.85),
                ],
                rect,
                c,
                Color32::from_rgb(90, 90, 100),
            );
            dot(
                painter,
                (0.45, 0.5),
                0.05,
                rect,
                Color32::from_rgb(120, 120, 130),
            );
        }
        IconKind::Ingot => {
            poly(
                painter,
                &[(0.1, 0.72), (0.25, 0.35), (0.9, 0.35), (0.75, 0.72)],
                rect,
                tint,
                dark,
            );
            seg(
                painter,
                (0.28, 0.42),
                (0.82, 0.42),
                rect,
                s * 0.04,
                darker(tint, 1.25),
            );
        }
        IconKind::Log => {
            painter.rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::new(0.1 * s, 0.35 * s),
                    Vec2::new(0.8 * s, 0.3 * s),
                ),
                3.0,
                wood,
            );
            dot(
                painter,
                (0.15, 0.5),
                0.14,
                rect,
                Color32::from_rgb(210, 180, 130),
            );
            dot(painter, (0.15, 0.5), 0.06, rect, wood);
        }
        IconKind::Plank => {
            painter.rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::new(0.1 * s, 0.25 * s),
                    Vec2::new(0.8 * s, 0.18 * s),
                ),
                2.0,
                tint,
            );
            painter.rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::new(0.1 * s, 0.55 * s),
                    Vec2::new(0.8 * s, 0.18 * s),
                ),
                2.0,
                darker(tint, 0.85),
            );
        }
        IconKind::Coins => {
            let gold = Color32::from_rgb(210, 175, 70);
            dot(painter, (0.35, 0.6), 0.2, rect, gold);
            dot(painter, (0.6, 0.5), 0.2, rect, darker(gold, 0.9));
            dot(painter, (0.5, 0.35), 0.18, rect, gold);
        }
        IconKind::Orb => {
            dot(
                painter,
                (0.5, 0.5),
                0.34,
                rect,
                Color32::from_rgb(120, 200, 230),
            );
            dot(
                painter,
                (0.42, 0.42),
                0.12,
                rect,
                Color32::from_rgb(220, 250, 255),
            );
        }
        IconKind::Crown => {
            let gold = Color32::from_rgb(220, 180, 60);
            poly(
                painter,
                &[
                    (0.1, 0.85),
                    (0.1, 0.3),
                    (0.3, 0.55),
                    (0.5, 0.15),
                    (0.7, 0.55),
                    (0.9, 0.3),
                    (0.9, 0.85),
                ],
                rect,
                gold,
                darker(gold, 0.6),
            );
            dot(
                painter,
                (0.5, 0.65),
                0.06,
                rect,
                Color32::from_rgb(200, 50, 60),
            );
        }
        IconKind::Mask => {
            let leather = Color32::from_rgb(120, 80, 45);
            poly(
                painter,
                &[
                    (0.1, 0.35),
                    (0.5, 0.2),
                    (0.9, 0.35),
                    (0.85, 0.75),
                    (0.5, 0.9),
                    (0.15, 0.75),
                ],
                rect,
                leather,
                darker(leather, 0.6),
            );
            dot(painter, (0.35, 0.5), 0.07, rect, Color32::BLACK);
            dot(painter, (0.65, 0.5), 0.07, rect, Color32::BLACK);
        }
        IconKind::Generic => {
            painter.rect_filled(rect.shrink(s * 0.2), 3.0, tint);
        }
    }
}

/// Tab-strip icons.
pub fn paint_tab_icon(painter: &Painter, rect: Rect, tab: PanelTab, active: bool) {
    let rect = rect.shrink(rect.width() * 0.2);
    let c = if active {
        super::theme::ACCENT
    } else {
        super::theme::TEXT_MUTED
    };
    let dark = darker(c, 0.6);
    let s = rect.width();
    match tab {
        PanelTab::Inventory => {
            painter.rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::new(0.1 * s, 0.3 * s),
                    Vec2::new(0.8 * s, 0.65 * s),
                ),
                3.0,
                c,
            );
            painter.rect_stroke(
                Rect::from_min_size(
                    rect.min + Vec2::new(0.3 * s, 0.12 * s),
                    Vec2::new(0.4 * s, 0.25 * s),
                ),
                2.0,
                Stroke::new(2.0, c),
            );
            painter.rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::new(0.35 * s, 0.5 * s),
                    Vec2::new(0.3 * s, 0.15 * s),
                ),
                1.0,
                dark,
            );
        }
        PanelTab::Equipment => {
            poly(
                painter,
                &[
                    (0.15, 0.55),
                    (0.25, 0.22),
                    (0.5, 0.1),
                    (0.75, 0.22),
                    (0.85, 0.55),
                    (0.85, 0.85),
                    (0.15, 0.85),
                ],
                rect,
                c,
                dark,
            );
        }
        PanelTab::Skills => {
            for (i, h) in [0.4, 0.65, 0.9].iter().enumerate() {
                let x = 0.12 + i as f32 * 0.3;
                painter.rect_filled(
                    Rect::from_min_size(
                        rect.min + Vec2::new(x * s, (1.0 - h) * s),
                        Vec2::new(0.22 * s, h * s),
                    ),
                    1.0,
                    c,
                );
            }
        }
        PanelTab::Quests => {
            painter.rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::new(0.2 * s, 0.1 * s),
                    Vec2::new(0.6 * s, 0.8 * s),
                ),
                2.0,
                c,
            );
            for y in [0.3, 0.5, 0.7] {
                seg(painter, (0.3, y), (0.7, y), rect, 1.5, dark);
            }
        }
        PanelTab::Social => {
            dot(painter, (0.35, 0.35), 0.15, rect, c);
            dot(painter, (0.68, 0.38), 0.12, rect, c);
            poly(
                painter,
                &[(0.1, 0.9), (0.2, 0.55), (0.5, 0.55), (0.6, 0.9)],
                rect,
                c,
                dark,
            );
            poly(
                painter,
                &[(0.55, 0.9), (0.6, 0.6), (0.85, 0.6), (0.92, 0.9)],
                rect,
                c,
                dark,
            );
        }
        PanelTab::Settings => {
            dot(painter, (0.5, 0.5), 0.32, rect, c);
            dot(painter, (0.5, 0.5), 0.14, rect, super::theme::PANEL_BG);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let (sn, cs) = a.sin_cos();
                seg(
                    painter,
                    (0.5 + cs * 0.3, 0.5 + sn * 0.3),
                    (0.5 + cs * 0.46, 0.5 + sn * 0.46),
                    rect,
                    s * 0.14,
                    c,
                );
            }
        }
    }
}
