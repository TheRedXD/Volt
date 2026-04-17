use std::{
    any::Any,
    borrow::Cow,
    cmp::Ordering,
    collections::HashSet,
    ffi::OsStr,
    iter::from_fn,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::Arc,
};

use gpui::{
    App, AppContext, BorrowAppContext, Context, ElementId, ExternalPaths, InteractiveElement, IntoElement, List, ListAlignment, ListState, ParentElement, Render, StatefulInteractiveElement, Styled,
    UniformList, WeakEntity, Window, deferred, div, hash, img, list, prelude::FluentBuilder, px, uniform_list,
};
use itertools::{Itertools, repeat_n};
use sum_tree::{Bias, Dimension, Item, SeekTarget, SumTree, Summary};
use tap::{Pipe, Tap};

use crate::{Drag, DragInner, FILE_AUDIO_ICON, FILE_OTHER_ICON, theme::ThemeColors};

#[derive(Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: Arc<Path>,
    depth: usize,
    data: EntryData,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EntryData {
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
pub struct EntrySummary {
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

#[derive(Clone)]
pub struct EntryDragPayload(pub Entry);

struct EntryDrag {
    payload: EntryDragPayload,
    theme: Arc<ThemeColors>,
}

impl Render for EntryDrag {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .text_color(self.theme.bg_text)
            .bg(self.theme.browser)
            .border_1()
            .p_2()
            .rounded_md()
            .border_color(self.theme.browser_outline)
            .child(self.payload.0.path.to_string_lossy().to_string())
    }
}

impl Render for BrowserView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .flex_grow()
            .p_4()
            .gap_4()
            .bg(self.theme.browser)
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
            .child(
                div()
                    .flex_grow()
                    .relative()
                    .flex()
                    .flex_col()
                    .child(if self.mode == Category::Files {
                        let view = cx.weak_entity();
                        uniform_list("files", self.file_tree.summary().count, move |range, window, cx| {
                            let view_outer = view.upgrade().unwrap().read(cx);
                            let mut cursor = view_outer.file_tree.cursor::<Index>(());
                            cursor.seek(&Index(range.start), Bias::Right);
                            let theme = Arc::clone(&view_outer.theme);
                            range
                                .map(|index| {
                                    let entry @ Entry { path, depth, data } = cursor.item().unwrap();
                                    let data = *data;
                                    let element = div()
                                        .w_full()
                                        .flex()
                                        .gap_4()
                                        .items_center()
                                        .id(index)
                                        .text_color(theme.browser_folder_text)
                                        .hover(|style| style.text_color(theme.browser_folder_hover_text))
                                        .when(matches!(data, EntryData::Directory { .. }) && path.read_dir().is_err(), |element| element.cursor_not_allowed())
                                        .on_drag(EntryDragPayload(entry.clone()), {
                                            let theme = Arc::clone(&theme);
                                            move |payload, offset, window, cx| {
                                                let theme = Arc::clone(&theme);
                                                let view = cx.new(move |_| EntryDrag { payload: payload.clone(), theme });
                                                cx.update_global(|Drag(drag), _| {
                                                    *drag = Some(DragInner {
                                                        start: window.mouse_position(),
                                                        item: gpui::AnyDrag {
                                                            view: view.clone().into(),
                                                            value: Arc::new(payload.clone()) as Arc<dyn Any>,
                                                            cursor_offset: offset,
                                                            cursor_style: None,
                                                        },
                                                    });
                                                });
                                                view
                                            }
                                        })
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
                                        .absolute()
                                        .children(from_fn(|| Some(div().bg(theme.browser_outline).w_px().self_stretch().flex_shrink_0())).take(*depth))
                                        .pipe(|element| match data {
                                            EntryData::Directory { open } => element.child(if open { "🔼" } else { "🔽" }),
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
                                                .size_4()
                                                .flex_shrink_0(),
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
                    .children(
                        cx.global::<Drag>()
                            .0
                            .as_ref()
                            .and_then(|drag| drag.item.value.downcast_ref())
                            .map(|EntryDragPayload(Entry { path, depth, data })| {
                                let in_workspace = self.file_tree.iter().position(|entry| entry.depth == 0 && entry.path == *path);
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .justify_center()
                                            .items_center()
                                            .rounded_md()
                                            .bg(self.theme.hover)
                                            .flex_grow()
                                            .child(if in_workspace.is_some() {
                                                format!("Drop to add a copy of {} to workspace", path.display())
                                            } else {
                                                format!("Drop to add {} to workspace", path.display())
                                            })
                                            .on_drop(cx.listener(|view, EntryDragPayload(entry), _, _| {
                                                view.file_tree.push(entry.clone().tap_mut(|entry| entry.depth = 0), ());
                                            })),
                                    )
                                    .when_some(in_workspace, |element, index| {
                                        element.child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .justify_center()
                                                .items_center()
                                                .rounded_md()
                                                .bg(self.theme.hover)
                                                .flex_grow()
                                                .child(format!("Drop to remove {} from workspace", path.display()))
                                                .on_drop(cx.listener(move |view, _: &EntryDragPayload, _, _| {
                                                    let mut cursor = view.file_tree.cursor::<Index>(());
                                                    let mut new = cursor.slice(&Index(index), Bias::Right);
                                                    cursor.next();
                                                    new.append(cursor.suffix(), ());
                                                    drop(cursor);
                                                    view.file_tree = new;
                                                })),
                                        )
                                    })
                                    .id("drag-overlay")
                                    .invisible()
                                    .drag_over(|style, _: &EntryDragPayload, _, _| style.visible())
                                    .on_drop(|_: &EntryDragPayload, _, _| {})
                            }),
                    ),
            )
    }
}
