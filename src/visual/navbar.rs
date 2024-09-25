use crate::images::ICON_IMAGE;
use crate::visual::ThemeColors;
use eframe::egui;
use egui::{
    pos2, Align2, Color32, FontFamily, FontId, Image, InnerResponse, Margin, Rect, Stroke, Ui,
};

fn default_nav_label(ui: &mut Ui) -> InnerResponse<Rect> {
    egui::Frame::default().show(ui, |ui| {
        let (_, rect) = ui.allocate_space(ui.available_size());
        ui.painter().text(
            egui::Pos2 {
                x: rect.left(),
                y: rect.top(),
            },
            Align2::LEFT_BOTTOM,
            "Version INDEV",
            FontId::new(12.0, FontFamily::Proportional),
            Color32::from_hex("#ffffff80").unwrap_or_default(),
        )
    })
}

fn default_nav_title(ui: &mut Ui) -> InnerResponse<Rect> {
    egui::Frame::default().show(ui, |ui| {
        let (_, rect) = ui.allocate_space(ui.available_size());
        ui.painter().text(
            egui::Pos2 {
                x: rect.left(),
                y: rect.top(),
            },
            Align2::LEFT_TOP,
            "Volt",
            FontId::new(20.0, FontFamily::Proportional),
            Color32::from_hex("#ffffff").unwrap_or_default(),
        )
    })
}

const NO_MARGIN: Margin = Margin {
    left: 0.,
    right: 0.,
    top: 0.,
    bottom: 0.,
};

const M5: Margin = Margin {
    left: 5.,
    right: 5.,
    top: 5.,
    bottom: 5.,
};

const M_5535: Margin = Margin {
    left: 5.,
    right: 5.,
    top: 3.,
    bottom: 5.,
};

pub fn paint_navbar(ui: &mut Ui, viewport: &Rect, theme: &ThemeColors) {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
        ui.set_height(50.);
        ui.set_width(viewport.width());
        egui::Frame::default()
            .fill(theme.navbar)
            .inner_margin(NO_MARGIN)
            .outer_margin(NO_MARGIN)
            .show(ui, |ui| {
                egui::Frame::default().show(ui, |ui| {
                    ui.painter().line_segment(
                        [pos2(300., 50.), pos2(viewport.width(), 50.)],
                        Stroke::new(0.5, theme.navbar_outline),
                    );
                    egui::Frame::default()
                        .fill(theme.navbar)
                        .inner_margin(M5)
                        .outer_margin(NO_MARGIN)
                        .show(ui, |ui| {
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    egui::Frame::default().fill(theme.navbar).show(ui, |ui| {
                                        ui.add(
                                            Image::new(ICON_IMAGE).max_width(40.).max_height(40.),
                                        );
                                        egui::Frame::default()
                                            .fill(theme.navbar)
                                            .inner_margin(M_5535)
                                            .outer_margin(NO_MARGIN)
                                            .show(ui, |ui| {
                                                ui.with_layout(
                                                    egui::Layout::top_down(egui::Align::TOP),
                                                    |ui| {
                                                        default_nav_title(ui);
                                                        ui.with_layout(
                                                            egui::Layout::left_to_right(
                                                                egui::Align::BOTTOM,
                                                            ),
                                                            default_nav_label,
                                                        );
                                                    },
                                                );
                                            });
                                    });
                                },
                            );
                        });
                    ui.allocate_space(ui.available_size());
                });
            });
    });
}
