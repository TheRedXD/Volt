use egui::{Align2, Context, CornerRadius, LayerId, Margin, Popup, PopupAnchor, RectAlign, Ui};

use crate::visual::theme::ThemeColors;

pub fn dialog(ctx: &Context, theme: &ThemeColors, inner: impl FnOnce(&mut Ui)) {
    Popup::new(
        "welcome".into(),
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
    .show(inner);
}
