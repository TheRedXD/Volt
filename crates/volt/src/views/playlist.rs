use std::{array::from_fn, collections::{HashMap, HashSet}, sync::Arc, ops::Add, range::Range};

use blerp::{Beats, Clip, ClipTiming, ClipTimingBeats, ClipTimingSamples, PlaylistAudio, SAMPLE_RATE, Samples, Time};
use cpal::{
    SampleRate, default_host,
    traits::{DeviceTrait, HostTrait},
};
use gpui::{
    AbsoluteLength, AppContext, Bounds, Context, DefiniteLength, InteractiveElement, IntoElement, Length, MouseButton, ParentElement, PathBuilder, Pixels, Point, Rems, Render, Size, StatefulInteractiveElement, Styled, Window, canvas, deferred, div, hsla, pattern_slash, point, px, rems, rgba, size
};
use gpui_component::scroll::ScrollableElement;
use itertools::Itertools;
use tap::{Conv, Pipe, Tap};

use crate::{
    components::adjustable_input::AdjustableInput,
    theme::ThemeColors,
    views::browser::{Entry, EntryDragPayload},
};

#[derive(Clone, Default)]
pub struct TimeSelection {
    pub start_track: usize,
    pub end_track: usize,
    pub start_beats: f64,
    pub end_beats: f64,
}

impl TimeSelection {
    pub fn normalized(&self) -> (std::ops::RangeInclusive<usize>, std::ops::Range<f64>) {
        let track_range = self.start_track.min(self.end_track)..=self.start_track.max(self.end_track);
        let time_range = self.start_beats.min(self.end_beats)..self.start_beats.max(self.end_beats);
        (track_range, time_range)
    }
}

pub struct PlaylistView {
    pub audio: PlaylistAudio,
    pub zoom: Size<Rems>,
    pub snapping: Snapping,
    pub pan: Point<Rems>,
    pub target_pan: Point<Rems>,
    pub auto_scroll: bool,

    pub hovered_position: Option<Point<Pixels>>,
    pub bounds: Bounds<Pixels>,

    pub theme: Arc<ThemeColors>,

    pub clip_waveforms: HashMap<usize, Vec<Vec<Vec<Range<f32>>>>>,
    pub selected_clips: HashSet<usize>,
    pub dragging_clips: Option<(usize, Point<Pixels>, HashMap<usize, (usize, ClipTiming)>)>,

    pub last_scrollbar_mouse_pos: Option<Point<Pixels>>,
    pub resizing_clip_start: Option<Point<Pixels>>,
    pub time_selection: Option<TimeSelection>,
    pub time_selection_start_pos: Option<Point<Pixels>>,
    pub focus_handle: Option<gpui::FocusHandle>,
}

const MIPMAP_HIGH: usize = 1;

#[derive(Clone)]
struct PlayheadScrub;
#[derive(Clone)]
struct ScrollbarDrag;
#[derive(Clone)]
struct ClipDrag;

#[derive(Clone)]
struct TimeSelectionDrag(usize);

#[derive(Clone)]
struct ClipResizeLeft {
    clip_id: usize,
    initial_timing: ClipTiming,
}

#[derive(Clone)]
struct ClipResizeRight {
    clip_id: usize,
    initial_timing: ClipTiming,
}

impl PlaylistView {
    pub fn new(theme: Arc<ThemeColors>) -> Self {
        let mut inner = PlaylistAudio::new();
        let host = default_host();
        let host_id = host.id();
        println!("{}", host_id.name());
        let device = host.default_output_device().unwrap();

        device.supported_output_configs().iter_mut().for_each(|item| {
            println!("{:?}", item.next().unwrap().max_sample_rate());
        });
        let config = device
            .default_output_config()
            .unwrap()
            .config()
            .tap_mut(|config| config.sample_rate = SampleRate(SAMPLE_RATE as u32));
        inner.device_out(&device, &config);
        Self {
            audio: inner,
            zoom: size(rems(16.), rems(4.)),
            snapping: Snapping::default(),
            hovered_position: None,
            pan: Point::new(rems(0.), rems(0.)),
            target_pan: Point::new(rems(0.), rems(0.)),
            auto_scroll: true,
            bounds: Bounds::default(),
            theme,
            clip_waveforms: HashMap::new(),
            selected_clips: HashSet::new(),
            dragging_clips: None,
            last_scrollbar_mouse_pos: None,
            resizing_clip_start: None,
            time_selection: None,
            time_selection_start_pos: None,
            focus_handle: None,
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

    pub fn handle_delete(&mut self) {
        if !self.selected_clips.is_empty() {
            let ids: Vec<_> = self.selected_clips.iter().copied().collect();
            self.audio.delete_clips(&ids);
            self.selected_clips.clear();
        } else if let Some(sel) = &self.time_selection {
            let (track_range, time_range) = sel.normalized();
            if time_range.is_empty() { return; }
            self.audio.delete_time_selection(track_range, time_range);
        }
    }

    pub fn handle_duplicate(&mut self) {
        if !self.selected_clips.is_empty() {
            let ids: Vec<_> = self.selected_clips.iter().copied().collect();
            let new_ids = self.audio.duplicate_clips(&ids);
            self.selected_clips.clear();
            for id in new_ids {
                self.selected_clips.insert(id);
            }
        } else if let Some(sel) = &self.time_selection {
            let (track_range, time_range) = sel.normalized();
            if time_range.is_empty() { return; }
            self.audio.duplicate_time_selection(track_range, time_range);

            let new_start = sel.end_beats.max(sel.start_beats);
            let duration = (sel.end_beats - sel.start_beats).abs();
            let new_end = new_start + duration;
            self.time_selection = Some(TimeSelection {
                start_track: sel.start_track,
                end_track: sel.end_track,
                start_beats: new_start,
                end_beats: new_end,
            });
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Snapping {
    None,
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
        let mut is_animating = false;
        let tempo = self.audio.playlist().tempo;
        let rem_size = window.rem_size();

        let focus_handle = self.focus_handle.get_or_insert_with(|| cx.focus_handle()).clone();

        let playhead_beats = self.audio.playhead().beats(tempo);
        let playhead_absolute_x = self.beats_to_width(playhead_beats);

        let view_width_rems = self.bounds.size.width.as_f32() / rem_size.as_f32();

        let culling_view_width_rems = if self.bounds.size.width.as_f32() == 0.0 {
            10000.0
        } else {
            view_width_rems
        };

        let half_screen_rems = view_width_rems / 2.0;

        if self.auto_scroll && self.audio.playing() {
            let ideal_pan_x = half_screen_rems - playhead_absolute_x.0;

            if ideal_pan_x < 0. {
                self.pan.x.0 = ideal_pan_x;
                self.target_pan.x.0 = ideal_pan_x;
            }
        }

        if self.pan != self.target_pan {
            let dy = self.target_pan.y.0 - self.pan.y.0;

            let dx = if self.auto_scroll && self.audio.playing() && (half_screen_rems - playhead_absolute_x.0) < 0. {
                0.0
            } else {
                self.target_pan.x.0 - self.pan.x.0
            };

            if dx.abs() < 0.01 && dy.abs() < 0.01 {
                self.pan = self.target_pan;
            } else {
                self.pan.x.0 += dx * 0.2;
                self.pan.y.0 += dy * 0.2;
                is_animating = true;
            }
        }

        if self.audio.playing() || is_animating {
            window.request_animation_frame();
        }

        let playhead_x = playhead_absolute_x + self.pan.x;
        let theme = Arc::clone(&self.theme);

        let max_clip_end = self.audio.playlist().tracks().iter()
            .flat_map(|t| t.clips().iter().map(|c| c.timing.as_beats(tempo).end.f64()))
            .fold(0.0_f64, f64::max);

        let total_beats = max_clip_end.max(playhead_beats.f64()).max(32.0);
        let total_width_px = self.beats_to_width(Beats::new(total_beats)).to_pixels(window.rem_size()).as_f32();
        let view_width_px = self.bounds.size.width.as_f32();
        let scroll_x_px = -self.pan.x.to_pixels(window.rem_size()).as_f32();

        let scrollbar_width_frac = (view_width_px / total_width_px.max(1.0)).clamp(0.02, 1.0);
        let scrollbar_left_frac = (scroll_x_px / total_width_px.max(1.0)).clamp(0., 1.0 - scrollbar_width_frac);

        div()
            .flex()
            .flex_col()
            .flex_grow()
            .relative()
            .overflow_hidden()
            .ml_6()
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
                    .min_h_8()
                    .id("ruler")
                    .on_hover(cx.listener(|view, bool, _, cx| {
                        if !bool {
                            view.hovered_position = None;
                        }
                        cx.notify();
                    }))
                    .on_mouse_down(MouseButton::Left, cx.listener(|view, event: &gpui::MouseDownEvent, window, cx| {
                        let x_pos = event.position.x;
                        view.audio.seek(Time::Beats(
                            view.width_to_beats((x_pos - view.bounds.left() - view.pan.x.to_pixels(window.rem_size())).max(Pixels::from(0.)), window.rem_size()),
                        ));
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
                    .on_drag(PlayheadScrub, |_, _, _, cx| cx.new(|_| gpui::Empty))
                    .on_drag_move(cx.listener(|view, event: &gpui::DragMoveEvent<PlayheadScrub>, window, cx| {
                        let x_pos = event.event.position.x;
                        view.audio.seek(Time::Beats(
                            view.width_to_beats((x_pos - view.bounds.left() - view.pan.x.to_pixels(window.rem_size())).max(Pixels::from(0.)), window.rem_size()),
                        ));
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
                            .text_color(theme.playhead)
                            .bg(theme.central_background)
                            .child(format!("{:.02} s", self.audio.playhead().beats(tempo).f64() / tempo.bps()))
                            .child(format!(
                                "{}.{}",
                                self.audio.playhead().beats(tempo).u32() / self.audio.playlist().time_signature.beats_per_measure,
                                self.audio.playhead().beats(tempo).u32() % self.audio.playlist().time_signature.beats_per_measure,
                            ))
                            .child(
                                div()
                                    .absolute()
                                    .top_px()
                                    .child(
                                        canvas(
                                            move |_, _, _| {},
                                            {
                                                let value = theme.clone();
                                                move |bounds, _, window, _cx| {
                                                    let mut builder = PathBuilder::fill();
                                                    let top_left = point(bounds.left(), bounds.top());
                                                    let top_right = point(bounds.right(), bounds.top());
                                                    let bottom_center = point(bounds.center().x, bounds.bottom());
                                                    builder.move_to(top_left);
                                                    builder.line_to(top_right);
                                                    builder.line_to(bottom_center);
                                                    if let Ok(path) = builder.build() {
                                                        window.paint_path(path, Arc::clone(&value).playhead);
                                                    }
                                                }
                                            }
                                        )
                                        .absolute()
                                        .top(px(24.))
                                        .left(px(-8.))
                                        .w(px(17.))
                                        .h(px(8.))
                                    )
                            )
                            .left(playhead_x),
                    )
                    .children(self.hovered_position.map(|hovered_position| {
                        let next = self.width_to_beats(hovered_position.x - self.pan.x.to_pixels(window.rem_size()), window.rem_size());
                        div()
                            .absolute()
                            .flex()
                            .gap_2()
                            .text_color(theme.playhead_hover)
                            .bg(theme.central_background)
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
                    .id("tracks_container")
                    .track_focus(&focus_handle)
                    .overflow_y_hidden()
                    .on_mouse_down(MouseButton::Left, cx.listener(|view, _, window, cx| {
                        view.selected_clips.clear();
                        view.time_selection = None;
                        if let Some(focus) = &view.focus_handle {
                            focus.focus(window, cx);
                        }
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(|view, event: &gpui::KeyDownEvent, _, cx| {
                        let key = event.keystroke.key.as_str();
                        if key.eq_ignore_ascii_case("delete") || key.eq_ignore_ascii_case("backspace") {
                            view.handle_delete();
                            cx.notify();
                        } else if key.eq_ignore_ascii_case("d") && event.keystroke.modifiers.control {
                            view.handle_duplicate();
                            cx.notify();
                        } else if key.eq_ignore_ascii_case("escape") {
                            view.selected_clips.clear();
                            view.time_selection = None;
                            cx.notify();
                        }
                    }))
                    .on_drag_move(cx.listener(|view, event: &gpui::DragMoveEvent<ClipDrag>, window, cx| {
                        let Some((leader_id, start_pos, initial_state)) = &view.dragging_clips else { return };
                        let dx = event.event.position.x - start_pos.x;
                        let dy = event.event.position.y - start_pos.y;

                        let mut dx_beats = view.width_to_beats(dx.abs(), window.rem_size());
                        if dx < Pixels::ZERO { dx_beats = Beats::new(-dx_beats.f64()); }

                        let track_height = view.zoom.height.to_pixels(window.rem_size());
                        let mut track_offset = (dy.as_f32() / track_height.as_f32()).round() as i32;

                        let mut min_track = usize::MAX;
                        let mut max_track = 0;
                        for (_, (t_idx, _)) in initial_state {
                            min_track = min_track.min(*t_idx);
                            max_track = max_track.max(*t_idx);
                        }
                        let num_tracks = view.audio.playlist().tracks().len();
                        let max_offset_up = -(min_track as i32);
                        let max_offset_down = (num_tracks.saturating_sub(1) as i32) - (max_track as i32);
                        track_offset = track_offset.clamp(max_offset_up, max_offset_down);

                        let tempo = view.audio.playlist().tempo;

                        if let Some((_, leader_initial_timing)) = initial_state.get(leader_id) {
                            let leader_initial = leader_initial_timing.as_beats(tempo);
                            let mut leader_new_start = leader_initial.start.f64() + dx_beats.f64();

                            if !event.event.modifiers.alt {
                                if let Snapping::Beats { divisor } = view.snapping {
                                    let snap_interval = 1.0 / divisor as f64;
                                    leader_new_start = (leader_new_start / snap_interval).round() * snap_interval;
                                }
                            }
                            leader_new_start = leader_new_start.max(0.0);

                            let actual_dx_beats = leader_new_start - leader_initial.start.f64();

                            let mut new_positions = HashMap::new();
                            for (id, (initial_t_idx, initial_timing)) in initial_state {
                                let initial_beats = initial_timing.as_beats(tempo);
                                let new_start = (initial_beats.start.f64() + actual_dx_beats).max(0.0);
                                let len = initial_beats.end.f64() - initial_beats.start.f64();

                                let target_track_idx = (*initial_t_idx as i32 + track_offset) as usize;

                                new_positions.insert(*id, (target_track_idx, ClipTiming::Beats(ClipTimingBeats {
                                    start: Beats::new(new_start),
                                    end: Beats::new(new_start + len),
                                    offset: initial_beats.offset,
                                })));
                            }

                            view.audio.move_clips(new_positions);
                            cx.notify();
                        }
                    }))
                    .on_drag_move(cx.listener(|view, event: &gpui::DragMoveEvent<ClipResizeLeft>, window, cx| {
                        let drag = &event.drag(cx);
                        let Some(start_pos) = view.resizing_clip_start else { return };
                        let dx = event.event.position.x - start_pos.x;
                        let tempo = view.audio.playlist().tempo;
                        let initial = drag.initial_timing.as_beats(tempo);
                        let sign = if dx < Pixels::ZERO { -1.0 } else { 1.0 };
                        let dx_beats = view.width_to_beats(dx.abs(), window.rem_size()).f64() * sign;

                        let mut new_start = initial.start.f64() + dx_beats;
                        if !event.event.modifiers.alt {
                            if let Snapping::Beats { divisor } = view.snapping {
                                let snap_interval = 1.0 / divisor as f64;
                                new_start = (new_start / snap_interval).round() * snap_interval;
                            }
                        }
                        new_start = new_start.max(0.0).min(initial.end.f64() - 0.01);

                        let actual_dx = new_start - initial.start.f64();
                        let new_offset = (initial.offset.f64() + actual_dx).max(0.0);

                        view.audio.update_clip_timings(HashMap::from([(
                            drag.clip_id,
                            ClipTiming::Beats(ClipTimingBeats {
                                start: Beats::new(new_start),
                                end: initial.end,
                                offset: Beats::new(new_offset),
                            }),
                        )]));
                        cx.notify();
                    }))
                    .on_drag_move(cx.listener(|view, event: &gpui::DragMoveEvent<ClipResizeRight>, window, cx| {
                        let drag = &event.drag(cx);
                        let Some(start_pos) = view.resizing_clip_start else { return };
                        let dx = event.event.position.x - start_pos.x;
                        let tempo = view.audio.playlist().tempo;
                        let initial = drag.initial_timing.as_beats(tempo);
                        let sign = if dx < Pixels::ZERO { -1.0 } else { 1.0 };
                        let dx_beats = view.width_to_beats(dx.abs(), window.rem_size()).f64() * sign;

                        let mut new_end = initial.end.f64() + dx_beats;
                        if !event.event.modifiers.alt {
                            if let Snapping::Beats { divisor } = view.snapping {
                                let snap_interval = 1.0 / divisor as f64;
                                new_end = (new_end / snap_interval).round() * snap_interval;
                            }
                        }
                        new_end = new_end.max(initial.start.f64() + 0.01);

                        view.audio.update_clip_timings(HashMap::from([(
                            drag.clip_id,
                            ClipTiming::Beats(ClipTimingBeats {
                                start: initial.start,
                                end: Beats::new(new_end),
                                offset: initial.offset,
                            }),
                        )]));
                        cx.notify();
                    }))
                    .on_drag_move(cx.listener(|view, event: &gpui::DragMoveEvent<TimeSelectionDrag>, window, cx| {
                        if let Some(selection) = &view.time_selection {
                            let x_pos = event.event.position.x - view.bounds.left() - view.pan.x.to_pixels(window.rem_size());
                            let mut beats = view.width_to_beats(x_pos.max(Pixels::ZERO), window.rem_size()).f64();

                            if !event.event.modifiers.alt {
                                if let Snapping::Beats { divisor } = view.snapping {
                                    let snap_interval = 1.0 / divisor as f64;
                                    beats = (beats / snap_interval).round() * snap_interval;
                                }
                            }

                            let start_y = view.time_selection_start_pos.map(|p| p.y).unwrap_or(event.event.position.y);
                            let dy = event.event.position.y - start_y;
                            let track_height = view.zoom.height.to_pixels(window.rem_size());
                            let track_offset = (dy.as_f32() / track_height.as_f32()).round() as i32;
                            let max_track = view.audio.playlist().tracks().len().saturating_sub(1);
                            let start_track = selection.start_track;

                            let end_track = (start_track as i32 + track_offset).clamp(0, max_track as i32) as usize;

                            if let Some(selection) = &mut view.time_selection {
                                selection.end_beats = beats;
                                selection.end_track = end_track;
                            }
                            cx.notify();
                        }
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_grow()
                            .relative()
                            .size_auto()
                            .gap_0()
                            .id("tracks")
                            .overflow_x_hidden()
                            .on_pinch(cx.listener(|view, event: &gpui::PinchEvent, window, cx| {
                                let delta = event.delta;
                                let old = view.zoom;
                                view.zoom = view.zoom.map(|length| length * (delta + 1.));
                                view.zoom.width.0 = view.zoom.width.0.max(8.);
                                view.zoom.height.0 = view.zoom.height.0.max(2.);

                                let position = event.position.relative_to(&view.bounds.origin);
                                let pos_x = position.x.as_f32();
                                let pos_y = position.y.as_f32();

                                let factor_x = view.zoom.width.0 / old.width.0;
                                let factor_y = view.zoom.height.0 / old.height.0;

                                let old_target_x = view.target_pan.x.to_pixels(window.rem_size()).as_f32();
                                let old_target_y = view.target_pan.y.to_pixels(window.rem_size()).as_f32();
                                let old_current_x = view.pan.x.to_pixels(window.rem_size()).as_f32();
                                let old_current_y = view.pan.y.to_pixels(window.rem_size()).as_f32();

                                view.target_pan = point(
                                    rems((pos_x - (pos_x - old_target_x) * factor_x) / window.rem_size().as_f32()),
                                    rems((pos_y - (pos_y - old_target_y) * factor_y) / window.rem_size().as_f32()),
                                );
                                view.pan = point(
                                    rems((pos_x - (pos_x - old_current_x) * factor_x) / window.rem_size().as_f32()),
                                    rems((pos_y - (pos_y - old_current_y) * factor_y) / window.rem_size().as_f32()),
                                );

                                view.target_pan.x.0 = view.target_pan.x.0.min(0.);
                                view.target_pan.y.0 = view.target_pan.y.0.min(0.);
                                view.pan.x.0 = view.pan.x.0.min(0.);
                                view.pan.y.0 = view.pan.y.0.min(0.);
                                cx.notify();
                            }))
                            .on_scroll_wheel(cx.listener(|view, event: &gpui::ScrollWheelEvent, window, cx| {
                                if event.control {
                                    let factor = event.delta.pixel_delta(window.rem_size()).scale(0.001).map(|length| length.as_f32() + 1.);
                                    let old = view.zoom;
                                    view.zoom.width.0 = (view.zoom.width.0 * factor.x).max(2.);
                                    view.zoom.height.0 = (view.zoom.height.0 * factor.y).max(2.);

                                    let position = event.position.relative_to(&view.bounds.origin);
                                    let pos_x = position.x.as_f32();
                                    let pos_y = position.y.as_f32();

                                    let factor_x = view.zoom.width.0 / old.width.0;
                                    let factor_y = view.zoom.height.0 / old.height.0;

                                    let old_target_x = view.target_pan.x.to_pixels(window.rem_size()).as_f32();
                                    let old_target_y = view.target_pan.y.to_pixels(window.rem_size()).as_f32();
                                    let old_current_x = view.pan.x.to_pixels(window.rem_size()).as_f32();
                                    let old_current_y = view.pan.y.to_pixels(window.rem_size()).as_f32();

                                    view.target_pan = point(
                                        rems((pos_x - (pos_x - old_target_x) * factor_x) / window.rem_size().as_f32()),
                                        rems((pos_y - (pos_y - old_target_y) * factor_y) / window.rem_size().as_f32()),
                                    );
                                    view.pan = point(
                                        rems((pos_x - (pos_x - old_current_x) * factor_x) / window.rem_size().as_f32()),
                                        rems((pos_y - (pos_y - old_current_y) * factor_y) / window.rem_size().as_f32()),
                                    );

                                    view.target_pan.x.0 = view.target_pan.x.0.min(0.);
                                    view.target_pan.y.0 = view.target_pan.y.0.min(0.);
                                    view.pan.x.0 = view.pan.x.0.min(0.);
                                    view.pan.y.0 = view.pan.y.0.min(0.);
                                } else {
                                    let delta = event.delta.pixel_delta(window.rem_size());
                                    if delta.x.as_f32().abs() > 0.0 {
                                        view.auto_scroll = false;
                                    }
                                    view.target_pan.x = rems(view.target_pan.x.0 + (delta.x.as_f32() / window.rem_size().as_f32()));
                                    view.target_pan.y = rems(view.target_pan.y.0 + (delta.y.as_f32() / window.rem_size().as_f32()));
                                    view.target_pan.x.0 = view.target_pan.x.0.min(0.);
                                    view.target_pan.y.0 = view.target_pan.y.0.min(0.);
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
                            .children(self.audio.playlist().tracks().iter().enumerate().map(|(track_index, track)| {
                                div()
                                    .id(format!("track_bg_{}", track_index))
                                    .border_b_1()
                                    .border_color(rgba(0xffffff04))
                                    .relative()
                                    .h(self.zoom.height)
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |view, event: &gpui::MouseDownEvent, window, cx| {
                                        cx.stop_propagation();
                                        view.selected_clips.clear();
                                        let x_pos = event.position.x - view.bounds.left() - view.pan.x.to_pixels(window.rem_size());
                                        let mut beats = view.width_to_beats(x_pos.max(Pixels::ZERO), window.rem_size()).f64();

                                        if !event.modifiers.alt {
                                            if let Snapping::Beats { divisor } = view.snapping {
                                                let snap_interval = 1.0 / divisor as f64;
                                                beats = (beats / snap_interval).round() * snap_interval;
                                            }
                                        }

                                        view.time_selection = Some(TimeSelection {
                                            start_track: track_index,
                                            end_track: track_index,
                                            start_beats: beats,
                                            end_beats: beats,
                                        });
                                        view.time_selection_start_pos = Some(event.position);
                                        if let Some(focus) = &view.focus_handle {
                                            focus.focus(window, cx);
                                        }
                                        cx.notify();
                                    }))
                                    .on_drag(TimeSelectionDrag(track_index), |_, _, _, cx| cx.new(|_| gpui::Empty))
                                    .children(track.clips().iter().filter_map(|clip| {
                                        let beats = clip.timing.as_beats(tempo);
                                        let start = self.beats_to_width(beats.start) + self.pan.x;
                                        let length = self.beats_to_width(beats.len());

                                        if start.0 + length.0 < 0.0 || start.0 > culling_view_width_rems {
                                            return None;
                                        }

                                        let is_selected = self.selected_clips.contains(&clip.id);

                                        Some(div()
                                            .absolute()
                                            .left(start)
                                            .top_0()
                                            .h_full()
                                            .w(length)
                                            .bg(pattern_slash(hsla(0., 0., 0.2, 1.), 2., 5.))
                                            .overflow_hidden()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(if is_selected { theme.playhead } else { theme.navbar_outline })
                                            .child(
                                                div()
                                                    .id(format!("inner track clip thing {} {}", track_index, clip.id))
                                                    .h(px(16.))
                                                    .w_full()
                                                    .bg(if is_selected { theme.playhead } else { theme.navbar_outline })
                                                    .text_color(theme.bg_text)
                                                    .text_xs()
                                                    .px_1()
                                                    .child(clip.name.clone())
                                                    .on_mouse_down(MouseButton::Left, cx.listener({
                                                        let clip_id = clip.id;
                                                        move |view, event: &gpui::MouseDownEvent, window, cx| {
                                                            cx.stop_propagation();
                                                            view.time_selection = None;
                                                            if event.modifiers.shift {
                                                                if view.selected_clips.contains(&clip_id) {
                                                                    view.selected_clips.remove(&clip_id);
                                                                } else {
                                                                    view.selected_clips.insert(clip_id);
                                                                }
                                                            } else {
                                                                if !view.selected_clips.contains(&clip_id) {
                                                                    view.selected_clips.clear();
                                                                    view.selected_clips.insert(clip_id);
                                                                }
                                                            }

                                                            let mut initial = HashMap::new();
                                                            for (t_idx, t) in view.audio.playlist().tracks().iter().enumerate() {
                                                                for c in t.clips() {
                                                                    if view.selected_clips.contains(&c.id) {
                                                                        initial.insert(c.id, (t_idx, c.timing));
                                                                    }
                                                                }
                                                            }
                                                            view.dragging_clips = Some((clip_id, event.position, initial));

                                                            if let Some(focus) = &view.focus_handle {
                                                                focus.focus(window, cx);
                                                            }

                                                            cx.notify();
                                                        }
                                                    }))
                                                    .on_mouse_up(MouseButton::Left, cx.listener(|view, _, _, _| {
                                                        view.dragging_clips = None;
                                                    }))
                                                    .on_mouse_down(MouseButton::Right, cx.listener({
                                                        let clip_id = clip.id;
                                                        move |view, _, window, cx| {
                                                            cx.stop_propagation();
                                                            view.time_selection = None;
                                                            let mut to_delete = view.selected_clips.clone();
                                                            to_delete.insert(clip_id);
                                                            view.audio.delete_clips(&to_delete.into_iter().collect::<Vec<_>>());
                                                            view.selected_clips.clear();

                                                            if let Some(focus) = &view.focus_handle {
                                                                focus.focus(window, cx);
                                                            }

                                                            cx.notify();
                                                        }
                                                    }))
                                                    .on_drag(ClipDrag, |_, _, _, cx| cx.new(|_| gpui::Empty))
                                            )
                                            .child(
                                                div()
                                                    .absolute()
                                                    .left_0()
                                                    .top(px(16.))
                                                    .bottom_0()
                                                    .w(self.beats_to_width(clip.data_len().beats(tempo)))
                                                    .bg(theme.central_background)
                                                    .child(
                                                        canvas(|_, _, _| {}, {
                                                            let mut clip = clip.clone();
                                                            let width = self.beats_to_width(clip.data_len().beats(tempo)).to_pixels(window.rem_size());
                                                            let window_size = clip.data_len().samples(tempo).f64() / width.to_f64();
                                                            let view = cx.entity().downgrade();
                                                            let theme = Arc::clone(&theme);
                                                            move |clip_bounds, (), window, cx| {
                                                                let Some(view_entity) = view.upgrade() else { return; };

                                                                let level = view_entity
                                                                    .update(cx, |view, _| {
                                                                        let levels = view.clip_waveforms.entry(clip.id).or_insert_with(|| {
                                                                            let base_channels = clip.base_minmax_mipmap(tempo, MIPMAP_HIGH).unwrap();

                                                                            base_channels.into_iter().map(|base| {
                                                                                let max = 10;
                                                                                (0..max).fold(Vec::with_capacity(max).tap_mut(|levels| levels.push(base)), |mut levels, _| {
                                                                                    let from = levels.last().unwrap();
                                                                                    levels.push(
                                                                                        from.chunks(2)
                                                                                            .map(|chunk| match chunk {[a, b] => Range::from(a.start.min(b.start)..a.end.max(b.end)),
                                                                                                [a] => Range::from(a.start..a.end),[] => Range::default(),
                                                                                                _ => unreachable!(),
                                                                                            })
                                                                                            .collect(),
                                                                                    );
                                                                                    levels
                                                                                })
                                                                            }).collect()
                                                                        });
                                                                        let level = (window_size / MIPMAP_HIGH as f64).log2().floor().max(0.) as usize;
                                                                        level.min(levels[0].len() - 1)
                                                                    });

                                                                let view_reader = view_entity.read(cx);
                                                                let Some(channels_waveforms) = view_reader.clip_waveforms.get(&clip.id) else { return; };

                                                                let num_channels = channels_waveforms.len();
                                                                if num_channels == 0 { return; }
                                                                let channel_height = clip_bounds.size.height / num_channels as f32;

                                                                let clip_left = clip_bounds.left();
                                                                let visible_bounds = clip_bounds.intersect(&window.bounds());
                                                                if visible_bounds.is_empty() {
                                                                    return;
                                                                }

                                                                let step = window_size / (MIPMAP_HIGH << level) as f64;

                                                                for (c, channel_levels) in channels_waveforms.iter().enumerate() {
                                                                    let waveform = channel_levels.get(level).unwrap();
                                                                    let center_y = clip_bounds.top() + channel_height * c as f32 + channel_height / 2.0;

                                                                    let start_x_usize = (visible_bounds.left() - clip_left).conv::<usize>();
                                                                    let end_x_usize = width.min(visible_bounds.left() - clip_left + visible_bounds.size.width).conv::<usize>();

                                                                    if start_x_usize >= end_x_usize {
                                                                        continue;
                                                                    }

                                                                    let paths = (start_x_usize..end_x_usize)
                                                                        .fold(
                                                                            from_fn(|i| {
                                                                                let mut b = PathBuilder::stroke(px(2.));
                                                                                let start_range = waveform.get((start_x_usize as f64 * step) as usize).copied().unwrap_or_default();
                                                                                let start_y = if i == 0 { start_range.start } else { start_range.end };
                                                                                b.move_to(point(start_x_usize.conv::<Pixels>() + clip_left, center_y + channel_height / 2. * start_y));
                                                                                b
                                                                            }),
                                                                            |mut builders:[_; 2], x| {
                                                                                let range = waveform.get((x as f64 * step) as usize).copied().unwrap_or_default();
                                                                                builders[0].line_to(point(x.conv::<Pixels>() + clip_left, center_y + channel_height / 2. * range.start));
                                                                                builders[1].line_to(point(x.conv::<Pixels>() + clip_left, center_y + channel_height / 2. * range.end));
                                                                                builders
                                                                            },
                                                                        )
                                                                        .map(PathBuilder::build)
                                                                        .map(Result::unwrap);

                                                                    for path in paths {
                                                                        window.paint_path(path, theme.accent);
                                                                    }
                                                                }
                                                            }
                                                        })
                                                        .size_full(),
                                                    )
                                            )
                                            .child(
                                                div()
                                                    .id(format!("clip_resize_left_{}", clip.id))
                                                    .absolute()
                                                    .left_0()
                                                    .top_0()
                                                    .bottom_0()
                                                    .w(px(8.))
                                                    .cursor_col_resize()
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|view, event: &gpui::MouseDownEvent, window, cx| {
                                                        cx.stop_propagation();
                                                        view.resizing_clip_start = Some(event.position);
                                                        if let Some(focus) = &view.focus_handle {
                                                            focus.focus(window, cx);
                                                        }
                                                    }))
                                                    .on_mouse_up(MouseButton::Left, cx.listener(|view, _, _, _| {
                                                        view.resizing_clip_start = None;
                                                    }))
                                                    .on_drag(ClipResizeLeft { clip_id: clip.id, initial_timing: clip.timing }, |_, _, _, cx| cx.new(|_| gpui::Empty)),
                                            )
                                            .child(
                                                div()
                                                    .id(format!("clip_resize_right_{}", clip.id))
                                                    .absolute()
                                                    .right_0()
                                                    .top_0()
                                                    .bottom_0()
                                                    .w(px(8.))
                                                    .cursor_col_resize()
                                                    .on_mouse_down(MouseButton::Left, cx.listener(|view, event: &gpui::MouseDownEvent, window, cx| {
                                                        cx.stop_propagation();
                                                        view.resizing_clip_start = Some(event.position);
                                                        if let Some(focus) = &view.focus_handle {
                                                            focus.focus(window, cx);
                                                        }
                                                    }))
                                                    .on_mouse_up(MouseButton::Left, cx.listener(|view, _, _, _| {
                                                        view.resizing_clip_start = None;
                                                    }))
                                                    .on_drag(ClipResizeRight { clip_id: clip.id, initial_timing: clip.timing }, |_, _, _, cx| cx.new(|_| gpui::Empty)),
                                            ))
                                    }))
                                    .child({
                                        div()
                                            .absolute()
                                            .right_0()
                                            .top_0()
                                            .h_full()
                                            .w(gpui::Pixels::from(150.))
                                            .bg(theme.central_background)
                                            // .rounded_md()
                                            .border_1()
                                            .border_color(theme.navbar_outline)
                                            .id(track_index)
                                            .on_mouse_down(MouseButton::Left, cx.listener(|_, _: &gpui::MouseDownEvent, _, cx| {
                                                cx.stop_propagation();
                                            }))
                                            .overflow_y_scroll()
                                            .line_height(DefiniteLength::Fraction(0.8))
                                            .text_sm()
                                            .p_1()
                                            .child(format!("Track {}", track_index + 1))
                                            .child(div().text_sm().flex().gap_1().items_center().child("Gain").child(AdjustableInput {
                                                value: 20. * track.gain.log10(),
                                                theme: Arc::clone(&theme),
                                                set: {
                                                    let view = cx.entity().downgrade();
                                                    Arc::new(move |gain, cx| {
                                                        view.upgrade().unwrap().update(cx, move |view, _| {
                                                            view.audio.update_track_gain(track_index, 10_f32.powf(gain / 20.));
                                                        });
                                                        cx.notify(view.entity_id());
                                                    })
                                                },
                                                scale: 0.01,
                                                name: format!("Track {} gain", track_index + 1).into(),
                                                default: 0.,
                                            }).child("dB"))
                                            // .pipe(deferred)
                                    })
                                    .child(
                                        div()
                                            .absolute()
                                            .inset_0()
                                            .id(track_index)
                                            .bg(theme.hover)
                                            .invisible()
                                            .drag_over(move |style, _: &EntryDragPayload, _, _| style.visible())
                                            .on_drop(cx.listener(move |view, EntryDragPayload(Entry { path, .. }), window, cx| {
                                                let start =
                                                    Time::Beats(view.width_to_beats(window.mouse_position().relative_to(&view.bounds.origin).x - view.pan.x.to_pixels(window.rem_size()), window.rem_size()));
                                                view.audio.update_playlist(|playlist| {
                                                    playlist.add_clips(track_index, Arc::clone(path), start);
                                                });
                                                cx.notify();
                                            }))
                                            .flex()
                                            .justify_center()
                                            .items_center()
                                            .child("Drop to add clips"),
                                    )
                            }))
                            .children(self.time_selection.as_ref().map(|sel| {
                                let (track_range, time_range) = sel.normalized();
                                let track_height = self.zoom.height.to_pixels(window.rem_size());
                                let top_track = *track_range.start();
                                let bottom_track = *track_range.end();

                                let top = px(top_track as f32 * track_height.as_f32());
                                let height = px((bottom_track - top_track + 1) as f32 * track_height.as_f32());

                                let left = self.beats_to_width(Beats::new(time_range.start)) + self.pan.x;
                                let width = self.beats_to_width(Beats::new(time_range.end)) - self.beats_to_width(Beats::new(time_range.start));

                                if time_range.start == time_range.end {
                                    div()
                                        .absolute()
                                        .top(top)
                                        .left(left)
                                        .w(px(1.))
                                        .h(height)
                                        .bg(theme.playhead.tap_mut(|c| c.a = 0.8))
                                } else {
                                    div()
                                        .absolute()
                                        .top(top)
                                        .left(left)
                                        .w(width)
                                        .h(height)
                                        .bg(gpui::hsla(200./360., 0.8, 0.5, 0.3))
                                        .border_1()
                                        .border_color(gpui::hsla(200./360., 0.8, 0.5, 0.8))
                                }
                            }))
                            .children(
                                self.hovered_position
                                    .map(|hovered_position| div().w_px().bg(theme.playhead_hover).absolute().top_0().bottom_0().left(hovered_position.x)),
                            )
                            .child(
                                div()
                                    .w_px()
                                    .bg(theme.playhead)
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .left(playhead_x)
                            )
                            .children(self.audio.playlist().preview.into_iter().flat_map(|preview| {
                                let timing = preview.as_beats(tempo);[timing.start, timing.end].map(|time| {
                                    div()
                                        .w_px()
                                        .bg(theme.preview)
                                        .absolute()
                                        .top_0()
                                        .bottom_0()
                                        .left(rems(time.f32() / self.audio.playlist().time_signature.beats_per_measure as f32 * self.zoom.width.0) + self.pan.x)
                                })
                            })),
                    )
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(14.))
                    .bg(theme.central_background)
                    .border_t_1()
                    .border_color(theme.navbar_outline)
                    .child(
                        div()
                            .id("scrollbar_thumb")
                            .absolute()
                            .left(px(scrollbar_left_frac * view_width_px))
                            .w(px(scrollbar_width_frac * view_width_px))
                            .h_full()
                            .bg(theme.navbar_outline)
                            .hover(|s| s.bg(theme.hover))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, cx.listener(|view, event: &gpui::MouseDownEvent, _, _| {
                                view.last_scrollbar_mouse_pos = Some(event.position);
                            }))
                            .on_mouse_up(MouseButton::Left, cx.listener(|view, _, _, _| {
                                view.last_scrollbar_mouse_pos = None;
                            }))
                            .on_drag(ScrollbarDrag, |_, _, _, cx| cx.new(|_| gpui::Empty))
                            .on_drag_move(cx.listener(|view, event: &gpui::DragMoveEvent<ScrollbarDrag>, window, cx| {
                                view.auto_scroll = false;

                                let Some(last_pos) = view.last_scrollbar_mouse_pos else { return };
                                let current_pos = event.event.position;

                                let dx = current_pos.x.as_f32() - last_pos.x.as_f32();
                                let dy = current_pos.y.as_f32() - last_pos.y.as_f32();
                                view.last_scrollbar_mouse_pos = Some(current_pos);

                                let tempo = view.audio.playlist().tempo;
                                let max_clip_end = view.audio.playlist().tracks().iter()
                                    .flat_map(|t| t.clips().iter().map(|c| c.timing.as_beats(tempo).end.f64()))
                                    .fold(0.0_f64, f64::max);

                                let total_beats = max_clip_end.max(view.audio.playhead().beats(tempo).f64()).max(32.0);
                                let total_width_px = view.beats_to_width(Beats::new(total_beats)).to_pixels(window.rem_size()).as_f32();
                                let view_width_px = view.bounds.size.width.as_f32();

                                let delta_scroll_x_px = dx * (total_width_px / view_width_px);
                                view.target_pan.x = rems(view.target_pan.x.0 - (delta_scroll_x_px / window.rem_size().as_f32()));

                                if dy.abs() > 0.0 {
                                    let old = view.zoom;
                                    let zoom_factor = 1.0 - (dy * 0.01);
                                    view.zoom.width.0 = (view.zoom.width.0 * zoom_factor).max(2.);
                                    let factor = view.zoom.width.0 / old.width.0;

                                    let center_x = view_width_px / 2.0;
                                    let pan_px = -view.target_pan.x.to_pixels(window.rem_size()).as_f32();
                                    let center_abs_px = pan_px + center_x;
                                    let new_center_abs_px = center_abs_px * factor;
                                    let new_pan_px = new_center_abs_px - center_x;

                                    view.target_pan.x = rems(-new_pan_px / window.rem_size().as_f32());
                                }

                                view.target_pan.x.0 = view.target_pan.x.0.min(0.);
                                cx.notify();
                            }))
                    )
            )
    }
}
