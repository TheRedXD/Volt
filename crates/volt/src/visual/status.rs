use eframe::egui;
use egui::{Button, FontFamily, Image, Label, Margin, RichText, Sense, TextureOptions, Ui, Vec2, Widget, hex_color, include_image};
use itertools::Itertools;
use tap::Pipe;

use crate::visual::{central::Mode, theme::ThemeColors};

pub fn status(themes: &ThemeColors, show_browser: &mut bool, central_mode: &mut Mode) -> impl Widget {
    |ui: &mut Ui| {
        Image::from_texture(&ui.ctx().load_texture(
            "navbar_texture",
            super::build_gradient(20, themes.navbar_background_gradient_bottom, themes.navbar_background_gradient_top),
            TextureOptions::default(),
        ))
        .paint_at(ui, ui.clip_rect());
        ui.horizontal(|ui| {
            ui.scope(|ui| {
                ui.style_mut().spacing.item_spacing = Vec2::ZERO;
                egui::Frame::new().inner_margin(Margin::same(5)).show(ui, |ui| {
                    ui.add_space(2.);
                    egui::Frame::new().outer_margin(Margin::same(2)).show(ui, |ui| {
                        if ui
                            .add(
                                Image::new(include_image!("../images/icons/browser-collapse.svg"))
                                    .fit_to_exact_size(Vec2::splat(16.))
                                    .tint(if *show_browser { themes.accent } else { hex_color!("#ffffff40") }),
                            )
                            .interact(Sense::click())
                            .clicked_by(egui::PointerButton::Primary)
                        {
                            *show_browser = !*show_browser;
                            ui.ctx().request_repaint();
                        }
                    });
                    ui.add_space(6.);
                    ui.separator();
                    ui.add_space(10.);
                    ui.add(Label::new(RichText::new(concat!("Volt ", env!("CARGO_PKG_VERSION"))).color(hex_color!("#777490"))).selectable(false));
                });
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.style_mut().spacing.item_spacing = Vec2::X * 10.;
                egui::Frame::new().inner_margin(Margin::same(5)).show(ui, |ui| {
                    enum Widget {
                        Button(&'static str, Mode),
                        Separator,
                    }
                    for widget in [("GRAPH", Mode::Graph), ("ARRANGE", Mode::Playlist)]
                        .into_iter()
                        .map(|(label, mode)| Widget::Button(label, mode))
                        .pipe(|iterator| Itertools::intersperse_with(iterator, || Widget::Separator))
                    {
                        match widget {
                            Widget::Button(label, mode) => {
                                if RichText::new(label)
                                    .family(FontFamily::Monospace)
                                    .color(if *central_mode == mode { themes.accent } else { hex_color!("#77749040") })
                                    .pipe(Button::new)
                                    .frame(false)
                                    .pipe(|button| ui.add(button))
                                    .clicked()
                                {
                                    *central_mode = mode;
                                    ui.ctx().request_repaint();
                                }
                            }
                            Widget::Separator => {
                                ui.separator();
                            }
                        }
                    }
                });
            });
        })
        .response
    }
}
