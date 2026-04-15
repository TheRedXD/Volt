use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::HashSet,
    ffi::OsStr,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::Arc,
};

use gpui::{
    App, Context, InteractiveElement, IntoElement, List, ListAlignment, ListState, ParentElement, Render, StatefulInteractiveElement, Styled, UniformList, WeakEntity, Window, div, img, list, px,
    uniform_list,
};
use itertools::Itertools;
use sum_tree::{Bias, Dimension, Item, SeekTarget, SumTree, Summary};
use tap::Pipe;

use crate::{FILE_AUDIO_ICON, FILE_OTHER_ICON, theme::ThemeColors};

#[derive(Clone)]
struct Entry {
    path: Arc<Path>,
    depth: usize,
    data: EntryData,
}

#[derive(Clone, Copy)]
enum EntryData {
    File,
    Directory { open: bool },
}

impl Item for Entry {
    type Summary = EntrySummary;

    fn summary(&self, cx: <Self::Summary as Summary>::Context<'_>) -> Self::Summary {
        EntrySummary { count: 1 }
    }
}

#[derive(Clone, Copy)]
struct EntrySummary {
    count: usize,
}

impl Summary for EntrySummary {
    type Context<'a> = ();

    fn zero<'a>(cx: Self::Context<'a>) -> Self {
        Self { count: 0 }
    }

    fn add_summary<'a>(&mut self, summary: &Self, cx: Self::Context<'a>) {
        self.count += summary.count;
    }
}

#[derive(Clone, Copy)]
struct Index(usize);

impl<'a> Dimension<'a, EntrySummary> for Index {
    fn zero(cx: <EntrySummary as Summary>::Context<'_>) -> Self {
        Self(0)
    }

    fn add_summary(&mut self, summary: &'a EntrySummary, cx: <EntrySummary as Summary>::Context<'_>) {
        self.0 += summary.count;
    }
}

impl<'a> SeekTarget<'a, EntrySummary, Self> for Index {
    fn cmp(&self, cursor_location: &Self, cx: <EntrySummary as Summary>::Context<'_>) -> Ordering {
        Ord::cmp(&self.0, &cursor_location.0)
    }
}

pub struct BrowserView {
    theme: Arc<ThemeColors>,
    mode: Category,
    file_tree: SumTree<Entry>,
}

impl BrowserView {
    pub fn new(theme: Arc<ThemeColors>) -> Self {
        Self {
            theme,
            mode: Category::Files,
            file_tree: SumTree::from_item(
                Entry {
                    path: root_dir().to_path_buf().into(),
                    depth: 0,
                    data: EntryData::Directory { open: false },
                },
                (),
            ),
        }
    }
}

fn root_dir() -> &'static Path {
    cfg_select! {
        windows => todo!(),
        _ => Path::new("/"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Category {
    Files,
    Devices,
}

impl Render for BrowserView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .p_4()
            .gap_4()
            .child(div().flex().gap_4().children([("Files", Category::Files), ("Devices", Category::Devices)].map(|(name, category)| {
                let color = if self.mode == category {
                    self.theme.browser_selected_button_fg
                } else {
                    self.theme.browser_unselected_button_fg
                };
                div()
                    .child(name)
                    .py_1()
                    .px_2()
                    .border_b_1()
                    .text_color(color)
                    .border_color(color)
                    .flex_grow()
                    .text_center()
                    .hover(|style| {
                        if self.mode == category {
                            return style;
                        }
                        style
                            .text_color(self.theme.browser_unselected_hover_button_fg)
                            .border_color(self.theme.browser_unselected_hover_button_fg)
                    })
                    .id(name)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.mode = category;
                        cx.notify();
                    }))
            })))
            .child(if self.mode == Category::Files {
                let view = cx.weak_entity();
                uniform_list("files", self.file_tree.summary().count, move |range, window, cx| {
                    let view_outer = view.upgrade().unwrap().read(cx);
                    let mut cursor = view_outer.file_tree.cursor::<Index>(());
                    cursor.seek(&Index(range.start), Bias::Right);
                    let theme = Arc::clone(&view_outer.theme);
                    range
                        .map(|index| {
                            let Entry { path, depth, data } = cursor.item().unwrap();
                            let data = *data;
                            let element = div()
                                .w_full()
                                .flex()
                                .gap_2()
                                .pl(*depth as f32 * window.rem_size())
                                .items_center()
                                .id(path.to_string_lossy().into_owned())
                                .text_color(theme.browser_folder_text)
                                .hover(|style| style.text_color(theme.browser_folder_hover_text))
                                .pipe(|element| if path.read_dir().is_err() { element.cursor_not_allowed() } else { element })
                                .on_click({
                                    let path = path.to_path_buf();
                                    let view = view.clone();
                                    move |_, _, cx| match data {
                                        EntryData::File => open::that_detached(&path).unwrap(),
                                        EntryData::Directory { .. } => {
                                            if path.read_dir().is_err() {
                                                return;
                                            }
                                            let mut cursor = view.upgrade().unwrap().read(cx).file_tree.cursor::<Index>(());
                                            let mut new = cursor.slice(&Index(index), Bias::Right);
                                            let mut entry = cursor.item().cloned().unwrap();
                                            let Entry {
                                                data: EntryData::Directory { ref mut open },
                                                depth,
                                                ref path,
                                            } = entry
                                            else {
                                                unreachable!()
                                            };
                                            let path = Arc::clone(path);
                                            cursor.next();
                                            let suffix = cursor.suffix();
                                            *open = !*open;
                                            let open = *open;
                                            new.push(entry, ());
                                            if open {
                                                new.extend(
                                                    path.read_dir()
                                                        .into_iter()
                                                        .flatten()
                                                        .map(|entry| entry.unwrap().path())
                                                        .filter(|path| path.file_name().is_none_or(|name| !name.to_string_lossy().starts_with('.')))
                                                        .sorted_unstable()
                                                        .map(|path| Entry {
                                                            data: if path.is_dir() { EntryData::Directory { open: false } } else { EntryData::File },
                                                            path: path.into(),
                                                            depth: depth + 1,
                                                        }),
                                                    (),
                                                );
                                            }
                                            new.extend(suffix.cursor::<Index>(()).skip_while(|item| item.depth > depth).cloned(), ());
                                            drop(cursor);
                                            view.update(cx, |view, _| {
                                                view.file_tree = new;
                                            })
                                            .unwrap();
                                            cx.notify(view.entity_id());
                                        }
                                    }
                                })
                                .pipe(|element| match data {
                                    EntryData::Directory { open } => element.child(if open { "🔽" } else { "🔼" }),
                                    EntryData::File => element.child(
                                        img(
                                            if path
                                                .extension()
                                                .and_then(OsStr::to_str)
                                                .is_some_and(|extension| ["flac", "mp3", "ogg", "opus", "wav", "wave"].iter().any(|audio| audio.eq_ignore_ascii_case(extension)))
                                            {
                                                FILE_AUDIO_ICON
                                            } else {
                                                FILE_OTHER_ICON
                                            },
                                        )
                                        .size_4(),
                                    ),
                                })
                                .pipe(|entry| {
                                    match if **path == *root_dir() {
                                        Some("/".to_string())
                                    } else {
                                        path.file_name().and_then(|name| name.to_str()).map(ToString::to_string)
                                    } {
                                        Some(name) => entry.child(name),
                                        None => entry.child("(invalid path)").bg(theme.browser_invalid_name_bg),
                                    }
                                });
                            cursor.next();
                            element
                        })
                        .collect()
                })
                .flex_grow()
                .into_any_element()
            } else {
                div().id("devices").into_any_element()
            })
    }
}
