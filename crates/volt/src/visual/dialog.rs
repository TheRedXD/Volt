use egui::{Align2, Context, CornerRadius, LayerId, Margin, Popup, PopupAnchor, RectAlign, Shadow, Ui, hex_color};

use crate::visual::ThemeColors;

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
            .stroke(egui::Stroke::new(1., hex_color!("#353248")))
            .shadow(Shadow {
                // TODO move all common shadows to a theme struct
                offset: [0, 0],
                blur: 10,
                spread: 5,
                color: hex_color!("#00000020"),
            })
            .corner_radius(CornerRadius::ZERO.at_least(5))
            .inner_margin(Margin::same(10)),
    )
    .show(inner);
}
