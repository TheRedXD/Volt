use eframe::egui;
use egui::{Color32, FontId, Frame, Image, Label, Sense, Stroke, TextureOptions, Ui, Vec2, Widget, containers::menu::MenuButton, hex_color, include_image};
use tap::Tap;

use crate::visual::{central::Central, dialog::dialog, popups::{about::show_about, settings::show_settings}};

use super::theme::ThemeColors;

use egui::{CursorIcon, RectAlign, Align2, CornerRadius, Popup, PopupAnchor, LayerId, Margin};

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

                macro_rules! menus {
                    [
                        $(
                            $menu_name:expr => [
                                $(
                                    $item_name:expr => $action:block
                                ),*
                                $(,)?
                            ]
                        ),*
                        $(,)?
                    ] => {
                        $(
                            ui.add_space(2.0);
                            let mut menu = MenuButton::new($menu_name);
                            menu.button = menu.button.sense(Sense::CLICK);
                            unhygienic2::unhygienic! {
                                menu.ui(ui, |ui| {
                                    $(
                                        if ui.button($item_name).clicked() $action
                                    )*
                                });
                            }
                        )*
                    };
                }

                menus![
                    "File" => [
                        "New" => {
                            todo!();
                        },
                        "Open" => {
                            todo!();
                        },
                        "Save" => {
                            todo!();
                        },
                        "Exit" => { ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close); },
                    ],
                    "Edit" => [
                        "Undo" => {
                            todo!();
                        },
                        "Redo" => {
                            todo!();
                        },
                        "Cut" => {
                            todo!();
                        },
                        "Copy" => {
                            todo!();
                        },
                        "Paste" => {
                            todo!();
                        },
                        "Settings" => {
                            show_settings(ui);
                        },
                    ],
                    "View" => [
                        "Zoom In" => {
                            todo!();
                        },
                        "Zoom Out" => {
                            todo!();
                        },
                        "Fit to Screen" => {
                            todo!();
                        },
                    ],
                    "Help" => [
                        // TODO: Add documentation eventually (this will probably take a while)
                        // "Documentation" => {
                        //     todo!();
                        // },
                        "About" => {
                            show_about(ui);
                        },
                    ],
                ];
            });
        })
        .response
}

// TODO: Figure out a more sane way to draw the navbar
pub fn navbar<'a>(theme: &'a ThemeColors, central: &'a mut Central) -> impl Widget + use<'a> {
    |ui: &mut Ui| {
        let mut navbar_area = ui.available_rect_before_wrap();
        let navbar_height = 40.0; 
        navbar_area.set_height(navbar_height);

        let navbar_texture_image = super::build_gradient(40, theme.navbar_background_gradient_top, theme.navbar_background_gradient_bottom);
        let navbar_texture = ui.ctx().load_texture("navbar_texture", navbar_texture_image, TextureOptions::default());
        ui.painter().image(
            navbar_texture.id(),
            navbar_area,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );

        ui.allocate_ui_at_rect(navbar_area, |ui| {
            ui.horizontal(|ui| {
                ui.style_mut().spacing.item_spacing = Vec2::ZERO;

                egui::Frame::new()
                    .outer_margin(egui::Margin::same(5))
                    .inner_margin(egui::Margin::same(5))
                    .corner_radius(egui::CornerRadius::same(5))
                    .show(ui, |ui| {
                        egui::Frame::new().inner_margin(egui::Margin::symmetric(5, -6)).show(ui, |ui| {
                            ui.add(Image::new(include_image!("../images/icons/navbar-icon.svg")).fit_to_exact_size(Vec2::splat(30.)));
                        });
                        ui.vertical(|ui| {
                            ui.add_space(2.0);
                            ui.style_mut().visuals.widgets.noninteractive.bg_stroke.color = theme.navbar_element_border;
                            ui.add(egui::Separator::default().vertical().grow(7.).spacing(16.));
                        });
                        navbar_menu_buttons(ui, theme);
                        ui.add_space(8.0);
                    });
            });
        });
        
        let transport_icons_container_width = 130.0;
        let transport_icons_height = 32.0;

        let center_x = (navbar_area.width() / 2.0) - (transport_icons_container_width / 2.0);
        let transport_rect_min = egui::pos2(
            navbar_area.min.x + center_x,
            navbar_area.min.y
        );
        let transport_rect_max = egui::pos2(
            transport_rect_min.x + transport_icons_container_width,
            navbar_area.min.y + transport_icons_height
        );
        let transport_rect = egui::Rect::from_min_max(transport_rect_min, transport_rect_max);

        ui.allocate_ui_at_rect(transport_rect, |ui| {
            ui.centered_and_justified(|ui| {
                ui.horizontal(|ui| {
                    ui.style_mut().spacing.item_spacing = Vec2::new(8.0, 0.0);
                    ui.add(Image::new(include_image!("../images/icons/loop-icon.svg")).tint(hex_color!("#888888")).fit_to_exact_size(Vec2::splat(16.)));
                    ui.add_space(4.);
                    ui.add(Image::new(include_image!("../images/icons/play-icon.svg")).tint(hex_color!("#8cdd8c")).fit_to_exact_size(Vec2::splat(16.)));
                    ui.add(Image::new(include_image!("../images/icons/stop-icon.svg")).tint(egui::Color32::WHITE).fit_to_exact_size(Vec2::splat(16.)));
                    ui.add(Image::new(include_image!("../images/icons/record-icon.svg")).tint(egui::Color32::WHITE).fit_to_exact_size(Vec2::splat(16.)));
                    ui.add_space(4.);
                    ui.add(Image::new(include_image!("../images/icons/playback-metronome-empty.svg")).tint(egui::Color32::WHITE).fit_to_exact_size(Vec2::new(20., 12.)));
                });
            });
        });

        ui.allocate_rect(navbar_area, Sense::hover())
    }
}