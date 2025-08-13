use eframe::egui;
use egui::{FontFamily, Label, Margin, RichText, TextureOptions, Ui, Vec2, Widget, hex_color};

use super::ThemeColors;

pub fn status(themes: &ThemeColors) -> impl Widget + use<'_> {
    |ui: &mut Ui| {
        let navbar_texture_image = super::build_gradient(20, themes.navbar_background_gradient_bottom, themes.navbar_background_gradient_top);
        let navbar_texture = ui.ctx().load_texture("navbar_texture", navbar_texture_image, TextureOptions::default());

        ui.painter().image(
            navbar_texture.id(),
            ui.available_rect_before_wrap(),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        // ui.painter().line(
        //     vec![ui.available_rect_before_wrap().left_top(), ui.available_rect_before_wrap().right_top()],
        //     egui::Stroke::new(1.0, Color32::from_hex("#353248").unwrap()),
        // );
        ui.horizontal(|ui| {
            egui::Frame::default().show(ui, |ui| {
                ui.horizontal(|ui| {
                    egui::Frame::new().show(ui, |ui| {
                        ui.style_mut().spacing.item_spacing = Vec2::ZERO;
                        egui::Frame::new().inner_margin(Margin::same(5)).show(ui, |ui| {
                            ui.add(Label::new(RichText::new("Volt v1.0.0").family(FontFamily::Proportional).color(hex_color!("#777490"))).selectable(false));
                        });
                    });
                })
            })
        })
        .response
    }
}
