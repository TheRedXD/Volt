use egui::{Align2, Color32, Context, CornerRadius, CursorIcon, FontId, Frame, Image, LayerId, Margin, Popup, PopupAnchor, RectAlign, RichText, Ui, Vec2, hex_color};

use crate::visual::{icons::macros::get_icon_image, theme::ThemeColors};

pub fn show_settings(ui: &mut Ui) {
    Popup::open_id(ui.ctx(), "settings".into());
}

pub fn render_settings(ctx: &Context, theme: &ThemeColors) {
    Popup::new(
        "settings".into(),
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
        ui.vertical(|ui| {
            ui.label(RichText::new("Settings").font(FontId::proportional(24.)));
            ui.label(RichText::new("TODO, settings will be shown here!").font(FontId::proportional(12.)));
        });
        ui.add_space(5.);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.style_mut().spacing.button_padding = Vec2 { x: 12., y: 4. };
            ui.style_mut().visuals.widgets.hovered.weak_bg_fill = hex_color!("#ffffff10");
            ui.style_mut().visuals.widgets.active.weak_bg_fill = hex_color!("#ffffff20");
            if ui.add(egui::Button::new("Ok").corner_radius(10.)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                ui.close();
            }
            if ui.add(egui::Button::new("Apply").corner_radius(10.)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
            }
            if ui.add(egui::Button::new("Discard").corner_radius(10.)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
            }
        });
    });
}