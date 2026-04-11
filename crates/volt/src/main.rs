#![warn(clippy::pedantic, clippy::nursery, clippy::allow_attributes_without_reason, clippy::undocumented_unsafe_blocks, clippy::clone_on_ref_ptr)]
use std::{
    array::from_fn,
    borrow::Cow,
    fmt::Display,
    ops::{DerefMut, Sub, SubAssign},
    sync::{Arc, Mutex},
    time::Instant,
};

use blerp::{
    processing::time::{Beats, Samples, Tempo, Time},
    streaming::playlist::{Playlist, PlaylistAudio},
};
use cpal::{
    default_host,
    traits::{DeviceTrait, HostTrait},
};
use gpui::{
    App, AssetSource, Bounds, Context, Entity, KeyBinding, MouseButton, PathBuilder, Pixels, Point, Rems, Rgba, SharedString, Size, WeakEntity, Window, WindowBounds, WindowOptions, actions, canvas,
    div, hsla, img, linear_color_stop, linear_gradient, pattern_slash, point, prelude::*, px, rems, rgb, rgba, size,
};
use gpui_platform::application;
use itertools::Itertools;
use tap::{Conv, Pipe, Tap};

struct PlaylistView {
    audio: PlaylistAudio,
    /// The zoom factor for the playlist view. `size(16., 4.)` means a measure is 16 rems wide and a track is 4 rems high.
    zoom: Size<Rems>,
    snapping: Snapping,
    pan: Point<Rems>,

    hovered_position: Option<Point<Pixels>>,

    theme: Arc<ThemeColors>,
}

impl PlaylistView {
    pub fn new(theme: Arc<ThemeColors>) -> Self {
        let mut inner = PlaylistAudio::new();
        let device = default_host().default_output_device().unwrap();
        let config = device.default_output_config().unwrap().config();
        inner.device_out(&device, &config);
        Self {
            audio: inner,
            zoom: size(rems(16.), rems(4.)),
            snapping: Snapping::default(),
            hovered_position: None,
            pan: Point::new(rems(0.), rems(0.)),
            theme,
        }
    }

    fn zoom_pixels(&self, rem_size: Pixels) -> Size<Pixels> {
        self.zoom.map(|length| length.to_pixels(rem_size))
    }

    fn beats_to_width(&self, beats: Beats) -> Rems {
        rems(beats.f64() as f32 / self.audio.playlist().time_signature.beats_per_measure as f32 * self.zoom.width.0)
    }

    fn width_to_beats(&self, width: Pixels, rem_size: Pixels) -> Beats {
        Beats::new(width.to_f64() / self.zoom_pixels(rem_size).width.to_f64() * f64::from(self.audio.playlist().time_signature.beats_per_measure))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Snapping {
    None,
    /// Snaps to the nearest beat divided by the given number, normally a power of 2.
    Beats {
        divisor: u32,
    },
}

impl Default for Snapping {
    fn default() -> Self {
        Self::Beats { divisor: 4 }
    }
}

actions!([TogglePlay]);

impl Render for PlaylistView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.audio.playing() {
            window.request_animation_frame();
        }
        let tempo = self.audio.playlist().tempo;
        let playhead_x = self.beats_to_width(self.audio.playhead().beats(tempo)) + self.pan.x;
        div()
            .flex()
            .flex_col()
            .flex_grow()
            .child(
                div()
                    .flex()
                    .gap_8()
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(div().child(format!(
                                "{} / {}",
                                self.audio.playlist().time_signature.beats_per_measure,
                                self.audio.playlist().time_signature.beat_unit
                            )))
                            .child(if self.audio.playing() { "Playing" } else { "Stopped" }),
                    )
                    .child(
                        div().flex().gap_2().children(
                            [
                                ("Play", &PlaylistAudio::play as &dyn Fn(&mut PlaylistAudio)),
                                ("Pause", &PlaylistAudio::stop as &dyn Fn(&mut PlaylistAudio)),
                                ("Stop", &PlaylistAudio::stop as &dyn Fn(&mut PlaylistAudio)),
                            ]
                            .map(|(text, method)| {
                                div()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            method(&mut view.audio);
                                            cx.notify();
                                        }),
                                    )
                                    .child(text)
                            }),
                        ),
                    ),
            )
            .child(
                div()
                    .relative()
                    .h_8()
                    .id("ruler")
                    .on_hover(cx.listener(|view, bool, _, cx| {
                        if !bool {
                            view.hovered_position = None;
                        }
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|view, _: &gpui::MouseDownEvent, window, cx| {
                            let Some(hovered_position) = view.hovered_position else { return };
                            view.audio
                                .seek(Time::Beats(view.width_to_beats(hovered_position.x - view.pan.x.to_pixels(window.rem_size()), window.rem_size())));
                            cx.notify();
                        }),
                    )
                    .on_mouse_move(cx.listener(|view, event: &gpui::MouseMoveEvent, window, cx| {
                        let hovered_position = event.position;
                        view.hovered_position = Some(hovered_position);
                        if event.dragging() {
                            view.audio
                                .seek(Time::Beats(view.width_to_beats(hovered_position.x - view.pan.x.to_pixels(window.rem_size()), window.rem_size())));
                        }
                        cx.notify();
                    }))
                    .children(
                        (0..=self.width_to_beats(window.bounds().size.width, window.rem_size()).u32() + self.audio.playlist().time_signature.beats_per_measure).map(|beat| {
                            let x = (self.beats_to_width(Beats::from_u32(beat)) + rems(self.pan.x.0.rem_euclid(self.zoom.width.0) - self.zoom.width.0)).to_pixels(window.rem_size());
                            let beat = self.width_to_beats(x - self.pan.x.to_pixels(window.rem_size()), window.rem_size()).f32().round() as i32;
                            div().absolute().top_0().left(x).child(format!(
                                "{}{}.{}",
                                if beat < 0 { "-" } else { "" },
                                (beat / self.audio.playlist().time_signature.beats_per_measure.cast_signed()).abs(),
                                (beat % self.audio.playlist().time_signature.beats_per_measure.cast_signed()).abs()
                            ))
                        }),
                    )
                    .child(
                        div()
                            .absolute()
                            .flex()
                            .gap_2()
                            .text_color(self.theme.playhead)
                            .bg(self.theme.central_background)
                            .child(format!("{:.02} s", self.audio.playhead().beats(tempo).f64() / tempo.bps()))
                            .child(format!(
                                "{}.{}",
                                self.audio.playhead().beats(tempo).u32() / self.audio.playlist().time_signature.beats_per_measure,
                                self.audio.playhead().beats(tempo).u32() % self.audio.playlist().time_signature.beats_per_measure,
                            ))
                            .left(playhead_x),
                    )
                    .children(self.hovered_position.map(|hovered_position| {
                        let next = self.width_to_beats(hovered_position.x - self.pan.x.to_pixels(window.rem_size()), window.rem_size());
                        div()
                            .absolute()
                            .flex()
                            .gap_2()
                            .text_color(self.theme.playhead_hover)
                            .bg(self.theme.central_background)
                            .child(format!("{:.02} s", next.f64() / tempo.bps()))
                            .child(format!(
                                "{}.{}",
                                next.u32() / self.audio.playlist().time_signature.beats_per_measure,
                                next.u32() % self.audio.playlist().time_signature.beats_per_measure,
                            ))
                            .left(hovered_position.x)
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .relative()
                    .size_full()
                    .gap_1()
                    .id("tracks")
                    .overflow_y_scroll()
                    .on_pinch(cx.listener(|view, event: &gpui::PinchEvent, window, cx| {
                        let delta = event.delta;
                        let old = view.zoom;
                        view.zoom = view.zoom.map(|length| length * (delta + 1.));
                        view.zoom.width.0 = view.zoom.width.0.max(8.);
                        view.zoom.height.0 = view.zoom.height.0.max(2.);
                        view.pan = point(
                            rems((event.position.x - (event.position.x - view.pan.x.to_pixels(window.rem_size())) * view.zoom.width.0 / old.width.0) / window.rem_size()),
                            rems((event.position.y - (event.position.y - view.pan.y.to_pixels(window.rem_size())) * view.zoom.height.0 / old.height.0) / window.rem_size()),
                        );
                        cx.notify();
                    }))
                    .on_scroll_wheel(cx.listener(|view, event: &gpui::ScrollWheelEvent, window, cx| {
                        if event.control {
                            let factor = event.delta.pixel_delta(window.rem_size()).scale(0.001).map(|length| length.as_f32() + 1.);
                            let old = view.zoom;
                            view.zoom.width.0 = (view.zoom.width.0 * factor.x).max(8.);
                            view.zoom.height.0 = (view.zoom.height.0 * factor.y).max(2.);
                            let factor = point(view.zoom.width.0 / old.width.0, view.zoom.height.0 / old.height.0);
                            view.pan = point(
                                rems((event.position.x - (event.position.x - view.pan.x.to_pixels(window.rem_size())) * factor.x) / window.rem_size()),
                                rems((event.position.y - (event.position.y - view.pan.y.to_pixels(window.rem_size())) * factor.y) / window.rem_size()),
                            );
                        } else {
                            view.pan = view.pan + event.delta.pixel_delta(window.rem_size()).map(|length| rems(length / window.rem_size()));
                        }
                        cx.notify();
                    }))
                    .child(
                        canvas(|_, _, _| {}, {
                            let view = cx.entity().downgrade();
                            move |bounds, (), window, cx| {
                                let Some(view) = view.upgrade().map(|entity| entity.read(cx)) else { return };
                                let beats_per_measure = view.audio.playlist().time_signature.beats_per_measure;
                                let (measure_builder, beat_builder) = (0..=view.width_to_beats(bounds.size.width, window.rem_size()).u32() + beats_per_measure).fold(
                                    (PathBuilder::stroke(px(2.)), PathBuilder::stroke(px(1.))),
                                    |(mut measure_builder, mut beat_builder), beat| {
                                        let builder = if beat % beats_per_measure == 0 { &mut measure_builder } else { &mut beat_builder };
                                        let top = bounds.origin.tap_mut(|point| {
                                            point.x += (view.beats_to_width(Beats::from_u32(beat)) + rems(view.pan.x.0.rem_euclid(view.zoom.width.0) - view.zoom.width.0)).to_pixels(window.rem_size());
                                        });
                                        builder.move_to(top);
                                        builder.line_to(top.tap_mut(|point| point.y = bounds.bottom()));
                                        (measure_builder, beat_builder)
                                    },
                                );
                                window.paint_path(measure_builder.build().unwrap(), view.theme.playlist_bar);
                                window.paint_path(beat_builder.build().unwrap(), view.theme.playlist_beat);
                            }
                        })
                        .absolute()
                        .inset_0()
                        .h_full(),
                    )
                    .children(
                        self.hovered_position
                            .map(|hovered_position| div().w_px().bg(self.theme.playhead_hover).absolute().top_0().bottom_0().left(hovered_position.x)),
                    )
                    .child(div().w_px().bg(self.theme.playhead).absolute().top_0().bottom_0().left(playhead_x))
                    .children(self.audio.playlist().preview.into_iter().flat_map(|preview| {
                        let timing = preview.as_beats(tempo);
                        [timing.start, timing.end].map(|time| {
                            div()
                                .w_px()
                                .bg(self.theme.preview)
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left(rems(time.f32() / self.audio.playlist().time_signature.beats_per_measure as f32 * self.zoom.width.0) + self.pan.x)
                        })
                    }))
                    .children(self.audio.playlist().tracks().iter().enumerate().map(|(index, track)| {
                        div()
                            .relative()
                            .h(self.zoom.height)
                            .children(track.clips().iter().map(|clip| {
                                let beats = clip.timing.as_beats(tempo);
                                let start = self.beats_to_width(beats.start) + self.pan.x;
                                let length = self.beats_to_width(beats.len());
                                div()
                                    .absolute()
                                    .left(start)
                                    .top_0()
                                    .h_full()
                                    .w(length)
                                    .bg(pattern_slash(hsla(0., 0., 0.2, 1.), 2., 5.))
                                    .overflow_hidden()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(self.theme.navbar_outline)
                                    .child(
                                        div()
                                            .absolute()
                                            .left_0()
                                            .top_0()
                                            .bottom_0()
                                            .w(self.beats_to_width(clip.data_len().beats(tempo)))
                                            .bg(self.theme.central_background),
                                    )
                                    .child(
                                        canvas(|_, _, _| {}, {
                                            let view = cx.entity().downgrade();
                                            let clip = clip.clone();
                                            move |bounds, (), window, cx| {
                                                let view = view.upgrade().unwrap().read(cx);
                                                let window_size = clip.data_len().samples(tempo).f64() / view.beats_to_width(clip.data_len().beats(tempo)).to_pixels(window.rem_size()).to_f64();
                                                let left = bounds.left();
                                                let bounds = bounds.intersect(&window.bounds());
                                                if bounds.is_empty() {
                                                    return;
                                                }
                                                let paths = ((bounds.left() - left).conv::<u32>()
                                                    ..view
                                                        .beats_to_width(clip.data_len().beats(tempo))
                                                        .to_pixels(window.rem_size())
                                                        .min(bounds.left() - left + bounds.size.width)
                                                        .conv::<u32>())
                                                    .fold(
                                                        from_fn(|_| PathBuilder::stroke(px(2.)).tap_mut(|builder| builder.move_to(bounds.center().tap_mut(|point| point.x = left)))),
                                                        |mut builders: [_; 2], x| {
                                                            let x = f64::from(x);
                                                            let start = x * window_size;
                                                            let end = start + window_size;
                                                            let range = clip.sample_range((Time::Samples(Samples::new(start))..Time::Samples(Samples::new(end))).into(), tempo);
                                                            for (sample, builder) in [range.start, range.end].iter().zip(&mut builders) {
                                                                builder.line_to(point(x.conv::<Pixels>() + left, bounds.center().y + bounds.size.height / 2. * *sample));
                                                            }
                                                            builders
                                                        },
                                                    )
                                                    .map(PathBuilder::build)
                                                    .map(Result::unwrap);
                                                for path in paths {
                                                    window.paint_path(path, view.theme.accent);
                                                }
                                            }
                                        })
                                        .size_full(),
                                    )
                            }))
                            .child(
                                div()
                                    .absolute()
                                    .right_0()
                                    .top_0()
                                    .h_full()
                                    .p_4()
                                    .bg(self.theme.central_background)
                                    .rounded_md()
                                    .border_1()
                                    .border_color(self.theme.navbar_outline)
                                    .id(format!("track-{}", index + 1))
                                    .overflow_y_scroll()
                                    .child(format!("Track {}", index + 1))
                                    .child(format!("Gain {:.02}", track.gain)),
                            )
                    })),
            )
    }
}

struct Volt {
    playlist: Entity<PlaylistView>,
    theme: Arc<ThemeColors>,
}

impl Volt {
    fn new(cx: &mut App, theme: Arc<ThemeColors>) -> Self {
        Self {
            playlist: cx.new(|_| PlaylistView::new(Arc::clone(&theme))),
            theme,
        }
    }
}

impl Render for Volt {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(self.theme.central_background)
            .text_color(self.theme.bg_text)
            .font_family("Inter")
            .on_action({
                let playlist = self.playlist.downgrade();
                move |_: &TogglePlay, _, cx| {
                    playlist
                        .update(cx, |playlist, cx| {
                            if playlist.audio.playing() {
                                playlist.audio.stop();
                            } else {
                                playlist.audio.play();
                            }
                            cx.notify();
                        })
                        .unwrap();
                }
            })
            .child(
                div()
                    .flex()
                    .h_16()
                    .p_2()
                    .gap_2()
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
                            .p_2()
                            .gap_2()
                            .items_center()
                            .border_1()
                            .border_color(self.theme.navbar_outline)
                            .rounded_md()
                            .bg(self.theme.navbar_widget)
                            .child(img("navbar-icon").size_8())
                            .child(div().w_px().bg(self.theme.navbar_outline).h_full())
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .items_center()
                                    .children(["File", "Edit", "View", "Help"].map(|name| div().child(name).py_1().px_2().rounded_md().id(name).hover(|style| style.bg(self.theme.hover)))),
                            ),
                    )
                    .child(
                        div()
                            .flex_grow()
                            .flex()
                            .p_2()
                            .border_1()
                            .border_color(self.theme.navbar_outline)
                            .rounded_md()
                            .items_center()
                            .bg(self.theme.navbar_widget)
                            .child(img("play-icon").size_8().on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|app, _, _, cx| {
                                    app.playlist.update(cx, |playlist, cx| {
                                        if playlist.audio.playing() {
                                            playlist.audio.stop();
                                        } else {
                                            playlist.audio.play();
                                        }
                                        cx.notify();
                                    });
                                }),
                            ))
                            .child({
                                div().child(format!("BPM: {:.02}", self.playlist.read(cx).audio.playlist().tempo.bpm())).id("bpm").hoverable_tooltip({
                                    let playlist = self.playlist.downgrade();
                                    move |_, cx| {
                                        const TAP_WINDOW: usize = 10;
                                        struct Tooltip {
                                            playlist_view: WeakEntity<PlaylistView>,
                                            tap_times: [Option<Instant>; TAP_WINDOW],
                                            tap_index: usize,
                                        }
                                        impl Render for Tooltip {
                                            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                                                let playlist = self.playlist_view.upgrade().unwrap().read(cx);
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
                                                        cx.listener(|tooltip, _, _, cx| {
                                                            tooltip.playlist_view.upgrade().unwrap().update(cx, |playlist, _| {
                                                                playlist.audio.update_tempo(|tempo| Tempo::from_bpm(tempo.bpm() + 1.));
                                                            });
                                                            cx.notify();
                                                        }),
                                                    ))
                                                    .child(
                                                        div()
                                                            .text_3xl()
                                                            .cursor_row_resize()
                                                            .on_scroll_wheel(cx.listener(|tooltip, event: &gpui::ScrollWheelEvent, window, cx| {
                                                                tooltip.playlist_view.upgrade().unwrap().update(cx, |playlist, _| {
                                                                    let previous = playlist
                                                                        .audio
                                                                        .update_tempo(|tempo| Tempo::from_bpm(tempo.bpm() + event.delta.pixel_delta(window.rem_size()).y.to_f64()))
                                                                        .bpm();
                                                                    let (previous, next) = (previous, playlist.audio.playlist().tempo.bpm());
                                                                    playlist.audio.seek(Time::Samples(Samples::new(playlist.audio.playhead().f64() * previous / next)));
                                                                });
                                                                cx.notify();
                                                            }))
                                                            .child(format!("{:.02}", playlist.audio.playlist().tempo.bpm())),
                                                    )
                                                    .child(div().child("-").on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(|tooltip, _, _, cx| {
                                                            tooltip.playlist_view.upgrade().unwrap().update(cx, |playlist, _| {
                                                                playlist.audio.update_tempo(|tempo| Tempo::from_bpm(tempo.bpm() - 1.));
                                                            });
                                                            cx.notify();
                                                        }),
                                                    ))
                                                    .child(div().child("Tap").on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(|tooltip, _, _, cx| {
                                                            tooltip.tap_times[tooltip.tap_index] = Some(Instant::now());
                                                            tooltip.tap_index = (tooltip.tap_index + 1) % TAP_WINDOW;
                                                            let mut times = tooltip.tap_times.iter().copied().flatten().collect_vec();
                                                            let other = times.split_off(tooltip.tap_index);
                                                            if times.len() + other.len() < 2 {
                                                                return;
                                                            }
                                                            tooltip.playlist_view.upgrade().unwrap().update(cx, |playlist, _| {
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
                                            }
                                        }
                                        let playlist = playlist.clone();
                                        cx.new(move |_| Tooltip {
                                            playlist_view: playlist,
                                            tap_times: [None; TAP_WINDOW],
                                            tap_index: 0,
                                        })
                                        .into()
                                    }
                                })
                            }),
                    ),
            )
            .child(self.playlist.clone())
            .child(
                div()
                    .flex()
                    .h_8()
                    .gap_4()
                    .flex_shrink_0()
                    .p_2()
                    .items_center()
                    .child(div().child(concat!("Volt ", env!("CARGO_PKG_VERSION")))),
            )
    }
}

struct ThemeColors {
    accent: Rgba,
    navbar_background_gradient_top: Rgba,
    navbar_background_gradient_bottom: Rgba,
    navbar_outline: Rgba,
    navbar_widget: Rgba,
    notification_background: Rgba,
    notification_border: Rgba,
    central_background: Rgba,
    browser: Rgba,
    browser_outline: Rgba,
    browser_selected_button_fg: Rgba,
    browser_unselected_button_fg: Rgba,
    browser_unselected_hover_button_fg: Rgba,
    browser_invalid_name_bg: Rgba,
    browser_unselected_button_fg_invalid: Rgba,
    browser_unselected_hover_button_fg_invalid: Rgba,
    browser_folder_text: Rgba,
    browser_folder_hover_text: Rgba,
    playlist_bar: Rgba,
    playlist_beat: Rgba,
    bg_text: Rgba,
    command_palette: Rgba,
    command_palette_border: Rgba,
    command_palette_text: Rgba,
    command_palette_placeholder_text: Rgba,
    playhead: Rgba,
    playhead_hover: Rgba,
    preview: Rgba,
    hover: Rgba,
}

#[expect(clippy::unreadable_literal, reason = "these are hex codes")]
fn default() -> ThemeColors {
    ThemeColors {
        accent: rgb(0xb6afff),
        navbar_background_gradient_top: rgb(0x1e2132),
        navbar_background_gradient_bottom: rgb(0x171825),
        navbar_outline: rgb(0x453f67),
        navbar_widget: rgba(0x07081520),
        notification_background: rgb(0x1d1b2b),
        notification_border: rgb(0x3d3b4b),
        central_background: rgb(0x171825),
        browser: rgb(0x171825),
        browser_outline: rgb(0x28243e),
        browser_selected_button_fg: rgb(0xffcf7b),
        browser_unselected_button_fg: rgb(0x646d88),
        browser_unselected_hover_button_fg: rgb(0x8591b5),
        browser_invalid_name_bg: rgba(0xff000010),
        browser_unselected_button_fg_invalid: rgb(0xa46d88),
        browser_unselected_hover_button_fg_invalid: rgb(0xf591b5),
        browser_folder_text: rgb(0x928ea7),
        browser_folder_hover_text: rgb(0xece9ff),
        playlist_bar: rgb(0x4c495f),
        playlist_beat: rgb(0x2e2b3f),
        bg_text: rgb(0x646987),
        command_palette: rgb(0x1d1b2b),
        command_palette_border: rgb(0x3d3b4b),
        command_palette_text: rgb(0x928ea7),
        command_palette_placeholder_text: rgba(0x928ea740),
        playhead: rgb(0xf00000),
        playhead_hover: rgba(0xf000007f),
        preview: rgb(0xf000f0),
        hover: rgba(0xffffff20),
    }
}

fn main() {
    struct Assets;
    impl AssetSource for Assets {
        fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
            match path {
                "navbar-icon" => Ok(Some(Cow::Borrowed(include_bytes!("images/icons/navbar-icon.svg")))),
                "play-icon" => Ok(Some(Cow::Borrowed(include_bytes!("images/icons/play-icon.svg")))),
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
        let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Maximized(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| Volt::new(cx, Arc::new(default()))),
        )
        .unwrap();
        cx.activate(true);
    });
}
