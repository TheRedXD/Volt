use std::{array::from_fn, collections::{HashMap, HashSet}, range::Range, sync::Arc};

use blerp::{Clip, ClipTiming, ClipTimingBeats, Tempo, Track};
use gpui::{
    AbsoluteLength, AppContext, Bounds, Context, DefiniteLength, Div, FontWeight, InteractiveElement, IntoElement, Length, MouseButton, ParentElement, PathBuilder, Pixels, Point, Rems, Render, Size, StatefulInteractiveElement, Styled, Window, canvas, deferred, div, hsla, img, pattern_slash, point, px, rems, rgb, rgba, size
};
use tap::{Conv, Tap};

use crate::{AUDIO_TRACK_ICON, theme::ThemeColors, components::svg_icon::SvgIcon, views::{playlist::{MIPMAP_HIGH, PlaylistView}}};

#[derive(Clone)]
pub struct ClipDrag;

#[derive(Clone)]
pub struct ClipResizeLeft {
    pub clip_id: usize,
    pub initial_timing: ClipTiming,
}

#[derive(Clone)]
pub struct ClipResizeRight {
    pub clip_id: usize,
    pub initial_timing: ClipTiming,
}

pub fn clip(beats: ClipTimingBeats, start: Rems, length: Rems, tempo_length: Rems, selected_clips: HashSet<usize>, culling_view_width_rems: f32, clip: &Clip, track: &Track, tempo: Tempo, track_index: usize, theme: Arc<ThemeColors>, window: &mut Window, cx: &mut Context<'_, PlaylistView>) -> Option<Div> {
    if start.0 + length.0 < 0.0 || start.0 > culling_view_width_rems {
        return None;
    }
    
    let is_selected = selected_clips.contains(&clip.id);
    
    Some(div()
        .absolute()
        .left(start)
        .top_0()
        .h_full()
        .w(length)
        .bg(if is_selected { pattern_slash(hsla(0., 0., 0.2, 0.5), 2., 5.) } else { pattern_slash(hsla(0., 0., 0.2, 1.), 2., 5.) })
        .overflow_hidden()
        .rounded_md()
        .border_1()
        .border_color(if is_selected { rgb(0xffffff) } else { rgb(track.color) })
        .child(
            div()
                .id(format!("inner track clip thing {} {}", track_index, clip.id))
                .h(px(16.))
                .w_full()
                .bg(if is_selected { rgb(0xffffff) } else { rgb(track.color) })
                .text_color(rgba(0x000000c0))
                .text_xs()
                .px_1()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .child(div().flex().flex_shrink_0().child(SvgIcon::new(AUDIO_TRACK_ICON, 16, 16)).mr(gpui::Pixels::from(2.)).opacity(0.6))
                        .child(clip.name.clone())
                )
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
                .w(tempo_length)
                .bg(if is_selected {let mut color = rgb(track.color); color.a = 0.4; color} else {let mut color = rgb(track.color); color.a = 0.2; color})
                .child(
                    canvas(|_, _, _| {}, {
                        let mut clip = clip.clone();
                        let width = tempo_length.to_pixels(window.rem_size());
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
                                    window.paint_path(path, rgb(0xffffff));
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
}