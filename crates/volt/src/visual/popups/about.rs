use egui::{Align2, Color32, Context, CornerRadius, CursorIcon, FontId, Frame, Image, LayerId, Margin, Popup, PopupAnchor, RectAlign, RichText, Ui, Vec2, hex_color};

use crate::visual::{icons::macros::get_icon_image, theme::ThemeColors};

pub fn show_about(ui: &mut Ui) {
    Popup::open_id(ui.ctx(), "about".into());
}

pub fn render_about(ctx: &Context, theme: &ThemeColors) {
    Popup::new(
        "about".into(),
        ctx.clone(),
        PopupAnchor::Position(ctx.screen_rect().center()),
        LayerId::new(egui::Order::Foreground, "popups".into()),
    )
    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
    .open_memory(None)
    .align(RectAlign {
        child: Align2::CENTER_CENTER,
        parent: Align2::CENTER_CENTER,
    })
    .frame(
        egui::Frame::new()
            .fill(theme.central_background)
            .stroke(egui::Stroke::new(1., theme.dialog_border))
            .shadow(theme.shadow)
            .corner_radius(CornerRadius::ZERO.at_least(5))
            .inner_margin(Margin::same(10)),
    )
    .show(|ui| {
        ui.horizontal(|ui| {
            Frame::new()
                .stroke(egui::Stroke::new(1., theme.dialog_border))
                .corner_radius(CornerRadius::same(10))
                .show(ui, |ui| {
                    ui.add(
                        get_icon_image!("app-icon.png")
                            .fit_to_exact_size(Vec2::new(64., 64.))
                    );
                });
            ui.add_space(8.);
            ui.vertical(|ui| {
                ui.label(RichText::new("Volt").font(FontId::proportional(32.)).color(Color32::WHITE));
                ui.label(RichText::new("The Digital Audio Workstation for everyone.\nVersion INDEV").font(FontId::proportional(12.)));
                ui.hyperlink_to("github.com/TheRedXD/Volt", "https://github.com/TheRedXD/Volt");
            });
        });
        ui.add_space(5.);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.style_mut().spacing.button_padding = Vec2 { x: 12., y: 4. };
            ui.style_mut().visuals.widgets.hovered.weak_bg_fill = hex_color!("#ffffff10");
            ui.style_mut().visuals.widgets.active.weak_bg_fill = hex_color!("#ffffff20");
            if ui.add(egui::Button::new("Ok").corner_radius(10.)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                ui.close();
            }
        });
    });
}