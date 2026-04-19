#![warn(clippy::pedantic, clippy::nursery, clippy::allow_attributes_without_reason, clippy::undocumented_unsafe_blocks, clippy::clone_on_ref_ptr)]
use std::{
    array::from_fn,
    borrow::Cow,
    fmt::Display,
    ops::{DerefMut, Sub, SubAssign},
    sync::{Arc, Mutex},
    time::Instant,
};

use blerp::{Beats, Playlist, PlaylistAudio, Samples, Tempo, Time};
use cpal::{
    default_host,
    traits::{DeviceTrait, HostTrait},
};
use gpui::{
    AnyDrag, AnyView, App, AssetSource, Bounds, Context, DefiniteLength, Div, DivFrameState, ElementId, Empty, Entity, FocusHandle, Global, Hitbox, KeyBinding, LayoutId, List, MouseButton, PathBuilder, Pixels, Point, Rems, Rgba, SharedString, Size, Stateful, Style, StyleRefinement, Styled, TitlebarOptions, WeakEntity, Window, WindowBounds, WindowOptions, actions, canvas, deferred, div, hsla, img, linear_color_stop, linear_gradient, pattern_slash, point, prelude::*, px, rems, rgb, rgba, size
};
use gpui_component::{StyledExt, button};
use gpui_platform::application;
use itertools::Itertools;
use tap::{Conv, Pipe, Tap};

use crate::theme::{default, gray};

use crate::components::adjustable_input::AdjustableInput;
use crate::theme::ThemeColors;
use crate::views::{browser::BrowserView, playlist::PlaylistView};

mod components;
mod theme;
mod views;

actions!([TogglePlay]);

#[derive(IntoElement)]
struct Navbar {
    theme: Arc<ThemeColors>,
    playlist: Entity<PlaylistView>,
    app: Entity<Volt>,
}

// Navbar widget
impl RenderOnce for Navbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let playlist_view = self.playlist.read(cx);
        div()
            .flex()
            .h_10()
            .p_2()
            .gap_0p5()
            .flex_shrink_0()
            .rounded_md()
            .bg(linear_gradient(
                0.,
                linear_color_stop(self.theme.navbar_background_gradient_bottom, 0.),
                linear_color_stop(self.theme.navbar_background_gradient_top, 1.),
            ))
            .child(
                div()
                    .flex()
                    .p_1()
                    .gap_1()
                    .items_center()
                    .rounded_md()
                    .child(img(NAVBAR_ICON).size_6().mr_1())
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .items_center()
                            .children(["File", "Edit", "View", "Help"].map(|name| div().child(name).text_sm().py_px().px_1().rounded_sm().id(name).hover(|style| style.bg(self.theme.hover)))),
                    )
                    .mr_1p5(),
            )
            .child(div().w_px().bg(self.theme.navbar_outline).h_full())
            .child(
                div()
                    .flex_grow()
                    .flex()
                    .gap_2()
                    .p_2()
                    .rounded_md()
                    .items_center()
                    .child(
                        div()
                            .flex_shrink()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_shrink()
                                    .text_sm()
                                    .items_center()
                                    .line_height(DefiniteLength::Fraction(0.8))
                                    .child(div().child("BPM").text_xs())
                                    .child(AdjustableInput {
                                        value: playlist_view.audio.playlist().tempo.bpm(),
                                        theme: Arc::clone(&self.theme),
                                        set: {
                                            let playlist = self.playlist.downgrade();
                                            Arc::new(move |bpm, cx| {
                                                playlist
                                                    .update(cx, |playlist, cx| {
                                                        playlist.audio.update_tempo(|_| Tempo::from_bpm(bpm));
                                                        cx.notify();
                                                    })
                                                    .unwrap();
                                            })
                                        },
                                        scale: 0.1,
                                        name: "Tempo BPM".into(),
                                        default: 120.,
                                    })
                                    .id("bpm")
                                    .hoverable_tooltip({
                                        let playlist_view = self.playlist.clone();
                                        move |_, cx| {
                                            let playlist_view = playlist_view.clone();
                                            cx.new(move |_| Bpm {
                                                playlist_view,
                                                tap_times: [None; _],
                                                tap_index: 0,
                                            })
                                            .into()
                                        }
                                    }),
                            )
                    )
                    .child(div().w_px().bg(self.theme.navbar_outline).h_full())
                    .child(
                        div()
                            .flex()
                            .gap_0p5()
                            .items_center()
                            .flex_col()
                            .flex_shrink()
                            .text_sm()
                            .line_height(DefiniteLength::Fraction(0.7))
                            .child(div().child("SIG").text_xs())
                            .child(
                                div()
                                    .flex()
                                    .gap_0p5()
                                    .items_center()
                                    .child(AdjustableInput {
                                        value: playlist_view.audio.playlist().time_signature.beats_per_measure,
                                        theme: Arc::clone(&self.theme),
                                        set: Arc::new({
                                            let playlist = self.playlist.downgrade();
                                            let app = self.app.downgrade();
                                            move |beats_per_measure, cx| {
                                                playlist
                                                    .update(cx, |playlist, cx| {
                                                        let beats_per_measure = beats_per_measure.max(1);
                                                        playlist.zoom.width = playlist.zoom.width / playlist.audio.update_beats_per_measure(|_| beats_per_measure) as f32 * beats_per_measure as f32;
                                                        cx.notify();
                                                    })
                                                    .unwrap();
                                                cx.notify(app.entity_id());
                                            }
                                        }),
                                        scale: 0.01,
                                        name: "Beats per measure".into(),
                                        default: 4,
                                    })
                                    .child("/")
                                    .child(AdjustableInput {
                                        value: playlist_view.audio.playlist().time_signature.beat_value,
                                        theme: Arc::clone(&self.theme),
                                        set: Arc::new({
                                            let playlist = self.playlist.downgrade();
                                            let app = self.app.downgrade();
                                            move |beat_value, cx| {
                                                playlist
                                                    .update(cx, |playlist, cx| {
                                                        playlist.audio.update_beat_value(|_| beat_value.max(1));
                                                        cx.notify();
                                                    })
                                                    .unwrap();
                                                cx.notify(app.entity_id());
                                            }
                                        }),
                                        scale: 0.02,
                                        name: "Beat value".into(),
                                        default: 4,
                                    }),
                            ),
                    )
                    .child(div().w_px().bg(self.theme.navbar_outline).h_full())
                    .child(
                        div()
                            .p_1()
                            .debug_blue()
                            .rounded_md()
                            .child(img(PLAY_ICON).text_color(gpui::green()).size_6())
                            .on_mouse_down(MouseButton::Left, {
                                let playlist = self.playlist.clone();
                                move |_, _, cx| {
                                    playlist.update(cx, |playlist, cx| {
                                        if playlist.audio.playing() {
                                            playlist.audio.stop();
                                        } else {
                                            playlist.audio.play();
                                        }
                                        cx.notify();
                                    });
                                }
                            })
                    ),
            )
    }
}

// BPM widget
struct Bpm {
    playlist_view: Entity<PlaylistView>,
    tap_times: [Option<Instant>; 10],
    tap_index: usize,
}
impl Render for Bpm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let playlist = self.playlist_view.read(cx);
        let theme = &playlist.theme;
        div()
            .flex_col()
            .bg(theme.notification_background)
            .text_color(theme.bg_text)
            .items_center()
            .gap_4()
            .p_4()
            .rounded_md()
            .shadow_md()
            .block_mouse_except_scroll()
            .child(div().child("+").on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.playlist_view.update(cx, |playlist, _| {
                        playlist.audio.update_tempo(|tempo| Tempo::from_bpm(tempo.bpm() + 1.));
                    });
                    cx.notify();
                }),
            ))
            .child(div().text_3xl().child(format!("{:.02}", playlist.audio.playlist().tempo.bpm())))
            .child(div().text_sm().child(format!("Rounded: {}", playlist.audio.playlist().tempo.bpm().round())))
            .child(div().child("-").on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.playlist_view.update(cx, |playlist, _| {
                        playlist.audio.update_tempo(|tempo| Tempo::from_bpm(tempo.bpm() - 1.));
                    });
                    cx.notify();
                }),
            ))
            .child(div().child("Tap").on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.tap_times[view.tap_index] = Some(Instant::now());
                    view.tap_index = (view.tap_index + 1) % view.tap_times.len();
                    let mut times = view.tap_times.iter().copied().flatten().collect_vec();
                    let other = times.split_off(view.tap_index);
                    if times.len() + other.len() < 2 {
                        return;
                    }
                    view.playlist_view.update(cx, |playlist, _| {
                        playlist.audio.update_tempo(|_| {
                            Tempo::from_bpm(
                                other
                                    .into_iter()
                                    .chain(times)
                                    .tuple_windows()
                                    .map(|(a, b)| 60. / (b - a).as_secs_f64())
                                    .fold((0., 0.), |(sum, count), bpm| (sum + bpm, count + 1.))
                                    .pipe(|(sum, count)| sum / count),
                            )
                        })
                    });
                    cx.notify();
                }),
            ))
            .child(
                div()
                    .child("Use rounded")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|view, _, _, cx| {
                            view.playlist_view.update(cx, |playlist, _| {
                                playlist.audio.update_tempo(|tempo| Tempo::from_bpm(tempo.bpm().round()));
                            });
                            cx.notify();
                        }),
                    )
            )
    }
}

// Volt GUI app setup
struct Drag(Option<DragInner>);
struct DragInner {
    start: Point<Pixels>,
    item: AnyDrag,
}
impl Global for Drag {}
struct Volt {
    browser: Entity<BrowserView>,
    playlist: Entity<PlaylistView>,
    browser_size: f32,
    theme: Arc<ThemeColors>,
}
impl Volt {
    fn new(cx: &mut App, theme: Arc<ThemeColors>) -> Self {
        Self {
            browser: cx.new(|_| BrowserView::new(Arc::clone(&theme))),
            playlist: cx.new(|_| PlaylistView::new(Arc::clone(&theme))),
            browser_size: 0.3,
            theme,
        }
    }
}
impl Render for Volt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(self.theme.central_background)
            .text_color(self.theme.bg_text)
            .font_family("Inter")
            .on_action({
                let playlist = self.playlist.clone();
                move |_: &TogglePlay, _, cx| {
                    playlist.update(cx, |playlist, cx| {
                        if playlist.audio.playing() {
                            playlist.audio.stop();
                        } else {
                            playlist.audio.play();
                        }
                        cx.notify();
                    });
                }
            })
            .child(Navbar {
                playlist: self.playlist.clone(),
                theme: Arc::clone(&self.theme),
                app: cx.entity(),
            })
            .child(
                div()
                    .flex_grow()
                    .flex()
                    .min_h_0()
                    .child(div().flex().flex_col().w(DefiniteLength::Fraction(self.browser_size)).child(self.browser.clone()))
                    .child({
                        struct Payload(Point<Pixels>, f32);
                        div()
                            .w_8()
                            .px_3()
                            .cursor_col_resize()
                            .mx_neg_3()
                            .flex()
                            .flex_col()
                            .child(div().flex_grow().bg(self.theme.browser_outline))
                            .id("separator")
                            .on_drag(Payload(window.mouse_position(), self.browser_size), |_, _, _, cx| cx.new(|_| Empty))
                            .on_drag_move(cx.listener(|app, event: &gpui::DragMoveEvent<Payload>, window, cx| {
                                app.browser_size = (event.drag(cx).1 + ((event.event.position - event.drag(cx).0).x) / window.bounds().size.width).clamp(0.1, 0.9);
                            }))
                            .pipe(deferred)
                    })
                    .child(self.playlist.clone()),
            )
            .child(
                div()
                    .flex()
                    .h_8()
                    .gap_4()
                    .flex_shrink_0()
                    .p_2()
                    .items_center()
                    .text_sm()
                    .child(div().child(concat!("Volt ", env!("CARGO_PKG_VERSION"))))
                    .child(div().child("Highly WIP, alpha build")),
            )
    }
}

// Main

const NAVBAR_ICON: &str = "navbar-icon";
const PLAY_ICON: &str = "play-icon";
const FILE_OTHER_ICON: &str = "file-other-icon";
const FILE_AUDIO_ICON: &str = "file-audio-icon";

fn main() {
    struct Assets;
    impl AssetSource for Assets {
        fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
            match path {
                NAVBAR_ICON => Ok(Some(Cow::Borrowed(include_bytes!("images/icons/navbar-icon.svg")))),
                PLAY_ICON => Ok(Some(Cow::Borrowed(include_bytes!("images/icons/play-icon.svg")))),
                FILE_OTHER_ICON => Ok(Some(Cow::Borrowed(include_bytes!("images/icons/file_other.svg")))),
                FILE_AUDIO_ICON => Ok(Some(Cow::Borrowed(include_bytes!("images/icons/file_audio.svg")))),
                _ => unimplemented!(),
            }
        }

        fn list(&self, _: &str) -> gpui::Result<Vec<SharedString>> {
            Ok(Vec::new())
        }
    }

    application().with_assets(Assets).run(|cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![
                Cow::Borrowed(include_bytes!("fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf")),
                Cow::Borrowed(include_bytes!("fonts/inter/Inter.ttf")),
            ])
            .unwrap();
        cx.bind_keys([KeyBinding::new("space", TogglePlay, None)]);
        cx.set_global(Drag(None));
        let bounds = Bounds::centered(None, size(px(1200.), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                app_id: Some("sh.thered.Volt".into()),
                titlebar: Some(TitlebarOptions{
                    title: Some("Volt".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| Volt::new(cx, Arc::new(gray()))),
        )
        .unwrap();
        cx.activate(true);
    });
}
