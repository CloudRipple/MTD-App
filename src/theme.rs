use eframe::egui;

pub(crate) const SURFACE: egui::Color32 = egui::Color32::from_rgb(252, 253, 253);
pub(crate) const CANVAS: egui::Color32 = egui::Color32::from_rgb(242, 245, 246);
pub(crate) const BORDER: egui::Color32 = egui::Color32::from_rgb(218, 226, 231);
pub(crate) const INK: egui::Color32 = egui::Color32::from_rgb(24, 34, 43);
pub(crate) const MUTED: egui::Color32 = egui::Color32::from_rgb(91, 104, 115);
pub(crate) const FAINT: egui::Color32 = egui::Color32::from_rgb(136, 149, 160);
pub(crate) const ACCENT: egui::Color32 = egui::Color32::from_rgb(30, 132, 118);
pub(crate) const ACCENT_DARK: egui::Color32 = egui::Color32::from_rgb(17, 99, 90);
pub(crate) const ACCENT_SOFT: egui::Color32 = egui::Color32::from_rgb(224, 243, 240);
pub(crate) const DANGER: egui::Color32 = egui::Color32::from_rgb(176, 57, 54);
pub(crate) const WINDOW_CORNER_RADIUS: u8 = 14;

pub(crate) fn panel_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(SURFACE)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::same(16))
}

pub(crate) fn preview_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(SURFACE)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::same(14))
}

pub(crate) fn install_app_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 9.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.interact_size = egui::vec2(44.0, 34.0);
    style.visuals.override_text_color = Some(INK);
    style.visuals.panel_fill = CANVAS;
    style.visuals.window_fill = SURFACE;
    style.visuals.faint_bg_color = egui::Color32::from_rgb(238, 243, 245);
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(247, 250, 251);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(246, 249, 250);
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    style.visuals.widgets.hovered.bg_fill = ACCENT_SOFT;
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(204, 232, 228);
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT_DARK);
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, BORDER);
    style.visuals.selection.bg_fill = ACCENT;
    style.visuals.hyperlink_color = ACCENT_DARK;
    ctx.set_style(style);
}
