use blerp::{read::Track, utils::zip};
use cpal::{
    Host,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use itertools::Itertools;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher};
use open::that_detached;
use std::{
    borrow::Cow,
    collections::HashMap,
    f32::consts::FRAC_PI_2,
    fs::{File, read_dir},
    iter::Iterator,
    ops::BitOr,
    path::{Path, PathBuf},
    rc::Rc,
    str::FromStr,
    string::ToString,
    sync::{Arc, RwLock},
    task::Poll,
    thread::spawn,
    time::{Duration, Instant},
    vec,
};
use strum::Display;
use tap::Pipe;
use tracing::{error, trace};
use unicode_truncate::UnicodeTruncateStr;

use egui::{
    Button, Color32, Context, CursorIcon, DragAndDrop, DroppedFile, FontId, Id, Image, LayerId, Margin, Order, Response, RichText, ScrollArea, Sense, Separator, Shape, Stroke, Ui, UiBuilder, Vec2,
    Widget,
    emath::{self, TSTransform},
    hex_color, include_image, vec2,
};

use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded, unbounded};

use crate::visual::theme::ThemeColors;

// https://veykril.github.io/tlborm/decl-macros/building-blocks/counting.html#bit-twiddling
macro_rules! count_tts {
    () => { 0 };
    ($odd:tt $($a:tt $b:tt)*) => { (count_tts!($($a)*) << 1) | 1 };
    ($($a:tt $even:tt)*) => { count_tts!($($a)*) << 1 };
}

macro_rules! enum_with_array {
    {
        #[derive($($derives:ident),*)]
        pub enum $name:ident
        {
            $($variants:ident),*
            $(,)?
        }
    } => {
        #[derive($($derives),*)]
        pub enum $name {
            $($variants,)*
        }
        impl $name {
            pub const VARIANTS: [$name; count_tts!($($variants)*)] = [$($name::$variants),*];
        }
    };
}

enum_with_array! {
    #[derive(Display, Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Category {
        Files,
        Devices,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    data: Poll<EntryData>,
    depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryData {
    path: Arc<Path>,
    kind: EntryKind,
}

#[derive(Display, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryKind {
    Directory,
    Audio,
    File,
}

pub struct Preview {
    pub path: Option<Arc<Path>>,
    pub path_tx: Sender<Arc<Path>>,
    pub file_data_rx: Receiver<PreviewData>,
    pub file_data: Option<PreviewData>,
}

impl Preview {
    pub fn play_file(&mut self, path: Arc<Path>) {
        self.path = Some(Arc::clone(&path));
        self.path_tx.send(path).unwrap();
        self.file_data = None;
    }

    pub fn data(&mut self) -> Option<PreviewData> {
        self.file_data = match self.file_data_rx.try_recv() {
            Ok(data) => Some(data),
            Err(_) => self.file_data,
        };
        if self.file_data.is_some_and(|data| data.progress() > data.length) {
            self.path = None;
            self.file_data = None;
        }
        self.file_data
    }
}

#[derive(Clone, Copy)]
pub struct PreviewData {
    pub length: Duration,
    pub started_playing: Instant,
}

impl PreviewData {
    fn progress(&self) -> Duration {
        self.started_playing.elapsed()
    }

    fn remaining(&self) -> Duration {
        self.length - self.progress()
    }

    /// Return a value between 0 and 1 representing how much of the audio has played.
    fn percentage(&self) -> f32 {
        self.progress().as_secs_f32() / self.length.as_secs_f32()
    }
}

pub struct Browser {
    selected_category: Category,
    open_paths: Vec<PathBuf>,
    expanded_paths: Vec<Arc<Path>>,
    preview: Preview,
    theme: Rc<ThemeColors>,
    cached_entries: FsWatcherCache<CachedEntries>,
    cached_entry_kinds: Arc<RwLock<FsWatcherCache<EntryKind>>>,
    pub show: bool,
}

struct CachedEntries {
    rx: Receiver<Vec<(EntryKind, Arc<Path>)>>,
    data: Poll<Vec<(EntryKind, Arc<Path>)>>,
}

struct FsWatcherCache<T> {
    data: HashMap<PathBuf, T>,
    watcher: RecommendedWatcher,
    rx: Receiver<notify::Result<Event>>,
}

impl<T> Default for FsWatcherCache<T> {
    fn default() -> Self {
        let (tx, rx) = unbounded();

        Self {
            data: HashMap::new(),
            watcher: recommended_watcher(tx).unwrap(),
            rx,
        }
    }
}

impl Browser {
    const ENTRY_HEIGHT: f32 = 20.;

    // TODO move some of this to blerp
    #[allow(clippy::too_many_lines)]
    pub fn new(theme: Rc<ThemeColors>) -> Self {
        Self {
            selected_category: Category::Files,
            open_paths: vec![PathBuf::from_str("/").unwrap()],
            expanded_paths: Vec::new(),
            preview: {
                let (path_tx, path_rx) = unbounded::<Arc<Path>>();
                let (file_data_tx, file_data_rx) = unbounded();
                spawn(move || {
                    let device = Host::default().default_output_device().unwrap();
                    let (samples_tx, samples_rx) = unbounded::<[f32; 2]>();
                    let config = device.default_output_config().unwrap();
                    let device_sample_rate = config.sample_rate().0;

                    let stream = {
                        let samples_rx = samples_rx.clone();
                        device
                            .build_output_stream(
                                &config.config(),
                                move |data, _| {
                                    for sample in data.chunks_mut(config.channels() as usize) {
                                        sample.copy_from_slice(&match samples_rx.try_recv() {
                                            Ok(sample) => sample,
                                            Err(TryRecvError::Empty) => [0., 0.],
                                            Err(TryRecvError::Disconnected) => {
                                                panic!()
                                            }
                                        });
                                    }
                                },
                                |_| {},
                                None,
                            )
                            .unwrap()
                    };
                    stream.play().unwrap();

                    let mut last_path = None;
                    loop {
                        let Ok(path) = path_rx.recv() else {
                            break;
                        };
                        samples_rx.try_iter().for_each(drop);
                        if last_path.as_ref().is_some_and(|last_path| last_path == &path) {
                            last_path = None;
                            continue;
                        }
                        let tracks = blerp::read(File::open(&path).unwrap()).unwrap();
                        let mut longest = None;
                        for track in tracks {
                            let Track {
                                channels,
                                sample_rate: track_sample_rate,
                            } = track.unwrap();
                            #[allow(clippy::cast_precision_loss, reason = "this is a duration")]
                            let length = Duration::from_secs_f64(channels.iter().map(|channel| channel.samples.len()).max().unwrap_or(0) as f64 / f64::from(track_sample_rate.unwrap_or(44100)));
                            if longest.is_none_or(|longest| length > longest) {
                                longest = Some(length);
                            }
                            let mut channels = channels.into_iter().map(|channel| channel.samples.into_iter()).collect_vec();
                            while let Some(sample) = channels
                                .iter_mut()
                                .map(Iterator::next)
                                .try_fold((0., 0., [None; 2]), |(sum, count, stereo), sample| {
                                    sample.map(|sample| {
                                        (
                                            sum + sample,
                                            count + 1.,
                                            match stereo {
                                                [None, None] => [Some(sample), None],
                                                [Some(left), None] => [Some(left), Some(sample)],
                                                [None, Some(right)] => [Some(sample), Some(right)],
                                                full => full,
                                            },
                                        )
                                    })
                                })
                                .map(|(sum, count, stereo)| {
                                    #[expect(clippy::float_cmp, reason = "count is a small integer represented by a float")]
                                    if let [Some(left), Some(right)] = stereo
                                        && count == 2.
                                    {
                                        [left, right]
                                    } else {
                                        [sum / count; 2]
                                    }
                                })
                            {
                                for _ in 0..track_sample_rate.map_or(1, |track_sample_rate| device_sample_rate / track_sample_rate) {
                                    samples_tx.send(sample).unwrap();
                                }
                            }
                        }
                        if last_path.is_none_or(|last_path| last_path != path) {
                            file_data_tx
                                .send(PreviewData {
                                    length: longest.unwrap_or_default(),
                                    started_playing: Instant::now(),
                                })
                                .unwrap();
                        }
                        last_path = Some(path);
                    }
                });
                Preview {
                    path_tx,
                    file_data_rx,
                    path: None,
                    file_data: None,
                }
            },
            theme,
            cached_entries: FsWatcherCache::default(),
            cached_entry_kinds: Arc::new(RwLock::new(FsWatcherCache::default())),
            show: true,
        }
    }

    fn entry_kind_of(path: impl AsRef<Path>, cached_entry_kinds: &mut FsWatcherCache<EntryKind>) -> EntryKind {
        let path = path.as_ref();
        for event in cached_entry_kinds.rx.try_iter() {
            let event = event.unwrap();
            match event.kind {
                EventKind::Access(_) => {}
                _ => {
                    for path in event.paths.iter().map(|path| if path.is_dir() { path } else { path.parent().unwrap() }) {
                        trace!("invalidating entry kind cache for {:?}", path);
                        cached_entry_kinds.data.remove(path);
                    }
                }
            }
        }

        *cached_entry_kinds.data.entry(path.to_path_buf()).or_insert_with(|| {
            let watch_result = cached_entry_kinds.watcher.watch(path.parent().unwrap_or(path), RecursiveMode::NonRecursive);
            if let Err(error) = watch_result {
                error!("Unexpected error while trying to watch directory: {:?}", error);
            }
            trace!("entry kind cache miss for {:?}", path);
            if path.is_dir() {
                EntryKind::Directory
            } else {
                path.extension().and_then(|ext| ext.to_str()).map_or(EntryKind::File, |extension| {
                    const AUDIO_EXTENSIONS: [&str; 6] = ["flac", "mp3", "ogg", "opus", "wav", "wave"];
                    if AUDIO_EXTENSIONS.into_iter().any(|other| other.eq_ignore_ascii_case(extension)) {
                        EntryKind::Audio
                    } else {
                        EntryKind::File
                    }
                })
            }
        })
    }

    // Animations
    fn loading(ui: &mut Ui) -> Response {
        #[allow(clippy::cast_possible_truncation, reason = "this is a visual effect")]
        let rotated = Image::new(include_image!("../images/icons/loading.png")).rotate(ui.input(|i| i.time * 6.0) as f32, vec2(0.5, 0.5));
        ui.ctx().request_repaint();
        ui.add_sized(vec2(16., 16.), rotated)
    }

    // Widgets
    pub fn button<'a>(theme: &'a ThemeColors, selected: bool, text: &'a str) -> impl Widget + use<'a> {
        move |ui: &mut Ui| {
            ui.allocate_ui(vec2(0., 16.), |ui| {
                ui.visuals_mut().widgets.inactive.fg_stroke.color = theme.browser_unselected_button_fg;
                ui.visuals_mut().widgets.hovered.fg_stroke.color = theme.browser_unselected_hover_button_fg;
                let button = ui
                    .centered_and_justified(|ui| Button::new(RichText::new(text).size(14.).pipe(|text| if selected { text.color(theme.browser_selected_button_fg) } else { text })).ui(ui))
                    .inner;
                ui.visuals_mut().widgets.noninteractive.bg_stroke.color = if selected {
                    theme.browser_selected_button_fg
                } else if button.hovered() {
                    theme.browser_unselected_hover_button_fg
                } else {
                    theme.browser_unselected_button_fg
                };
                ui.add(Separator::default().shrink(10.).spacing(0.));
                button
            })
            .inner
        }
    }

    pub fn collapsing_header_icon(&self, openness: f32) -> impl Widget + use<'_> {
        move |ui: &mut Ui| {
            ui.allocate_painter(Vec2::splat(ui.available_height()), Sense::hover()).pipe(|(response, painter)| {
                let rect = response.rect.shrink(6.);
                let mut points = vec![rect.left_top(), rect.right_top(), rect.center_bottom()];
                let rotation = emath::Rot2::from_angle((openness - 1.) * FRAC_PI_2);
                for p in &mut points {
                    *p = rect.center() + rotation * (*p - rect.center());
                }
                painter.add(Shape::convex_polygon(points, self.theme.browser_folder_text, Stroke::NONE));
                response
            })
        }
    }

    fn add_files(&mut self, ui: &mut Ui, scroll_area: ScrollArea, browser_width: f32) -> Response {
        self.handle_file_or_folder_drop(ui.ctx());
        let entries = self.open_paths.iter().fold(Vec::new(), |mut entries, path| {
            Self::entries(&mut entries, path, 0, &mut self.cached_entries, &self.cached_entry_kinds, &self.expanded_paths);
            entries
        });
        scroll_area
            .show_rows(ui, Self::ENTRY_HEIGHT, entries.len(), |ui, row_range| {
                egui::Frame::default()
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.visuals_mut().widgets.noninteractive.fg_stroke.color = self.theme.browser_folder_text;
                            ui.visuals_mut().widgets.hovered.fg_stroke.color = self.theme.browser_folder_hover_text;
                            ui.style_mut().spacing.item_spacing.x = 4.;
                            let entries_iter = entries.into_iter();
                            for entry in entries_iter.skip(row_range.start).take(row_range.len() + 8) {
                                self.add_entry(entry, ui, browser_width);
                            }
                        })
                    })
                    .response
            })
            .inner
    }

    fn list_cached<'a>(path: &Path, cached_entries: &'a mut FsWatcherCache<CachedEntries>, cached_entry_kinds: &Arc<RwLock<FsWatcherCache<EntryKind>>>) -> &'a mut CachedEntries {
        for event in cached_entries.rx.try_iter() {
            let event = event.unwrap();
            match event.kind {
                EventKind::Access(_) => {}
                _ => {
                    for path in event.paths.iter().map(|path| if path.is_dir() { path } else { path.parent().unwrap() }) {
                        trace!("invalidating cached entries cache for {:?}", path);
                        cached_entries.data.remove(path);
                    }
                }
            }
        }

        cached_entries.data.entry(path.to_path_buf()).or_insert_with(|| {
            trace!("list cache miss for {:?}", path);
            let watch_result = cached_entries.watcher.watch(path.parent().unwrap_or(path), RecursiveMode::NonRecursive);
            if let Err(error) = watch_result {
                error!("Unexpected error while trying to watch directory: {:?}", error);
            }
            let (tx, rx) = bounded(1);
            let Ok(read_dir) = read_dir(path) else {
                error!("Failed to read directory: {:?}", path);
                return CachedEntries { data: Poll::Ready(Vec::new()), rx };
            };
            let cached_entry_kinds = Arc::clone(cached_entry_kinds);
            spawn(move || {
                let read_dir = read_dir
                    .map(|entry| {
                        let path = entry.unwrap().path();
                        (Self::entry_kind_of(&path, &mut cached_entry_kinds.write().unwrap()), Arc::from(path.as_path()))
                    })
                    .sorted_unstable()
                    .collect_vec();
                tx.send(read_dir).unwrap();
            });

            CachedEntries { data: Poll::Pending, rx }
        })
    }

    fn entries(
        entries: &mut Vec<Entry>,
        path: &Path,
        mut depth: usize,
        cached_entries: &mut FsWatcherCache<CachedEntries>,
        cached_entry_kinds: &Arc<RwLock<FsWatcherCache<EntryKind>>>,
        expanded_paths: &[Arc<Path>],
    ) {
        if depth == 0 {
            entries.push(Entry {
                data: Poll::Ready(EntryData {
                    path: Arc::from(path),
                    kind: Self::entry_kind_of(path, &mut cached_entry_kinds.write().unwrap()),
                }),
                depth,
            });
        }
        if !expanded_paths.iter().any(|expanded| **expanded == *path) {
            return;
        }
        depth += 1;
        let CachedEntries { data, rx } = Self::list_cached(path, cached_entries, cached_entry_kinds);
        match data {
            Poll::Ready(list) => {
                for (kind, entry) in list.clone() {
                    entries.push(Entry {
                        data: Poll::Ready(EntryData { path: Arc::from(Path::new("")), kind }),
                        depth,
                    });
                    let len = entries.len();
                    if expanded_paths.iter().any(|expanded| **expanded == *entry) {
                        Self::entries(entries, &entry, depth, cached_entries, cached_entry_kinds, expanded_paths);
                    }
                    match &mut entries[len - 1].data {
                        Poll::Ready(EntryData { path, .. }) => *path = entry,
                        Poll::Pending => unreachable!(),
                    }
                }
            }
            Poll::Pending => match rx.try_recv() {
                Ok(list) => {
                    *data = Poll::Ready(list);
                }
                Err(TryRecvError::Disconnected) => {
                    *data = Poll::Ready(Vec::new());
                }
                Err(TryRecvError::Empty) => {
                    entries.push(Entry { data: Poll::Pending, depth });
                }
            },
        }
    }

    fn add_entry(&mut self, Entry { data, depth }: Entry, ui: &mut Ui, browser_width: f32) -> Response {
        const INDENT_SIZE: f32 = 16.;
        let Poll::Ready(EntryData { path, kind }) = data else {
            return ui
                .horizontal(|ui| {
                    #[allow(clippy::cast_possible_truncation, reason = "this is a visual effect")]
                    #[allow(clippy::cast_precision_loss, reason = "this is a visual effect")]
                    ui.add_space(INDENT_SIZE * depth as f32);
                    ui.add(Self::loading);
                })
                .response;
        };
        let next_top = ui.next_widget_position().y;
        let next_bottom = next_top + Self::ENTRY_HEIGHT;
        if next_top >= ui.clip_rect().bottom() || next_bottom <= ui.clip_rect().top() && kind != EntryKind::Directory {
            return ui.allocate_response(vec2(0.0, Self::ENTRY_HEIGHT), Sense::hover());
        }
        let name = path.file_name().map_or_else(|| path.to_string_lossy(), |name| name.to_string_lossy());
        let full_width = ui.painter().layout(name.to_string(), FontId::proportional(12.), Color32::WHITE, f32::INFINITY).size().x;
        let char_length = name.to_string().len();
        let mut final_char_length = char_length;
        let mut final_text = name.to_string();
        #[allow(clippy::cast_precision_loss, reason = "this is a visual effect")]
        let available_width = INDENT_SIZE.mul_add(-(depth as f32), browser_width - 30.);
        if full_width > available_width {
            for i in char_length..0 {
                let string = name.to_string();
                let text = string.unicode_truncate(i).0;
                let width = ui.painter().layout(text.to_string(), FontId::proportional(12.), Color32::WHITE, f32::INFINITY).size().x;
                if width <= available_width {
                    final_char_length = i;
                    break;
                }
            }
            // final_text = format!("{}{}", final_text.unicode_truncate(usize::max(final_char_length, 3)-3).0, "...");
            final_text = final_text.unicode_truncate(final_char_length).0.to_string();
        }
        let button = |theme: &ThemeColors| -> Button<'static> {
            Button::new(RichText::new(final_text.clone()).font(FontId::proportional(12.)).pipe(|text| {
                if matches!(&name, &Cow::Owned(_)) {
                    text.color(theme.browser_unselected_button_fg_invalid)
                } else {
                    text
                }
            }))
        };
        let response = ui
            .allocate_ui(vec2(ui.available_width(), Self::ENTRY_HEIGHT), |ui| {
                ui.horizontal(|ui| {
                    #[allow(clippy::cast_possible_truncation, reason = "this is a visual effect")]
                    #[allow(clippy::cast_precision_loss, reason = "this is a visual effect")]
                    ui.add_space(INDENT_SIZE * depth as f32);
                    egui::Frame::new()
                        .show(ui, |ui| match kind {
                            EntryKind::Audio => self.add_audio_entry(&path, ui, &Rc::clone(&self.theme), button),
                            EntryKind::File => Self::add_file(ui, button(&self.theme)),
                            EntryKind::Directory => {
                                ui.horizontal(|ui| ui.add(self.collapsing_header_icon(f32::from(self.expanded_paths.contains(&path)))) | ui.add(button(&self.theme)))
                                    .inner
                            }
                        })
                        .inner
                        | ui.allocate_response(ui.available_size(), Sense::click())
                })
                .inner
            })
            .inner;
        if response.clicked() {
            match kind {
                EntryKind::Audio => {
                    self.preview.play_file(Arc::clone(&path));
                }
                EntryKind::File => {
                    that_detached(path.as_os_str()).unwrap();
                }
                EntryKind::Directory => {
                    if let Some(index) = self.expanded_paths.iter().position(|expanded| expanded == &path) {
                        self.expanded_paths.swap_remove(index);
                    } else {
                        self.expanded_paths.push(path);
                    }
                }
            }
        }
        if response.hovered() {
            ui.painter().rect_filled(response.rect, 2.0, self.theme.browser_unselected_hover_button_fg.linear_multiply(0.2));
            ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
        }
        response
    }

    fn add_audio_entry(&mut self, path: &Path, ui: &mut Ui, theme: &Rc<ThemeColors>, button: impl Fn(&ThemeColors) -> Button<'static>) -> Response {
        let mut add_contents = |ui: &mut Ui| {
            ui.horizontal(|ui| {
                ui.add(Image::new(include_image!("../images/icons/audio.png"))).union(ui.add(button(theme))).pipe(|response| {
                    let data = self.preview.data();
                    if let Some(data @ PreviewData { length, .. }) = self.preview.path.as_ref().filter(|preview_path| ***preview_path == *path).zip(data).map(|(_, data)| data) {
                        ui.ctx().request_repaint();
                        response
                            | ui.label(format!(
                                "{:>02}:{:>02} of {:>02}:{:>02}",
                                data.progress().as_secs() / 60,
                                data.progress().as_secs() % 60,
                                length.as_secs() / 60,
                                length.as_secs() % 60
                            ))
                    } else {
                        response
                    }
                })
            })
        };
        let mut response = if ui.ctx().is_being_dragged(Id::new(path.to_owned())) {
            DragAndDrop::set_payload(ui.ctx(), path.to_path_buf());
            let layer_id = LayerId::new(Order::Tooltip, Id::new(path.to_owned()));
            let response = ui.scope_builder(UiBuilder::new().layer_id(layer_id), add_contents).response;
            if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                let delta = pointer_pos - response.rect.center();
                ui.ctx().transform_layer_shapes(layer_id, TSTransform::from_translation(delta));
            }
            response
        } else {
            let response = ui.scope(&mut add_contents).response;
            let dnd_response = ui.interact(response.rect, Id::new(path.to_owned()), Sense::click_and_drag()).on_hover_cursor(CursorIcon::Grab);
            dnd_response | response
        };
        if let Some(data) = self.preview.data()
            && self.preview.path.as_ref().is_some_and(|previewing| **previewing == *path)
        {
            ui.ctx().request_repaint();
            ui.painter().rect_filled(
                response.rect.with_max_x(response.rect.width().mul_add(data.percentage(), response.rect.left())),
                0,
                hex_color!("#ffffff20"),
            );
        }
        response.layer_id = ui.layer_id();
        response
    }

    fn handle_file_or_folder_drop(&mut self, ctx: &Context) {
        ctx.input(|input| {
            for path in input.raw.dropped_files.iter().filter_map(|DroppedFile { path, .. }| path.as_deref()) {
                self.open_paths.push(path.to_path_buf());
            }
        });
    }

    fn add_file(ui: &mut Ui, button: Button<'_>) -> Response {
        ui.horizontal(|ui| ui.add(Image::new(include_image!("../images/icons/file.png"))) | (ui.add(button))).inner
    }
}

impl Widget for &mut Browser {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.add_space(6.);
        let browser_width = ui.available_width();
        ui.vertical(|ui| {
            ui.visuals_mut().button_frame = false;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 16.;
                ui.columns_const(|uis| {
                    zip(Category::VARIANTS, uis.each_mut())
                        .map(|(category, ui)| {
                            let selected = self.selected_category == category;
                            let string = category.to_string();
                            let response = ui.add(Browser::button(&self.theme, selected, &string));
                            if response.clicked() {
                                self.selected_category = category;
                            }
                            response
                        })
                        .into_iter()
                        .reduce(Response::bitor)
                        .unwrap()
                })
            });
            ui.add_space(4.);
            ui.visuals_mut().extreme_bg_color = Color32::from_hex("#7676a340").unwrap();
            // ui.style_mut().spacing.scroll.floating = false;
            let scroll_area = ScrollArea::both()
                .scroll_source(egui::scroll_area::ScrollSource {
                    scroll_bar: true,
                    drag: false,
                    mouse_wheel: true,
                })
                .auto_shrink(false)
                // .hscroll(false)
                .max_width(ui.available_width() - 6.)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded);
            egui::Frame::default()
                .show(ui, |ui| {
                    match self.selected_category {
                        Category::Files => self.add_files(ui, scroll_area, browser_width),
                        Category::Devices => {
                            // TODO: Show some devices here!
                            ui.label("Devices")
                        }
                    }
                })
                .response
        })
        .inner
    }
}
