use egui::{
    include_image, pos2, vec2, Align2, Color32,
    FontFamily, FontId, Image, Rect, Stroke, Ui,
    RichText, Margin
};
use eframe::egui;

use crate::visual::ThemeColors;

pub fn paint_navbar(ui: &mut Ui, viewport: &Rect, theme: &ThemeColors) {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
        ui.set_height(50.);
        ui.set_width(viewport.width());
        egui::Frame::default()
            .fill(theme.navbar)
            .inner_margin(Margin { left: 0., right: 0., top: 0., bottom: 0. })
            .outer_margin(Margin { left: 0., right: 0., top: 0., bottom: 0. })
            .show(ui, |ui| {
                egui::Frame::default()
                    .show(ui, |ui| {
                        ui.painter().line_segment(
                            [pos2(300., 50.), pos2(viewport.width(), 50.)],
                            Stroke::new(0.5, theme.navbar_outline),
                        );
                        egui::Frame::default()
                            .fill(theme.navbar)
                            .inner_margin(Margin { left: 5., right: 5., top: 5., bottom: 5. })
                            .outer_margin(Margin { left: 0., right: 0., top: 0., bottom: 0. })
                            .show(ui, |ui| {
                                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                    egui::Frame::default()
                                        .fill(theme.navbar)
                                        .show(ui, |ui| {
                                            ui.add(
                                                Image::new(include_image!("../images/icons/icon.png"))
                                                    .max_width(40.)
                                                    .max_height(40.)
                                            );
                                            egui::Frame::default()
                                                .fill(theme.navbar)
                                                .inner_margin(Margin { left: 5., right: 5., top: 3., bottom: 5. })
                                                .outer_margin(Margin { left: 0., right: 0., top: 0., bottom: 0. })
                                                .show(ui, |ui| {
                                                    ui.with_layout(egui::Layout::top_down(egui::Align::TOP), |ui| {
                                                        egui::Frame::default()
                                                            .show(ui, |ui| {
                                                                let (_, rect) = ui.allocate_space(ui.available_size());
                                                                ui.painter().text(
                                                                    egui::Pos2 { x: rect.left(), y: rect.top() },
                                                                    Align2::LEFT_TOP,
                                                                    "Volt",
                                                                    FontId::new(20.0, FontFamily::Proportional),
                                                                    Color32::from_hex("#ffffff").unwrap_or_default()
                                                                );
                                                            });
                                                        ui.with_layout(egui::Layout::left_to_right(egui::Align::BOTTOM), |ui| {
                                                            egui::Frame::default()
                                                                .show(ui, |ui| {
                                                                    let (_, rect) = ui.allocate_space(ui.available_size());
                                                                    ui.painter().text(
                                                                        egui::Pos2 { x: rect.left(), y: rect.top() },
                                                                        Align2::LEFT_BOTTOM,
                                                                        "Version INDEV",
                                                                        FontId::new(12.0, FontFamily::Proportional),
                                                                        Color32::from_hex("#ffffff80").unwrap_or_default()
                                                                    );
                                                                });
                                                        });
                                                    });
                                                });
                                        });
                                });
                            });
                        ui.allocate_space(ui.available_size());
                    });
            });
    });
}