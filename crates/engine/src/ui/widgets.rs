use egui::{RichText, Ui};
use openmmo_common::{
    ContentPack, EntityKind, InventorySlot, ItemId, RefinementRecipe, WorldEntity,
};

use super::theme;

pub fn item_name(content: &ContentPack, item_id: ItemId) -> &str {
    content
        .item(item_id)
        .map(|i| i.name.as_str())
        .unwrap_or("Unknown")
}

pub fn item_label(content: &ContentPack, slot: &InventorySlot) -> String {
    format!("{} x{}", item_name(content, slot.item_id), slot.quantity)
}

pub fn item_label_by_id(content: &ContentPack, item_id: ItemId, quantity: u32) -> String {
    format!("{} x{quantity}", item_name(content, item_id))
}

/// Name and a short descriptor for an entity ("Mutant Rat", "Level 2").
pub fn entity_display_name(
    content: &ContentPack,
    entity: &WorldEntity,
) -> (String, Option<String>) {
    match &entity.kind {
        EntityKind::Player { name, .. } => (name.clone(), Some("Player".into())),
        EntityKind::Npc {
            name,
            npc_id,
            aggro_range,
            ..
        } => {
            let def = content.npc(*npc_id);
            let subtitle = match def {
                Some(d) if d.prowess > 0 || *aggro_range > 0 => {
                    format!("Level {}", d.combat_level())
                }
                _ => "NPC".to_string(),
            };
            (name.clone(), Some(subtitle))
        }
        EntityKind::Boss { name, .. } => (name.clone(), Some("Boss".into())),
        EntityKind::Object {
            object_id,
            depleted,
            ..
        } => {
            let def = content.object(*object_id);
            let title = def
                .map(|o| o.name.clone())
                .unwrap_or_else(|| "Unknown Object".into());
            let subtitle = match def {
                Some(d) if d.station.is_some() => "Station".to_string(),
                Some(d) if d.harvest_tag.is_some() => {
                    if *depleted {
                        "Depleted".to_string()
                    } else {
                        format!("Scavenging {}", d.scavenging_level)
                    }
                }
                _ => "Scenery".to_string(),
            };
            (title, Some(subtitle))
        }
        EntityKind::GroundItem {
            item_id, quantity, ..
        } => (
            item_label_by_id(content, *item_id, *quantity),
            Some("Item".into()),
        ),
    }
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
    let (rect, _) = ui.allocate_exact_size(egui::vec2(bar_width, bar_height), egui::Sense::hover());
    let fill_width = bar_width * ratio;
    ui.painter().rect_filled(
        rect,
        1.0,
        egui::Color32::from_rgba_unmultiplied(20, 20, 20, 180),
    );
    if fill_width > 0.0 {
        let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_width, bar_height));
        ui.painter()
            .rect_filled(fill_rect, 1.0, theme::hp_color(ratio));
    }
    ui.painter().rect_stroke(
        rect,
        1.0,
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40),
        ),
    );
}

pub fn recipe_tooltip(ui: &mut Ui, content: &ContentPack, recipe: &RefinementRecipe) {
    ui.label(
        RichText::new(format!("Fabrication {}", recipe.fabrication_level)).color(theme::TEXT_MUTED),
    );
    for input in &recipe.inputs {
        ui.label(format!(
            "  {}",
            item_label_by_id(content, input.item_id, input.quantity)
        ));
    }
    ui.label(format!(
        "→ {}",
        item_label_by_id(content, recipe.output, recipe.output_qty)
    ));
}
