//! Visual theme: a dark, warm palette in the spirit of classic RuneScape's
//! brown-and-gold interface, tuned for egui.

use egui::{Color32, Context, FontId, Margin, Rounding, Stroke, Visuals};

pub const PANEL_BG: Color32 = Color32::from_rgb(40, 32, 24);
pub const PANEL_BG_DARK: Color32 = Color32::from_rgb(28, 22, 16);
pub const PANEL_BORDER: Color32 = Color32::from_rgb(104, 80, 44);
pub const SLOT_BG: Color32 = Color32::from_rgb(58, 46, 32);
pub const SLOT_BORDER: Color32 = Color32::from_rgb(78, 62, 40);
pub const ACCENT: Color32 = Color32::from_rgb(220, 176, 72);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(140, 110, 44);
pub const TEXT: Color32 = Color32::from_rgb(232, 222, 200);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(168, 156, 132);
pub const HOVER_TEXT: Color32 = Color32::from_rgb(255, 240, 110);
pub const HP_HIGH: Color32 = Color32::from_rgb(70, 170, 70);
pub const HP_LOW: Color32 = Color32::from_rgb(190, 55, 45);
pub const ONLINE: Color32 = Color32::from_rgb(90, 200, 90);
pub const OFFLINE: Color32 = Color32::from_rgb(120, 120, 120);
pub const DANGER: Color32 = Color32::from_rgb(230, 90, 70);
pub const GOOD: Color32 = Color32::from_rgb(120, 210, 120);

// Chat colours.
pub const CHAT_GAME: Color32 = Color32::from_rgb(240, 220, 120);
pub const CHAT_COMBAT: Color32 = Color32::from_rgb(230, 150, 130);
pub const CHAT_LOCAL: Color32 = Color32::from_rgb(235, 235, 235);
pub const CHAT_GLOBAL: Color32 = Color32::from_rgb(140, 200, 240);
pub const CHAT_CLAN: Color32 = Color32::from_rgb(140, 220, 150);
pub const CHAT_PRIVATE: Color32 = Color32::from_rgb(200, 150, 240);
pub const CHAT_ERROR: Color32 = Color32::from_rgb(240, 110, 100);

pub fn setup_theme(ctx: &Context) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = PANEL_BG;
    visuals.window_fill = PANEL_BG;
    visuals.window_stroke = Stroke::new(1.5, PANEL_BORDER);
    visuals.window_rounding = Rounding::same(4.0);
    visuals.extreme_bg_color = PANEL_BG_DARK;
    visuals.faint_bg_color = Color32::from_rgb(50, 40, 30);
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(46, 37, 27);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.inactive.bg_fill = SLOT_BG;
    visuals.widgets.inactive.weak_bg_fill = SLOT_BG;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(76, 60, 40);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(76, 60, 40);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, HOVER_TEXT);
    visuals.widgets.active.bg_fill = Color32::from_rgb(96, 76, 48);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(96, 76, 48);
    visuals.selection.bg_fill = ACCENT_DIM;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = CHAT_GAME;
    visuals.error_fg_color = CHAT_ERROR;
    visuals.window_shadow = egui::epaint::Shadow::NONE;
    visuals.popup_shadow = egui::epaint::Shadow::NONE;
    visuals.override_text_color = Some(TEXT);

    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 3.0);
        style.spacing.window_margin = Margin::same(8.0);
        style.text_styles.insert(
            egui::TextStyle::Heading,
            FontId::new(17.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Body,
            FontId::new(14.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            FontId::new(14.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Small,
            FontId::new(11.5, egui::FontFamily::Proportional),
        );
    });
}

/// Framed panel used for the minimap, side panel, chat and windows.
pub fn panel_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(PANEL_BG)
        .stroke(Stroke::new(1.5, PANEL_BORDER))
        .rounding(Rounding::same(4.0))
        .inner_margin(Margin::same(6.0))
}

pub fn hp_color(ratio: f32) -> Color32 {
    let t = ratio.clamp(0.0, 1.0);
    Color32::from_rgb(
        (HP_LOW.r() as f32 * (1.0 - t) + HP_HIGH.r() as f32 * t) as u8,
        (HP_LOW.g() as f32 * (1.0 - t) + HP_HIGH.g() as f32 * t) as u8,
        (HP_LOW.b() as f32 * (1.0 - t) + HP_HIGH.b() as f32 * t) as u8,
    )
}
