use egui::{Color32, Context, FontId, Stroke, Visuals};

pub const PANEL_BG: Color32 = Color32::from_rgb(24, 28, 22);
pub const ACCENT: Color32 = Color32::from_rgb(210, 165, 60);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(140, 110, 40);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(160, 170, 150);
pub const HP_HIGH: Color32 = Color32::from_rgb(80, 160, 70);
pub const HP_LOW: Color32 = Color32::from_rgb(180, 60, 50);
pub const ONLINE: Color32 = Color32::from_rgb(90, 200, 90);
pub const OFFLINE: Color32 = Color32::from_rgb(120, 120, 120);

pub fn setup_theme(ctx: &Context) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = PANEL_BG;
    visuals.window_fill = Color32::from_rgb(30, 34, 28);
    visuals.extreme_bg_color = Color32::from_rgb(18, 20, 16);
    visuals.faint_bg_color = Color32::from_rgb(36, 42, 32);
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(32, 38, 28);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 48, 34);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(52, 60, 42);
    visuals.widgets.active.bg_fill = Color32::from_rgb(68, 78, 52);
    visuals.selection.bg_fill = ACCENT_DIM;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = Color32::from_rgb(220, 170, 80);
    visuals.error_fg_color = Color32::from_rgb(220, 90, 70);
    visuals.window_shadow = egui::epaint::Shadow::NONE;

    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.text_styles.insert(
            egui::TextStyle::Heading,
            FontId::new(18.0, egui::FontFamily::Proportional),
        );
        style.text_styles.insert(
            egui::TextStyle::Body,
            FontId::new(14.0, egui::FontFamily::Proportional),
        );
    });
}

pub fn hp_color(ratio: f32) -> Color32 {
    let t = ratio.clamp(0.0, 1.0);
    Color32::from_rgb(
        (HP_LOW.r() as f32 * (1.0 - t) + HP_HIGH.r() as f32 * t) as u8,
        (HP_LOW.g() as f32 * (1.0 - t) + HP_HIGH.g() as f32 * t) as u8,
        (HP_LOW.b() as f32 * (1.0 - t) + HP_HIGH.b() as f32 * t) as u8,
    )
}
