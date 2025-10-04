use eframe::egui;
use egui::{hex_color, include_image, FontFamily, Image, Label, Margin, RichText, Sense, Stroke, TextureOptions, Ui, Vec2, Widget};

use super::ThemeColors;

pub fn status<'a, 'b>(themes: &'a ThemeColors, show_browser: &'b mut bool) -> impl Widget + use<'a, 'b> {
    |ui: &mut Ui| {
        let navbar_texture_image = super::build_gradient(20, themes.navbar_background_gradient_bottom, themes.navbar_background_gradient_top);
        let navbar_texture = ui.ctx().load_texture("navbar_texture", navbar_texture_image, TextureOptions::default());
        
        ui.painter().image(
            navbar_texture.id(),
            ui.available_rect_before_wrap(),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        ui.painter().line(
            vec![ui.available_rect_before_wrap().left_top(), ui.available_rect_before_wrap().right_top()],
            Stroke::new(1.0, hex_color!("#353248")),
        );
        ui.horizontal(|ui| {
            egui::Frame::default().show(ui, |ui| {
                ui.horizontal(|ui| {
                    egui::Frame::new().show(ui, |ui| {
                        ui.style_mut().spacing.item_spacing = Vec2::ZERO;
                        egui::Frame::new().inner_margin(Margin::same(5)).show(ui, |ui| {
                            ui.add_space(2.);
                            egui::Frame::new().outer_margin(Margin::same(2)).show(ui, |ui| {
                                let mut tint = themes.accent;
                                if !*show_browser {
                                    tint = hex_color!("#ffffff40");
                                }
                                let resp = ui.add(Image::new(include_image!("../images/icons/browser-collapse.svg")).fit_to_exact_size(Vec2 {x: 16., y: 16.}).tint(tint)).interact(Sense::click());
                                if resp.clicked_by(egui::PointerButton::Primary) {
                                    *show_browser = !*show_browser;
                                }
                            });
                            ui.add_space(6.);
                            ui.painter().rect_stroke(
                                egui::Rect::from_min_max(
                                    egui::pos2(ui.cursor().left(), ui.cursor().top() - 20. + 6.),
                                    egui::pos2(ui.cursor().left() + 1., ui.cursor().top() + 20. + 6.),
                                ),
                                0.0,
                                egui::Stroke::new(1.0, hex_color!("#353248")),
                                egui::StrokeKind::Inside
                            );
                            ui.add_space(10.);
                            ui.add(Label::new(RichText::new("Volt v0.1.0").family(FontFamily::Proportional).color(hex_color!("#777490"))).selectable(false));
                        });
                    });
                })
            })
        })
        .response
    }
}
