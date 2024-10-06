use eframe::egui;
use egui::{
    Pos2, Align2, Color32, FontFamily, FontId, Rect, Ui,
};

use crate::visual::ThemeColors;

pub fn paint_background(ui: &mut Ui, viewport: &Rect, theme: &ThemeColors) {
    ui.painter().rect_filled(
        Rect::from_min_size(Pos2::ZERO, viewport.size()),
        0.0,
        Color32::from_hex("#1e222f").unwrap_or_default(),
    );
    ui.painter().text(
        Rect::from_min_size(Pos2::ZERO, viewport.size()).center(),
        Align2::CENTER_CENTER,
        "In development",
        FontId::new(32.0, FontFamily::Name("IBMPlexMono".into())),
        theme.bg_text,
    );
}