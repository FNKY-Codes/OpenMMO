use egui::{RichText, Ui};
use openmmo_common::{
    ContentPack, EntityKind, EquipSlot, Equipment, InventorySlot, ItemId, RefinementRecipe, Skill,
    WorldEntity,
};

use super::theme::{self, ACCENT, TEXT_MUTED};
use super::SidePanelTab;

pub fn item_name(content: &ContentPack, item_id: ItemId) -> &str {
    content
        .item(item_id)
        .map(|i| i.name.as_str())
        .unwrap_or("Unknown")
}

pub fn item_label(content: &ContentPack, slot: &InventorySlot) -> String {
    format!(
        "{} x{}",
        item_name(content, slot.item_id),
        slot.quantity
    )
}

pub fn item_label_by_id(content: &ContentPack, item_id: ItemId, quantity: u32) -> String {
    format!("{} x{quantity}", item_name(content, item_id))
}

pub fn entity_display_name(content: &ContentPack, entity: &WorldEntity) -> (String, Option<String>) {
    match &entity.kind {
        EntityKind::Player { name, .. } => (name.clone(), Some("Player".into())),
        EntityKind::Npc { name, aggro_range, .. } => {
            let subtitle = if *aggro_range == 0 {
                "NPC"
            } else {
                "Hostile NPC"
            };
            (name.clone(), Some(subtitle.into()))
        }
        EntityKind::Boss { name, .. } => (name.clone(), Some("Boss".into())),
        EntityKind::Object { object_id, .. } => {
            let title = content
                .object(*object_id)
                .map(|o| o.name.clone())
                .unwrap_or_else(|| "Unknown Object".into());
            (title, Some("Harvestable".into()))
        }
        EntityKind::GroundItem {
            item_id,
            quantity,
            ..
        } => (
            item_label_by_id(content, *item_id, *quantity),
            Some("Item".into()),
        ),
    }
}

pub fn tab_bar(ui: &mut Ui, active_tab: &mut Option<SidePanelTab>) {
    ui.horizontal_wrapped(|ui| {
        for (tab, label) in [
            (SidePanelTab::Inventory, "Inv"),
            (SidePanelTab::Bank, "Bank"),
            (SidePanelTab::Skills, "Skills"),
            (SidePanelTab::Quests, "Quests"),
            (SidePanelTab::Market, "Market"),
            (SidePanelTab::Friends, "Friends"),
        ] {
            let selected = *active_tab == Some(tab);
            if ui.selectable_label(selected, label).clicked() {
                *active_tab = if selected { None } else { Some(tab) };
            }
        }
    });
}

pub fn hp_bar(ui: &mut Ui, hp: u32, max_hp: u32) {
    let ratio = if max_hp > 0 {
        hp as f32 / max_hp as f32
    } else {
        0.0
    };
    let bar = egui::ProgressBar::new(ratio)
        .text(format!("HP {hp}/{max_hp}"))
        .fill(theme::hp_color(ratio));
    ui.add(bar);
}

/// Compact HP bar for rendering above entities in the world.
pub fn world_hp_bar(ui: &mut Ui, hp: u32, max_hp: u32) {
    let ratio = if max_hp > 0 {
        hp as f32 / max_hp as f32
    } else {
        0.0
    };
    let bar_width = 52.0;
    let bar_height = 6.0;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(bar_width, bar_height), egui::Sense::hover());
    let fill_width = bar_width * ratio;
    ui.painter()
        .rect_filled(rect, 1.0, egui::Color32::from_rgba_unmultiplied(20, 20, 20, 180));
    if fill_width > 0.0 {
        let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_width, bar_height));
        ui.painter().rect_filled(fill_rect, 1.0, theme::hp_color(ratio));
    }
    ui.painter().rect_stroke(
        rect,
        1.0,
        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40)),
    );
}

pub fn status_row(ui: &mut Ui, connected: bool, status: &str) {
    ui.horizontal(|ui| {
        let dot = if connected { theme::ONLINE } else { theme::OFFLINE };
        let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, dot);
        ui.label(RichText::new(status).color(TEXT_MUTED).small());
    });
}

pub fn item_row(
    ui: &mut Ui,
    content: &ContentPack,
    slot: &InventorySlot,
    slot_index: usize,
    show_equip: bool,
) -> ItemRowAction {
    let mut action = ItemRowAction::None;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("#{}: {}", slot_index, item_label(content, slot))).size(13.0),
        );
        if show_equip && ui.small_button("Equip").clicked() {
            action = ItemRowAction::Equip(slot_index);
        }
        if ui.small_button("Drop").clicked() {
            action = ItemRowAction::Drop(slot_index);
        }
    });
    action
}

pub enum ItemRowAction {
    None,
    Drop(usize),
    Equip(usize),
}

pub fn equipment_strip(
    ui: &mut Ui,
    content: &ContentPack,
    equipment: &Equipment,
) -> Option<EquipSlot> {
    let mut unequip = None;
    ui.label(RichText::new("Equipment").strong().color(ACCENT));
    for (label, slot, equip_slot) in [
        ("Head", &equipment.head, EquipSlot::Head),
        ("Body", &equipment.body, EquipSlot::Body),
        ("Legs", &equipment.legs, EquipSlot::Legs),
        ("Weapon", &equipment.weapon, EquipSlot::Weapon),
        ("Shield", &equipment.shield, EquipSlot::Shield),
    ] {
        ui.horizontal(|ui| {
            let text = slot
                .as_ref()
                .map(|s| item_label(content, s))
                .unwrap_or_else(|| "—".to_string());
            ui.label(format!("{label}: {text}"));
            if slot.is_some() && ui.small_button(format!("Unequip {label}")).clicked() {
                unequip = Some(equip_slot);
            }
        });
    }
    unequip
}

pub fn skill_row(ui: &mut Ui, skill: Skill, level: u32, xp: u64) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(skill.name()).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("Lv {level}")).color(ACCENT));
            ui.label(RichText::new(format!("{xp} xp")).color(TEXT_MUTED).small());
        });
    });
}

pub fn recipe_tooltip(ui: &mut Ui, content: &ContentPack, recipe: &RefinementRecipe) {
    ui.label(format!("Requires Fabrication Lv {}", recipe.fabrication_level));
    for input in &recipe.inputs {
        ui.label(format!(
            "  - {}",
            item_label_by_id(content, input.item_id, input.quantity)
        ));
    }
    ui.label(format!(
        "  → {}",
        item_label_by_id(content, recipe.output, recipe.output_qty)
    ));
}
