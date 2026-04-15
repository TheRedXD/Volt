use crate::theme::ThemeColors;
use gpui::{App, AppContext, Context, Empty, InteractiveElement, IntoElement, ParentElement, Pixels, Point, Render, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div};
use std::fmt::Display;
use std::sync::Arc;

pub(crate) type AdjustableInputSet<V> = dyn Fn(V, &mut App) + 'static;

pub(crate) trait AdjustableInputValue: Display + 'static + Sized + Clone {
    fn apply_delta(self, delta: f32) -> Self;
}

impl AdjustableInputValue for u32 {
    fn apply_delta(self, delta: f32) -> Self {
        (self as f32 + delta).round().max(0.) as u32
    }
}

impl AdjustableInputValue for f32 {
    fn apply_delta(self, delta: f32) -> Self {
        self + delta
    }
}

impl AdjustableInputValue for f64 {
    fn apply_delta(self, delta: f32) -> Self {
        self + Self::from(delta)
    }
}

#[derive(IntoElement)]
pub(crate) struct AdjustableInput<V: AdjustableInputValue> {
    pub(crate) value: V,
    pub(crate) theme: Arc<ThemeColors>,
    pub(crate) set: Box<AdjustableInputSet<V>>,
    pub(crate) name: SharedString,
    pub(crate) scale: f32,
}

impl<V: AdjustableInputValue> RenderOnce for AdjustableInput<V> {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        struct Payload<V: AdjustableInputValue>(Point<Pixels>, V, SharedString);
        div()
            .child(div().child(format!("{:.02}", self.value)))
            .rounded_md()
            .border_1()
            .border_color(self.theme.navbar_outline)
            .cursor_ns_resize()
            .py_1()
            .px_2()
            .id(self.name.clone())
            .on_drag(Payload(window.mouse_position(), self.value, self.name.clone().into()), move |_, _, _, cx| cx.new(|_| Empty))
            .on_drag_move({
                let name = self.name.clone();
                move |event: &gpui::DragMoveEvent<Payload<V>>, _, cx| {
                    if event.drag(cx).2 != name {
                        return;
                    }
                    cx.stop_propagation();
                    let delta = (event.drag(cx).0.y - event.event.position.y).as_f32() * self.scale * if event.event.modifiers.shift { 0.2 } else { 1. };
                    let value = event.drag(cx).1.clone().apply_delta(delta);
                    (self.set)(value, cx);
                }
            })
            .tooltip({
                let name = self.name.clone();
                move |_, cx| {
                    struct Tooltip {
                        name: SharedString,
                        theme: Arc<ThemeColors>,
                    }

                    impl Render for Tooltip {
                        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                            div()
                                .bg(self.theme.central_background)
                                .text_color(self.theme.bg_text)
                                .rounded_md()
                                .p_2()
                                .border_1()
                                .border_color(self.theme.navbar_outline)
                                .shadow_sm()
                                .child(self.name.clone())
                        }
                    }
                    cx.new(|_| Tooltip {
                        name: name.clone(),
                        theme: Arc::clone(&self.theme),
                    })
                    .into()
                }
            })
    }
}
