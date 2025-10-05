use eframe::egui;
use egui::{Color32, FontId, Image, Sense, Shadow, Stroke, TextureOptions, Ui, Vec2, Widget, containers::menu::MenuButton, hex_color, include_image};
use tap::Tap;

use super::theme::ThemeColors;

pub fn navbar_menu_buttons(ui: &mut Ui, theme: &ThemeColors) -> egui::Response {
    egui::Frame::new()
        .show(ui, |ui| {
            ui.scope(|ui| {
                ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                ui.visuals_mut().widgets.hovered.weak_bg_fill = hex_color!("#ffffff10");
                ui.visuals_mut().widgets.active.weak_bg_fill = hex_color!("#ffffff20");
                ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::NONE;
                ui.visuals_mut().widgets.hovered.bg_stroke = Stroke::new(1., hex_color!("#ffffff20"));
                ui.visuals_mut().widgets.active.bg_stroke = Stroke::new(1., hex_color!("#ffffff30"));
                ui.style_mut().spacing.button_padding = Vec2 { x: 6., y: 2. };
                ui.style_mut().override_font_id = Some(FontId::proportional(12.));
                ui.style_mut().visuals.popup_shadow = theme.shadow.tap_mut(|shadow| shadow.color = hex_color!("#00000020"));

                ui.add_space(2.0);
                let mut click_no_focus = Sense::click();
                click_no_focus.remove(Sense::focusable_noninteractive());
                let mut file_menu = MenuButton::new("File");
                file_menu.button = file_menu.button.sense(click_no_focus);
                file_menu.ui(ui, |ui| {
                    if ui.button("New").clicked() {
                        todo!();
                    }
                    if ui.button("Open").clicked() {
                        todo!();
                    }
                    if ui.button("Save").clicked() {
                        todo!();
                    }
                    if ui.button("Exit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(2.0);
                let mut edit_menu = MenuButton::new("Edit");
                edit_menu.button = edit_menu.button.sense(click_no_focus);
                edit_menu.ui(ui, |ui| {
                    if ui.button("Undo").clicked() {
                        todo!();
                    }
                    if ui.button("Redo").clicked() {
                        todo!();
                    }
                    if ui.button("Cut").clicked() {
                        todo!();
                    }
                    if ui.button("Copy").clicked() {
                        todo!();
                    }
                    if ui.button("Paste").clicked() {
                        todo!();
                    }
                });
                ui.add_space(2.0);
                let mut view_menu = MenuButton::new("View");
                view_menu.button = view_menu.button.sense(click_no_focus);
                view_menu.ui(ui, |ui| {
                    if ui.button("Zoom In").clicked() {
                        todo!();
                    }
                    if ui.button("Zoom Out").clicked() {
                        todo!();
                    }
                    if ui.button("Fit to Screen").clicked() {
                        todo!();
                    }
                });
                ui.add_space(2.0);
                let mut help_menu = MenuButton::new("Help");
                help_menu.button = help_menu.button.sense(click_no_focus);
                help_menu.ui(ui, |ui| {
                    if ui.button("Documentation").clicked() {
                        todo!();
                    }
                    if ui.button("About").clicked() {
                        todo!();
                    }
                });
            });
        })
        .response
}

pub fn navbar(theme: &ThemeColors) -> impl Widget + use<'_> {
    |ui: &mut Ui| {
        let navbar_texture_image = super::build_gradient(40, theme.navbar_background_gradient_top, theme.navbar_background_gradient_bottom);
        let navbar_texture = ui.ctx().load_texture("navbar_texture", navbar_texture_image, TextureOptions::default());

        ui.painter().image(
            navbar_texture.id(),
            ui.available_rect_before_wrap(),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        ui.horizontal(|ui| {
            egui::Frame::default().show(ui, |ui| {
                ui.horizontal(|ui| {
                    egui::Frame::new().show(ui, |ui| {
                        ui.style_mut().spacing.item_spacing = Vec2::ZERO;
                        egui::Frame::new()
                            .outer_margin(egui::Margin::same(5))
                            .inner_margin(egui::Margin::same(5))
                            .stroke(Stroke::new(1., hex_color!("#35324840")))
                            .corner_radius(egui::CornerRadius::same(5))
                            .fill(theme.navbar_widget)
                            .show(ui, |ui| {
                                egui::Frame::new().inner_margin(egui::Margin::symmetric(5, -6)).show(ui, |ui| {
                                    ui.add(Image::new(include_image!("../images/icons/navbar-icon.svg")).fit_to_exact_size(Vec2::splat(30.)));
                                });
                                ui.vertical(|ui| {
                                    ui.add_space(2.0);
                                    ui.add(egui::Separator::default().vertical().grow(7.).spacing(16.));
                                });
                                navbar_menu_buttons(ui, theme);
                                ui.add_space(8.0);
                            });
                        ui.centered_and_justified(|ui| {
                            egui::Frame::new().show(ui, |ui| {
                                egui::Frame::new()
                                    .outer_margin(egui::Margin::symmetric(2, 5))
                                    .inner_margin(egui::Margin::same(5))
                                    .stroke(Stroke::new(1., hex_color!("#35324840")))
                                    .corner_radius(egui::CornerRadius::same(5))
                                    .fill(theme.navbar_widget)
                                    .show(ui, |ui| {
                                        ui.add(
                                            Image::new(include_image!("../images/icons/play-icon.svg"))
                                                .tint(egui::Color32::GREEN)
                                                .fit_to_exact_size(Vec2::splat(16.)),
                                        );
                                    });
                            });
                        });
                    });
                })
            })
        })
        .response
    }
}
