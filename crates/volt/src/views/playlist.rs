use std::{array::from_fn, sync::Arc};

use blerp::{processing::time::{Beats, Samples, Time}, streaming::playlist::PlaylistAudio};
use cpal::{
    default_host,
    traits::{DeviceTrait, HostTrait},
};
use gpui::{Bounds, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, PathBuilder, Pixels, Point, Rems, Render, Size, StatefulInteractiveElement, Styled, Window, canvas, deferred, div, hsla, pattern_slash, point, px, rems, size};
use tap::{Conv, Pipe, Tap};

use crate::{components::adjustable_input::AdjustableInput, theme::ThemeColors};

pub struct PlaylistView {
    pub audio: PlaylistAudio,
    /// The zoom factor for the playlist view. `size(16., 4.)` means a measure is 16 rems wide and a track is 4 rems high.
    pub zoom: Size<Rems>,
    pub snapping: Snapping,
    pub pan: Point<Rems>,

    pub hovered_position: Option<Point<Pixels>>,
    pub bounds: Bounds<Pixels>,

    pub theme: Arc<ThemeColors>,
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
            bounds: Bounds::default(),
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
            .relative()
            .overflow_hidden()
            .child(
                canvas(|_, _, _| {}, {
                    let view = cx.entity().downgrade();
                    move |bounds, (), _, cx| {
                        view.update(cx, |view, _| {
                            view.bounds = bounds;
                        })
                        .unwrap();
                    }
                })
                .absolute()
                .inset_0(),
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
                        let hovered_position = event.position.tap_mut(|position| position.x -= view.bounds.left());
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
                        let position = event.position.relative_to(&view.bounds.origin);
                        view.pan = point(
                            rems((position.x - (position.x - view.pan.x.to_pixels(window.rem_size())) * view.zoom.width.0 / old.width.0) / window.rem_size()),
                            rems((position.y - (position.y - view.pan.y.to_pixels(window.rem_size())) * view.zoom.height.0 / old.height.0) / window.rem_size()),
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
                            let position = event.position.relative_to(&view.bounds.origin);
                            view.pan = point(
                                rems((position.x - (position.x - view.pan.x.to_pixels(window.rem_size())) * factor.x) / window.rem_size()),
                                rems((position.y - (position.y - view.pan.y.to_pixels(window.rem_size())) * factor.y) / window.rem_size()),
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
                                    .child(div().flex().gap_4().items_center().child("Gain").child(AdjustableInput {
                                        value: track.gain,
                                        theme: Arc::clone(&self.theme),
                                        set: {
                                            let view = cx.entity().downgrade();
                                            Box::new(move |gain, cx| {
                                                view.upgrade().unwrap().update(cx, move |view, _| {
                                                    view.audio.update_playlist(|playlist| playlist.set_track_gain(index, gain));
                                                });
                                                cx.notify(view.entity_id());
                                            })
                                        },
                                        scale: 0.01,
                                        name: format!("Track {} gain", index + 1).into(),
                                    }))
                                    .pipe(deferred),
                            )
                    }))
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
                    })),
            )
    }
}
