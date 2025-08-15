use std::ops::BitOr;
use std::path::PathBuf;
use std::{collections::HashMap, num::NonZeroU64};

use blerp::processing::effects::clip::ClipEffect;
use blerp::processing::effects::scale::ScaleEffect;
use eframe::egui;
use egui::scroll_area::ScrollSource;
use egui::{
    Align, Align2, Color32, CursorIcon, Frame, Id, InputState, Layout, Rect, Response, ScrollArea, Sense, Stroke, Ui, UiBuilder, Vec2, Widget, hex_color, pos2, scroll_area::ScrollBarVisibility, vec2,
};
use graph::{Graph, Node, NodeData, NodeId};
use itertools::Itertools;
use playlist::{Clip, ClipData, Playlist, Time};

use super::ThemeColors;

mod graph {
    use blerp::processing::effects::Effect;
    use egui::Vec2;
    use std::collections::HashMap;
    use std::fmt::Debug;
    use std::num::NonZeroU64;

    #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
    pub enum NodeId {
        Output,
        Middle(NonZeroU64),
    }

    pub struct Graph {
        pub nodes: HashMap<NodeId, Node>,
        pub pan_offset: Vec2,
        pub drag_start_offset: Option<Vec2>,
    }

    pub struct Node {
        pub position: Vec2,
        pub data: NodeData,
        pub drag_start_offset: Option<Vec2>,
    }

    pub enum NodeData {
        Output,
        Middle { effect: Box<dyn Effect>, output: Option<NodeId> },
    }
}

mod playlist {
    use cpal::Sample;
    use egui::{Vec2, vec2};
    use itertools::Itertools;
    use std::{fs::File, io::BufReader, path::PathBuf, time::Duration};

    #[derive(Debug)]
    pub struct Playlist {
        pub clips: Vec<Clip>,
        pub time_signature: TimeSignature,
        pub tempo: Tempo,
        pub time: Time,
        /// The zoom factor for the playlist view. `[400.0 60.0]` means a measure is 400 pixels wide and a track is 60 pixels tall.
        pub zoom: Vec2,
        pub snapping: Snapping,
    }

    impl Default for Playlist {
        fn default() -> Self {
            Self {
                clips: Vec::new(),
                time_signature: TimeSignature::default(),
                tempo: Tempo::default(),
                time: Time::default(),
                zoom: vec2(400., 60.),
                snapping: Snapping::default(),
            }
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

    #[derive(Debug, Clone, Copy)]
    pub struct Tempo {
        beats_per_hectominute: u32,
    }

    impl Default for Tempo {
        fn default() -> Self {
            Self::from_bpm(120.)
        }
    }

    impl Tempo {
        pub fn from_bpm(bpm: f64) -> Self {
            #[allow(clippy::cast_sign_loss, reason = "bpm is always positive")]
            #[allow(clippy::cast_possible_truncation, reason = "bpm only goes up to 999.99, so never truncates")]
            let beats_per_hectominute = (bpm as u32 * 100).clamp(1, 99999);
            Self { beats_per_hectominute }
        }

        pub fn bpm(self) -> f64 {
            f64::from(self.beats_per_hectominute) / 100.
        }

        pub fn bps(self) -> f64 {
            self.bpm() / 60.
        }
    }

    #[derive(Debug, Clone)]
    pub struct Clip {
        pub start: Time,
        pub track: u32,
        pub data: ClipData,
    }

    #[derive(Debug, Clone)]
    pub enum ClipData {
        Audio { path: PathBuf, samples: Vec<f64>, length: Duration },
        Midi { length: Time },
    }

    impl ClipData {
        pub fn from_path(path: PathBuf) -> Self {
            todo!();
            // let length = decoder.total_duration().unwrap();
            // let samples = decoder.map(f64::from_sample).collect_vec();
            // Self::Audio { path, samples, length }
        }
    }

    #[derive(Debug, Clone, Copy, Default)]
    pub struct Time {
        beats: f64,
    }

    impl Time {
        pub fn from_beats(beats: f64) -> Option<Self> {
            (beats > 0.).then_some(Self { beats })
        }

        pub const fn beats(self) -> f64 {
            self.beats
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct TimeSignature {
        pub beats_per_measure: u32,
        pub beat_unit: u32,
    }

    impl Default for TimeSignature {
        fn default() -> Self {
            Self { beats_per_measure: 4, beat_unit: 4 }
        }
    }

    impl Playlist {
        pub fn now(&self) -> Duration {
            Duration::from_secs_f64(self.time.beats / self.tempo.bpm() * 60.)
        }

        pub const fn measure(&self) -> u32 {
            #[allow(clippy::cast_possible_truncation, reason = "truncation is intentional")]
            #[allow(clippy::cast_sign_loss, reason = "beats cannot be negative")]
            {
                self.time.beats as u32 / self.time_signature.beats_per_measure
            }
        }

        pub fn beats_to_duration(&self, beats: f64) -> Duration {
            Duration::from_secs_f64(beats / self.tempo.bps())
        }

        pub fn duration_of_clip(&self, clip: &ClipData) -> Duration {
            match clip {
                ClipData::Audio { length, .. } => *length,
                ClipData::Midi { length } => self.beats_to_duration(length.beats()),
            }
        }
    }
}

enum Mode {
    Playlist,
    Graph,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Playlist
    }
}

pub struct Central {
    mode: Mode,
    playlist: Playlist,
    graph: Graph,
}

impl Default for Central {
    fn default() -> Self {
        Self::new()
    }
}

impl Central {
    pub fn new() -> Self {
        Self {
            mode: Mode::Playlist,
            playlist: Playlist::default(),

            graph: Graph {
                drag_start_offset: Some(vec2(0., 0.)),
                pan_offset: vec2(0., 0.),
                nodes: [
                    (
                        NodeId::Middle(NonZeroU64::new(1).unwrap()),
                        Node {
                            data: NodeData::Middle {
                                effect: Box::new(ClipEffect::new_symmetrical(0.5)),
                                output: Some(NodeId::Middle(NonZeroU64::new(2).unwrap())),
                            },
                            position: vec2(-200., -20.),
                            drag_start_offset: None,
                        },
                    ),
                    (
                        NodeId::Middle(NonZeroU64::new(2).unwrap()),
                        Node {
                            data: NodeData::Middle {
                                effect: Box::new(ScaleEffect::new(2.)),
                                output: Some(NodeId::Output),
                            },
                            position: vec2(-30., 80.),
                            drag_start_offset: None,
                        },
                    ),
                    (
                        NodeId::Output,
                        Node {
                            data: NodeData::Output,
                            position: vec2(150., 10.),
                            drag_start_offset: None,
                        },
                    ),
                ]
                .into(),
            },
        }
    }

    fn add_playlist(ui: &mut Ui, playlist: &mut Playlist) -> Response {
        playlist.zoom = playlist.zoom * ui.input(InputState::zoom_delta_2d);
        playlist.zoom += ui.input(|input| input.modifiers.alt.then_some(input.smooth_scroll_delta)).unwrap_or_default();
        playlist.zoom = playlist.zoom.max(vec2(50., 50.));
        ScrollArea::both()
            .auto_shrink(false)
            .scroll_source(ScrollSource {
                scroll_bar: true,
                drag: false,
                mouse_wheel: ui.input(|input| input.modifiers.alt),
            })
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                let response = ui
                    .with_layout(Layout::top_down(Align::Min), |ui| {
                        (0..=playlist.clips.iter().map(|clip| clip.track + 1).max().unwrap_or_default())
                            .rev()
                            .map(|y| {
                                Frame::default()
                                    .fill(ThemeColors::default().central_background)
                                    .show(ui, |ui| {
                                        let (response, painter) = ui.allocate_painter(vec2(f32::INFINITY, playlist.zoom.y), Sense::hover());
                                        if let Some(path) = response.dnd_release_payload::<PathBuf>()
                                            && let Some(start) = Time::from_beats(
                                                f64::from((ui.input(|input| input.pointer.latest_pos().unwrap().x) - response.rect.min.x) / playlist.zoom.x)
                                                    * f64::from(playlist.time_signature.beats_per_measure),
                                            )
                                        {
                                            playlist.clips.push(Clip {
                                                start,
                                                track: y,
                                                data: ClipData::from_path((*path).clone()),
                                            });
                                        }
                                        #[allow(clippy::cast_precision_loss, reason = "rounding errors are negligible because this is a visual effect")]
                                        #[allow(clippy::cast_possible_truncation, reason = "truncation only occurs at unreasonably high numbers")]
                                        for Clip { start, track, data } in &playlist.clips {
                                            if track != &y {
                                                continue;
                                            }
                                            let left = (start.beats() as f32 / playlist.time_signature.beats_per_measure as f32).mul_add(playlist.zoom.x, response.rect.min.x);
                                            let width =
                                                playlist.duration_of_clip(data).as_secs_f32() * playlist.tempo.bps() as f32 / playlist.time_signature.beats_per_measure as f32 * playlist.zoom.x;
                                            let rect = Rect::from_min_size(pos2(left, painter.clip_rect().top()), vec2(width, painter.clip_rect().height()));
                                            painter.rect(rect, 4., Color32::GRAY, Stroke::new(2., Color32::DARK_GRAY), egui::StrokeKind::Middle);
                                            painter.debug_text(
                                                rect.left_top(),
                                                Align2::LEFT_TOP,
                                                Color32::BLUE,
                                                match data {
                                                    ClipData::Audio { path, .. } => path.file_name().unwrap().to_string_lossy(),
                                                    ClipData::Midi { .. } => "<midi data>".into(),
                                                },
                                            );
                                        }
                                    })
                                    .response
                            })
                            .reduce(Response::bitor)
                            .unwrap()
                    })
                    .response;
                #[allow(clippy::cast_possible_truncation, reason = "truncation is intentional")]
                #[allow(clippy::cast_precision_loss, reason = "rounding errors are negligible because this is a visual effect")]
                for index in ((ui.clip_rect().left() - response.rect.min.x) / playlist.zoom.x) as i32..((ui.clip_rect().right() - response.rect.min.x) / playlist.zoom.x).ceil() as i32 {
                    let x = (index as f32).mul_add(playlist.zoom.x, response.rect.min.x);
                    ui.painter().vline(x, ui.clip_rect().y_range(), Stroke::new(1., hex_color!("5e5a75")));
                    for sub_index in 1..playlist.time_signature.beats_per_measure {
                        let x = (sub_index as f32).mul_add(playlist.zoom.x / playlist.time_signature.beats_per_measure as f32, x);
                        ui.painter().vline(x, ui.clip_rect().y_range(), Stroke::new(1., hex_color!("2e2b3f")));
                    }
                }
                response
            })
            .inner
    }

    fn add_graph(ui: &mut Ui, Graph { nodes, pan_offset, drag_start_offset }: &mut Graph) -> Response {
        let (_, rect) = ui.allocate_space(ui.available_size());
        let painter = ui.painter_at(rect);
        Frame::default()
            .show(ui, |ui| {
                let responses: HashMap<_, _> = nodes
                    .iter()
                    .map(|(id, node)| {
                        let response = ui
                            .scope_builder(UiBuilder::new().max_rect(Rect::from_min_size(rect.center() + node.position + *pan_offset, Vec2::INFINITY)), |ui| {
                                Frame::default()
                                    .corner_radius(4)
                                    .inner_margin(4.)
                                    .stroke(Stroke::new(1., hex_color!("80808080")))
                                    .show(ui, |ui| {
                                        ui.label("Effect");
                                        ui.label(match &node.data {
                                            NodeData::Output => "Output".to_string(),
                                            NodeData::Middle { effect, output } => format!("{effect} to {output:?}"),
                                        });
                                    })
                                    .response
                            })
                            .inner;
                        (*id, response)
                    })
                    .collect();
                let is_being_dragged = ui.ctx().is_being_dragged(Id::new("graph background"));
                if is_being_dragged {
                    let pos = ui.ctx().pointer_interact_pos().unwrap();
                    if let Some(drag_start_offset) = drag_start_offset {
                        *pan_offset = pos - rect.center() - *drag_start_offset;
                    } else {
                        *drag_start_offset = Some(pos - rect.center() - *pan_offset);
                    }
                } else {
                    ui.interact(rect, Id::new("graph background"), Sense::click_and_drag()).on_hover_and_drag_cursor(CursorIcon::Grab);
                    *drag_start_offset = None;
                }
                for (id, node) in nodes.iter_mut() {
                    let is_being_dragged = ui.ctx().is_being_dragged(Id::new(id));
                    if is_being_dragged {
                        let pos = ui.ctx().pointer_interact_pos().unwrap();
                        if let Some(drag_start_offset) = node.drag_start_offset {
                            node.position = pos - rect.center() - drag_start_offset;
                        } else {
                            node.drag_start_offset = Some(pos - rect.center() - node.position);
                        }
                    } else {
                        ui.interact(responses.get(id).unwrap().rect, Id::new(id), Sense::click_and_drag())
                            .on_hover_and_drag_cursor(CursorIcon::Move);
                        node.drag_start_offset = None;
                    }
                }
                for (a, b) in nodes.iter().filter_map(move |(id, node)| {
                    if let NodeData::Middle { output: Some(output), .. } = &node.data {
                        Some((responses.get(id).unwrap().rect, responses.get(output).unwrap().rect))
                    } else {
                        None
                    }
                }) {
                    const RESOLUTION: usize = 20;
                    let a = a.right_center();
                    let b = b.left_center();
                    let strength = 100_f32.min(a.distance(b) / 2.);

                    for (a, b) in (0..=RESOLUTION)
                        .map(|t| {
                            #[allow(clippy::cast_precision_loss, reason = "rounding errors are negligible because this is a visual effect")]
                            let t = t as f32 / RESOLUTION as f32;

                            (1. - t).powi(3) * a
                                + (3. * (1. - t).powi(2) * t * (a + vec2(strength, 0.))).to_vec2()
                                + (3. * (1. - t) * t.powi(2) * (b - vec2(strength, 0.))).to_vec2()
                                + (t.powi(3) * b).to_vec2()
                        })
                        .tuple_windows()
                    {
                        #[allow(clippy::tuple_array_conversions, reason = "this looks fine")]
                        painter.line_segment([a, b], Stroke::new(2., hex_color!("#80808080")));
                    }
                }
            })
            .response
    }
}

impl Widget for &mut Central {
    fn ui(self, ui: &mut Ui) -> Response {
        Frame::default()
            .show(ui, |ui| match &mut self.mode {
                Mode::Playlist => Central::add_playlist(ui, &mut self.playlist),
                Mode::Graph => Central::add_graph(ui, &mut self.graph),
            })
            .response
    }
}
