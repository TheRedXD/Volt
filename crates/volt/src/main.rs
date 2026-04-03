#![warn(clippy::pedantic, clippy::nursery, clippy::allow_attributes_without_reason, clippy::undocumented_unsafe_blocks, clippy::clone_on_ref_ptr)]
// use human_panic::setup_panic;
// use image::{ImageFormat, ImageReader};
// use info::handle_args;
// use std::{
//     io::{BufReader, Cursor},
//     rc::Rc,
//     sync::mpsc::{Sender, channel},
//     time::Instant,
// };
// use tap::{Pipe, Tap};
// use visual::{
//     browser::Browser,
//     central::Central,
//     navbar::navbar,
//     notification::NotificationDrawer,
//     status::status,
// };

// use crate::visual::{dialog::dialog, theme::ThemeColors};
// use crate::visual::notification::Notification;
// use crate::visual::palette::Palette;
// use volt_waveform;

// mod audio;
// mod info;
// mod shortcuts;
// mod timings;
// mod visual;

use std::borrow::Cow;

use blerp::{
    processing::time::{Beats, Tempo, TimeSignature},
    streaming::{clip::ClipTiming, playlist::Playlist},
};
use cpal::{
    default_host,
    traits::{DeviceTrait, HostTrait},
};
use gpui::{
    App, Application, Bounds, Context, Entity, Menu, MenuItem, MouseButton, Path, PathBuilder, Point, PromptLevel, Rems, SharedString, Size, Window, WindowBounds, WindowOptions, canvas, div, font,
    hsla, linear_gradient, pattern_slash, point, prelude::*, px, rems, rgb, size, white,
};
use gpui_platform::application;
use tap::Tap;

struct PlaylistView {
    pub inner: Playlist,
    /// The zoom factor for the playlist view. `size(16., 4.)` means a measure is 16 rems wide and a track is 4 rems high.
    pub zoom: Size<Rems>,
    pub snapping: Snapping,
}

impl PlaylistView {
    pub fn new() -> Self {
        let mut inner = Playlist::new();
        let device = default_host().default_output_device().unwrap();
        let config = device.default_output_config().unwrap().config();
        inner.device_out(&device, &config);
        Self {
            inner,
            zoom: size(rems(16.), rems(4.)),
            snapping: Snapping::default(),
        }
    }

    fn beats_to_width(&self, beats: Beats) -> Rems {
        rems(beats.beats() as f32 / self.inner.time_signature.beats_per_measure as f32 * self.zoom.width.0)
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

impl Render for PlaylistView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.inner.playing() {
            window.request_animation_frame();
        }
        let tempo = *self.inner.tempo.lock().unwrap();
        let playhead_x = self.beats_to_width(self.inner.playhead().beats(tempo));
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
                            .child(
                                div()
                                    .on_scroll_wheel(cx.listener(|view, event: &gpui::ScrollWheelEvent, window, cx| {
                                        view.inner.stop();
                                        {
                                            let mut tempo = view.inner.tempo.lock().unwrap();
                                            *tempo = Tempo::from_bpm(tempo.bpm() - event.delta.pixel_delta(window.rem_size()).y.to_f64());
                                        }
                                        cx.notify();
                                    }))
                                    .child(format!("{} bpm (scroll to change)", tempo.bpm())),
                            )
                            .child(div().child(format!("{} / {}", self.inner.time_signature.beats_per_measure, self.inner.time_signature.beat_unit)))
                            .child(if self.inner.playing() { "Playing" } else { "Stopped" }),
                    )
                    .child(
                        div().flex().gap_2().children(
                            [
                                ("Play", &Playlist::play as &dyn Fn(&mut Playlist)),
                                ("Pause", &Playlist::pause as &dyn Fn(&mut Playlist)),
                                ("Stop", &Playlist::stop as &dyn Fn(&mut Playlist)),
                            ]
                            .map(|(text, method)| {
                                div()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|view, _, _, cx| {
                                            method(&mut view.inner);
                                            cx.notify();
                                        }),
                                    )
                                    .child(text)
                            }),
                        ),
                    ),
            )
            .child(
                div().relative().h_16().child(
                    div()
                        .absolute()
                        .flex()
                        .flex_col()
                        .text_color(rgb(0xf00000))
                        .child(format!("{:.02} s", self.inner.playhead().beats(tempo).beats() / tempo.bps()))
                        .child(format!(
                            "{}.{}",
                            self.inner.playhead().beats(tempo).beats() as u32 / self.inner.time_signature.beats_per_measure,
                            self.inner.playhead().beats(tempo).beats() as u32 % self.inner.time_signature.beats_per_measure,
                        ))
                        .left(playhead_x),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .relative()
                    .size_full()
                    .on_pinch(cx.listener(|view, event: &gpui::PinchEvent, _, app| {
                        let delta = event.delta;
                        view.zoom = view.zoom.map(|length| length * (delta + 1.));
                        view.zoom.width.0 = view.zoom.width.0.max(8.);
                        view.zoom.height.0 = view.zoom.height.0.max(2.);
                        app.notify();
                    }))
                    .on_scroll_wheel(cx.listener(|view, event: &gpui::ScrollWheelEvent, window, app| {
                        if event.control {
                            let pixel_delta = event.delta.pixel_delta(window.rem_size()).scale(0.01);
                            view.zoom.width.0 += pixel_delta.x.as_f32();
                            view.zoom.height.0 += pixel_delta.y.as_f32();
                            view.zoom.width.0 = view.zoom.width.0.max(8.);
                            view.zoom.height.0 = view.zoom.height.0.max(2.);
                        }
                        app.notify();
                    }))
                    .child(
                        canvas(|_, _, _| {}, {
                            let zoom = self.zoom;
                            let beats_per_measure = self.inner.time_signature.beats_per_measure;
                            move |bounds, (), window, _| {
                                let (measure_builder, beat_builder) = (0..(bounds.size.width / zoom.width.to_pixels(window.rem_size()) * beats_per_measure as f32).ceil() as u32).fold(
                                    (PathBuilder::stroke(px(2.)), PathBuilder::stroke(px(1.))),
                                    |(mut measure_builder, mut beat_builder), measure| {
                                        let builder = if measure % beats_per_measure == 0 { &mut measure_builder } else { &mut beat_builder };
                                        let top = bounds
                                            .origin
                                            .tap_mut(|point| point.x += measure as f32 * zoom.width.to_pixels(window.rem_size()) / beats_per_measure as f32);
                                        builder.move_to(top);
                                        builder.line_to(top.tap_mut(|point| point.y = bounds.size.height));
                                        (measure_builder, beat_builder)
                                    },
                                );
                                window.paint_path(measure_builder.build().unwrap(), rgb(0x373737));
                                window.paint_path(beat_builder.build().unwrap(), rgb(0x303030));
                            }
                        })
                        .absolute()
                        .inset_0(),
                    )
                    .children(self.inner.tracks().iter().map(|track| {
                        div().relative().border_1().border_color(rgb(0x303030)).h(self.zoom.height).children(track.clips().iter().map(|clip| {
                            let beats = clip.timing.as_beats(tempo);
                            let start = self.beats_to_width(beats.start);
                            let length = self.beats_to_width(beats.end - beats.start);
                            div()
                                .absolute()
                                .left(start)
                                .top_0()
                                .h_full()
                                .w(length)
                                .bg(pattern_slash(hsla(0., 0., 0.2, 1.), 2., 5.))
                                .overflow_hidden()
                                .child(div().absolute().left_0().top_0().bottom_0().w(self.beats_to_width(clip.data_len().beats(tempo))).bg(rgb(0x404040)))
                                .child(format!("{} - {}", clip.timing.as_beats(tempo).start.beats(), clip.timing.as_beats(tempo).end.beats()))
                        }))
                    }))
                    .child(div().w_px().bg(rgb(0xf00000)).absolute().top_0().bottom_0().left(playhead_x))
                    .children(self.inner.preview.lock().unwrap().iter().flat_map(|preview| {
                        let timing = preview.as_beats(tempo);
                        [timing.start, timing.end].map(|time| {
                            div()
                                .w_px()
                                .bg(rgb(0xf000f0))
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left(rems(time.beats() as f32 / self.inner.time_signature.beats_per_measure as f32 * self.zoom.width.0))
                        })
                    })),
            )
    }
}

struct Volt {
    playlist: Entity<PlaylistView>,
}

impl Volt {
    fn new(cx: &mut App) -> Self {
        Self {
            playlist: cx.new(|_| PlaylistView::new()),
        }
    }
}

impl Render for Volt {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_8()
            .bg(rgb(0x101010))
            .text_color(white())
            .font_family("IBM Plex Mono")
            .child(self.playlist.clone())
            .child(div().flex().justify_end().h_8().border_t_1().flex_shrink_0().border_color(rgb(0x202020)))
    }
}

fn main() {
    application().run(|cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![
                Cow::Borrowed(include_bytes!("fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf")),
                Cow::Borrowed(include_bytes!("fonts/inter/Inter.ttf")),
            ])
            .unwrap();

        let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Maximized(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| Volt::new(cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}
