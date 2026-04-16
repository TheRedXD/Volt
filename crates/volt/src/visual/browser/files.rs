use std::{collections::HashMap, path::{Path, PathBuf}, rc::Rc, sync::{Arc, RwLock, atomic::AtomicBool, mpsc::{self, Sender, channel}}, task::Poll, thread::{self, Thread}};

use crossbeam_channel::unbounded;
use egui::{Color32, Frame, Image, Label, Margin, Response, ScrollArea, TextureOptions, Ui, Vec2, Widget};
use notify::{Event, RecommendedWatcher, recommended_watcher};
use strum::Display;

use crate::visual::{browser::categories::Category, theme::ThemeColors};

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

struct CachedEntries {
    rx: crossbeam_channel::Receiver<Vec<(EntryKind, Arc<Path>)>>,
    data: Poll<Vec<(EntryKind, Arc<Path>)>>,
}

struct FsWatcherCache<T> {
    data: HashMap<PathBuf, T>,
    watcher: RecommendedWatcher,
    rx: crossbeam_channel::Receiver<notify::Result<Event>>,
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

enum BrowserFilesEntryToPullThreadMessages {
    PullEntries
}
enum BrowserFilesEntryFromPullThreadMessages {
    
}

struct EntryPullThreadStates {
    pulling_entries: Arc<AtomicBool>
}

pub struct BrowserFiles {
    theme: Rc<ThemeColors>,
    category: Arc<RwLock<Category>>,
    
    entry_pull_thread_handles: (Sender<BrowserFilesEntryToPullThreadMessages>, mpsc::Receiver<BrowserFilesEntryFromPullThreadMessages>),
    entry_pull_thread_states: EntryPullThreadStates,
    
    open_paths: Vec<PathBuf>,
    expanded_paths: Vec<Arc<Path>>,
    
    cached_entries: Arc<RwLock<FsWatcherCache<CachedEntries>>>,
    cached_entry_kinds: Arc<RwLock<FsWatcherCache<EntryKind>>>,
}

impl BrowserFiles {
    pub fn new(theme: Rc<ThemeColors>, category: Arc<RwLock<Category>>) -> BrowserFiles {
        let pulling_entries = Arc::new(AtomicBool::new(false));
        BrowserFiles {
            theme: theme,
            category: category,
            
            entry_pull_thread_handles: BrowserFiles::make_entry_pull_thread_handles(Arc::clone(&pulling_entries)),
            
            entry_pull_thread_states: EntryPullThreadStates {
                pulling_entries: Arc::clone(&pulling_entries),
            },
            
            open_paths: {
                #[cfg(target_os = "windows")]
                {
                    use std::fs::exists;
                    (b'A'..=b'Z')
                        .filter_map(|letter| {
                            format!(r"{}:\", letter as char)
                                .pipe(PathBuf::from)
                                .pipe(Some)
                                .filter(|drive| matches!(exists(drive), Ok(true)))
                        })
                        .collect()
                }
                #[cfg(not(target_os = "windows"))]
                {
                    use std::str::FromStr;
                    vec![PathBuf::from_str("/").unwrap()]
                }
            },
            expanded_paths: Vec::new(),
            
            cached_entries: Arc::new(RwLock::new(FsWatcherCache::default())),
            cached_entry_kinds: Arc::new(RwLock::new(FsWatcherCache::default())),
        }
    }
    
    /// Create a separate thread designated specifically only for pulling browser entries from the file system.
    /// We use mpsc channels in order to communicate with the thread.
    /// 
    /// The reason we have a separate thread is such that the GUI does not get locked up
    /// while doing FS operations in order to make the app feel smooth.
    fn make_entry_pull_thread_handles(pulling_entries: Arc<AtomicBool>) -> (Sender<BrowserFilesEntryToPullThreadMessages>, mpsc::Receiver<BrowserFilesEntryFromPullThreadMessages>) {
        let (to_thread_tx, to_thread_rx) = channel();
        let (from_thread_tx, from_thread_rx) = channel();
        
        thread::spawn(move || {
            while let Ok(msg) = to_thread_rx.recv() {
                match msg {
                    BrowserFilesEntryToPullThreadMessages::PullEntries => {
                        if !pulling_entries.load(std::sync::atomic::Ordering::SeqCst) {
                            pulling_entries.store(true, std::sync::atomic::Ordering::SeqCst);
                            
                            // Update entries
                        }
                    }
                }
            }
        });
        
        (to_thread_tx, from_thread_rx)
    }

    pub fn draw(&mut self, ui: &mut Ui) {
        let scroll_area = ScrollArea::both()
            .scroll_source(egui::scroll_area::ScrollSource {
                scroll_bar: true,
                drag: false,
                mouse_wheel: true,
            })
            .auto_shrink([false, false])
            .max_width(ui.available_width() - 6.)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded);
        let text_style = egui::TextStyle::Body;
        let row_height = ui.text_style_height(&text_style);
        let scroll_output = scroll_area.show_rows(ui, row_height, 100, |ui, row_range| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            // Browser items
            // for i in row_range {
            //     let label_text = format!("amogus impostor {}", i);
            //     ui.add(Label::new(label_text).wrap_mode(egui::TextWrapMode::Extend));
            // }
            
            self.entry_pull_thread_handles.0.send(BrowserFilesEntryToPullThreadMessages::PullEntries).unwrap();
            
            // Spacing and gradients
            ui.add_space(5.);
            let top_rect = egui::Rect::from_min_size(
                ui.clip_rect().min,
                egui::vec2(ui.clip_rect().width(), 10.0)
            );
            Image::from_texture(&ui.ctx().load_texture(
                "browser_list_top_texture",
                super::super::build_gradient(10, self.theme.browser, Color32::from_rgba_premultiplied(self.theme.browser.r(), self.theme.browser.g(), self.theme.browser.b(), 0)),
                TextureOptions::default(),
            ))
            .paint_at(ui, top_rect);
            let bottom_rect = egui::Rect::from_min_size(
                egui::pos2(ui.clip_rect().min.x, ui.clip_rect().max.y - 10.0),
                egui::vec2(ui.clip_rect().width(), 10.0)
            );
            Image::from_texture(&ui.ctx().load_texture(
                "browser_list_bottom_texture",
                super::super::build_gradient(10, Color32::from_rgba_premultiplied(self.theme.browser.r(), self.theme.browser.g(), self.theme.browser.b(), 0), self.theme.browser),
                TextureOptions::default(),
            ))
            .paint_at(ui, bottom_rect);
        });
    }
}

impl Widget for &mut BrowserFiles {
    fn ui(self, ui: &mut Ui) -> Response {
        Frame{
            inner_margin: Margin::ZERO,
            outer_margin: Margin::ZERO,
            ..Default::default()
        }.show(ui, |ui| {
            self.draw(ui);
        }).response
    }
}