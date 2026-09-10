//! Item grids: the inventory, bank, shop and trade views all share this.
//! Each slot draws a procedural icon, a quantity badge and an optional
//! caption (price), supports a primary click, a right-click menu and a
//! hover tooltip with the item's examine text and stats.

use egui::{Color32, Rect, RichText, Sense, Stroke, Ui, Vec2};
use openmmo_common::{ContentPack, InventorySlot, ItemDef};

use super::icons::paint_item_icon;
use super::theme::{self, SLOT_BG, SLOT_BORDER};

pub struct GridStyle {
    pub cols: usize,
    pub slot: f32,
    /// Extra height under each slot for a caption line.
    pub caption: bool,
}

impl GridStyle {
    pub fn inventory() -> Self {
        Self {
            cols: 4,
            slot: 44.0,
            caption: false,
        }
    }
    pub fn wide(cols: usize) -> Self {
        Self {
            cols,
            slot: 40.0,
            caption: false,
        }
    }
    pub fn shop(cols: usize) -> Self {
        Self {
            cols,
            slot: 44.0,
            caption: true,
        }
    }
}

/// Draw a grid of slots. `caption(i, slot)` supplies text under a slot;
/// `primary(i, slot)` maps a left click to an action; `menu(ui, i, slot)`
/// fills the right-click menu and returns an action when one is chosen.
#[allow(clippy::too_many_arguments)]
pub fn item_grid<A>(
    ui: &mut Ui,
    id: impl std::hash::Hash,
    style: &GridStyle,
    slots: &[Option<InventorySlot>],
    content: &ContentPack,
    caption: &dyn Fn(usize, &InventorySlot) -> Option<String>,
    primary: &dyn Fn(usize, &InventorySlot) -> Option<A>,
    menu: &mut dyn FnMut(&mut Ui, usize, &InventorySlot) -> Option<A>,
) -> Option<A> {
    let mut action = None;
    let base_id = egui::Id::new(id);
    let cell_h = if style.caption {
        style.slot + 14.0
    } else {
        style.slot
    };
    let gap = 3.0;
    let rows = slots.len().div_ceil(style.cols);
    let (grid_rect, _) = ui.allocate_exact_size(
        Vec2::new(
            style.cols as f32 * (style.slot + gap),
            rows as f32 * (cell_h + gap),
        ),
        Sense::hover(),
    );
    for (i, slot) in slots.iter().enumerate() {
        let col = (i % style.cols) as f32;
        let row = (i / style.cols) as f32;
        let min = grid_rect.min + Vec2::new(col * (style.slot + gap), row * (cell_h + gap));
        let rect = Rect::from_min_size(min, Vec2::splat(style.slot));
        let resp = ui.interact(rect, base_id.with(i), Sense::click());
        let painter = ui.painter();
        let bg = if resp.hovered() {
            Color32::from_rgb(78, 62, 42)
        } else {
            SLOT_BG
        };
        painter.rect_filled(rect, 3.0, bg);
        painter.rect_stroke(rect, 3.0, Stroke::new(1.0, SLOT_BORDER));
        let Some(s) = slot else { continue };
        let Some(def) = content.item(s.item_id) else {
            continue;
        };
        paint_item_icon(painter, rect, def);
        if s.quantity > 1 || def.stackable {
            painter.text(
                rect.right_top() + Vec2::new(-3.0, 2.0),
                egui::Align2::RIGHT_TOP,
                short_qty(s.quantity),
                egui::FontId::proportional(11.0),
                if s.quantity >= 10_000 {
                    theme::GOOD
                } else {
                    Color32::from_rgb(255, 250, 200)
                },
            );
        }
        if style.caption {
            if let Some(text) = caption(i, s) {
                painter.text(
                    rect.center_bottom() + Vec2::new(0.0, 2.0),
                    egui::Align2::CENTER_TOP,
                    text,
                    egui::FontId::proportional(11.0),
                    theme::ACCENT,
                );
            }
        }
        let resp = resp.on_hover_ui(|ui| item_tooltip(ui, def, s.quantity));
        if resp.clicked() {
            if let Some(a) = primary(i, s) {
                action = Some(a);
            }
        }
        resp.context_menu(|ui| {
            ui.label(RichText::new(&def.name).color(theme::ACCENT).strong());
            if let Some(a) = menu(ui, i, s) {
                action = Some(a);
                ui.close_menu();
            }
        });
    }
    action
}

pub fn short_qty(q: u32) -> String {
    if q >= 10_000_000 {
        format!("{}M", q / 1_000_000)
    } else if q >= 100_000 {
        format!("{}K", q / 1000)
    } else {
        q.to_string()
    }
}

pub fn item_tooltip(ui: &mut Ui, def: &ItemDef, quantity: u32) {
    ui.set_max_width(240.0);
    ui.label(RichText::new(&def.name).color(theme::ACCENT).strong());
    if !def.examine.is_empty() {
        ui.label(
            RichText::new(&def.examine)
                .color(theme::TEXT_MUTED)
                .italics(),
        );
    }
    let mut stats = Vec::new();
    if def.prowess_bonus != 0 {
        stats.push(format!("Prowess +{}", def.prowess_bonus));
    }
    if def.fortitude_bonus != 0 {
        stats.push(format!("Fortitude +{}", def.fortitude_bonus));
    }
    if def.electrics_bonus != 0 {
        stats.push(format!("Electrics +{}", def.electrics_bonus));
    }
    if def.heals > 0 {
        stats.push(format!("Heals {}", def.heals));
    }
    if def.equip_level > 1 {
        stats.push(format!("Requires Combat {}", def.equip_level));
    }
    if !stats.is_empty() {
        ui.label(stats.join("  ·  "));
    }
    let mut foot = format!("Value {} Scrap", def.alchemy_value);
    if quantity > 1 {
        foot.push_str(&format!("  ·  x{quantity}"));
    }
    ui.label(RichText::new(foot).color(theme::TEXT_MUTED).small());
}

/// A single equipment slot (used by the equipment panel).
pub fn equip_slot(
    ui: &mut Ui,
    id: impl std::hash::Hash,
    label: &str,
    slot: &Option<InventorySlot>,
    content: &ContentPack,
) -> egui::Response {
    let size = 44.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let painter = ui.painter();
    let bg = if resp.hovered() {
        Color32::from_rgb(78, 62, 42)
    } else {
        SLOT_BG
    };
    painter.rect_filled(rect, 3.0, bg);
    painter.rect_stroke(rect, 3.0, Stroke::new(1.0, SLOT_BORDER));
    let _ = id;
    match slot
        .as_ref()
        .and_then(|s| content.item(s.item_id).map(|d| (d, s.quantity)))
    {
        Some((def, qty)) => {
            paint_item_icon(painter, rect, def);
            resp.on_hover_ui(|ui| item_tooltip(ui, def, qty))
        }
        None => {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(10.0),
                theme::TEXT_MUTED,
            );
            resp
        }
    }
}
