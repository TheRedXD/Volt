use egui::{
    include_image, pos2, vec2, Align2, Color32,
    FontFamily, FontId, Image, Rect, Stroke, Ui,
};

use crate::visual::ThemeColors;

pub fn paint_navbar(ui: &Ui, viewport: &Rect, theme: &ThemeColors) {
    ui.painter()
        .rect_filled(viewport.with_max_y(50.), 0.0, theme.navbar);
    Image::new(include_image!("images/icons/icon.png"))
        .paint_at(ui, Rect::from_min_size(pos2(5., 5.), vec2(40., 40.)));
    ui.painter().text(
        pos2(55., 8.),
        Align2::LEFT_TOP,
        "Volt",
        FontId::new(20.0, FontFamily::Proportional),
        Color32::from_hex("#ffffff").unwrap_or_default(),
    );
    ui.painter().text(
        pos2(55., 28.),
        Align2::LEFT_TOP,
        "Version INDEV",
        FontId::new(12.0, FontFamily::Proportional),
        Color32::from_hex("#ffffff80").unwrap_or_default(),
    );

    ui.painter().line_segment(
        [pos2(300., 50.), pos2(viewport.width(), 50.)],
        Stroke::new(0.5, theme.navbar_outline),
    );
}