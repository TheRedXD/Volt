pub(crate) mod entry;
pub(crate) mod preview;

use crate::blerp::device::DeviceEntry;
use crate::images::DEVICE_IMAGE;
use crate::visual::ThemeColors;
use egui::{
    pos2, vec2, Align2, Context, FontFamily, FontId, Image, LayerId, PointerButton, Pos2, Rect,
    Stroke, Ui,
};
pub use entry::{BrowserCategory, BrowserEntry};
use open::that_detached;
pub use preview::Preview;
use std::path::PathBuf;
use unicode_truncate::UnicodeTruncateStr;

fn hovered(ctx: &Context, rect: &Rect) -> bool {
    ctx.rect_contains_pointer(
        ctx.layer_id_at(ctx.pointer_hover_pos().unwrap_or_default())
            .unwrap_or_else(LayerId::background),
        *rect,
    )
}

pub struct Browser {
    pub entries: Vec<BrowserEntry>,
    pub selected_category: BrowserCategory,
    pub path: PathBuf,
    pub preview_tx: std::sync::mpsc::Sender<PathBuf>,
    pub offset_y: f32,
    pub began_scroll: bool,
    pub dragging_audio: bool,
    pub dragging_audio_text: String,
    pub sidebar_width: f32,
    pub started_drag: bool,
    pub devices: Vec<DeviceEntry>,
}

impl Browser {
    pub fn paint_button(
        ctx: &Context,
        ui: &Ui,
        button: &Rect,
        selected: bool,
        text: &str,
        theme: &ThemeColors,
    ) {
        let color = match selected {
            true => theme.browser_selected_button_fg,
            false if hovered(ctx, button) => theme.browser_unselected_hover_button_fg,
            _ => theme.browser_unselected_button_fg,
        };

        ui.painter().text(
            button.center(),
            Align2::CENTER_CENTER,
            text,
            FontId::new(14.0, FontFamily::Name("IBMPlexMono".into())),
            color,
        );
        ui.painter().line_segment(
            [
                Pos2 {
                    x: button.left() + 8.,
                    y: button.bottom(),
                },
                Pos2 {
                    x: button.right() - 8.,
                    y: button.bottom(),
                },
            ],
            Stroke::new(0.5, color),
        );
    }

    pub fn paint(&mut self, ctx: &Context, ui: &mut Ui, viewport: &Rect, theme: &ThemeColors) {
        ui.painter().rect_filled(
            Rect {
                min: Pos2 { x: 0., y: 50. },
                max: Pos2 {
                    x: self.sidebar_width,
                    y: viewport.height(),
                },
            },
            0.0,
            theme.browser,
        );
        ui.painter().line_segment(
            [
                Pos2 {
                    x: self.sidebar_width,
                    y: 50.,
                },
                Pos2 {
                    x: self.sidebar_width,
                    y: viewport.height(),
                },
            ],
            Stroke::new(0.5, theme.browser_outline),
        );
        let (was_pressed, press_position) = ctx
            .input(|input_state| {
                Some((
                    input_state.pointer.button_released(PointerButton::Primary),
                    input_state.pointer.latest_pos(),
                ))
            })
            .unwrap_or((false, None));

        let category_rects = [
            (
                BrowserCategory::Files,
                Rect::from_min_size(pos2(0., 55.), vec2(self.sidebar_width / 2., 30.)),
            ),
            (
                BrowserCategory::Devices,
                Rect::from_min_size(
                    pos2(self.sidebar_width / 2., 55.),
                    vec2(self.sidebar_width / 2., 30.),
                ),
            ),
        ];

        for (category, rect) in category_rects {
            Self::paint_button(
                ctx,
                ui,
                &rect,
                self.selected_category == category,
                category.to_string().as_str(),
                theme,
            );
            if press_position.is_some_and(|position| was_pressed && rect.contains(position)) {
                self.selected_category = category;
            }
        }

        match ctx.input(|i| (i.smooth_scroll_delta.y, i.pointer.latest_pos())) {
            (scroll, latest_pos) if scroll != 0_f32 => {
                if latest_pos.is_some_and(|pos| pos.x <= self.sidebar_width && !self.began_scroll) {
                    self.began_scroll = true;
                }

                if self.began_scroll {
                    self.offset_y += scroll;
                }
            }
            (0_f32, _) if self.began_scroll => self.began_scroll = false,
            _ => (),
        }

        match self.selected_category {
            BrowserCategory::Files => {
                // Add ".." entry if not at root
                if self.path != AsRef::<std::path::Path>::as_ref("/") {
                    let b = BrowserEntry::Directory(
                        self.path.parent().unwrap_or(&self.path).to_path_buf(),
                    );
                    self.entries.insert(0, b);
                }

                // Calculate the maximum offset based on the number of entries and browser height
                let max_entries = self.entries.len();
                let browser_height = viewport.height() - 90.0; // Adjust for header height
                let bottom_margin = 8.0; // Add a slight margin at the bottom
                let max_offset = (max_entries as f32 * 16.0) - browser_height + bottom_margin;

                // Clamp the offset
                self.offset_y = self.offset_y.clamp(-max_offset.max(0.0), 0.0);

                // Handle sidebar resizing
                let resize_rect = Rect::from_min_size(
                    pos2(self.sidebar_width - 5., 50.),
                    vec2(10., viewport.height() - 50.),
                );
                if ctx.rect_contains_pointer(LayerId::background(), resize_rect) {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                    if ctx.input(|i| i.pointer.primary_pressed()) {
                        self.started_drag = true;
                    }
                }
                if self.started_drag {
                    if let Some(mouse_pos) = ctx.pointer_hover_pos() {
                        self.sidebar_width = mouse_pos.x.clamp(100.0, viewport.width() / 2.0);
                    }
                    if ctx.input(|i| i.pointer.primary_released()) {
                        self.started_drag = false;
                    }
                }

                // Draw resize handle
                if ctx
                    .pointer_hover_pos()
                    .is_some_and(|pos| (pos.x - self.sidebar_width).abs() <= 20.0)
                {
                    ui.painter().rect_filled(
                        resize_rect,
                        0.0,
                        theme.browser_outline.gamma_multiply(0.2),
                    );
                }

                for (index, entry) in self.entries.iter().enumerate() {
                    #[allow(clippy::cast_precision_loss)]
                    let y = (index as f32).mul_add(16.0, 90.);
                    let rect = Rect::from_min_size(
                        pos2(0., y + self.offset_y),
                        vec2(self.sidebar_width, 16.),
                    );

                    egui::Frame::none().show(ui, |ui| {
                        ui.allocate_space(ui.available_size());
                        let mut invalid = false;

                        let name = {
                            if self.path.parent().is_some_and(|p| p == entry.path()) {
                                ".."
                            } else {
                                entry
                                    .path()
                                    .file_name()
                                    .and_then(|name| name.to_str())
                                    .unwrap_or_else(|| {
                                        invalid = true;
                                        "• Invalid Name •"
                                    })
                            }
                        };

                        if y + self.offset_y >= 90. {
                            let text_width = ui
                                .painter()
                                .layout_no_wrap(
                                    name.to_string(),
                                    FontId::new(14., FontFamily::Name("IBMPlexMono".into())),
                                    theme.browser_unselected_button_fg,
                                )
                                .rect
                                .width();

                            let char_width = ui
                                .painter()
                                .layout_no_wrap(
                                    "a".to_string(),
                                    FontId::new(14., FontFamily::Name("IBMPlexMono".into())),
                                    theme.browser_unselected_button_fg,
                                )
                                .rect
                                .width();

                            if invalid {
                                ui.painter().rect_filled(
                                    Rect::from_min_size(
                                        pos2(30., y + self.offset_y),
                                        vec2(text_width, 16.),
                                    ),
                                    0.0,
                                    theme.browser_invalid_name_bg,
                                );
                            }

                            let chars_to_truncate = (self.sidebar_width / char_width) as usize - 10;

                            let font = FontId::new(14., FontFamily::Name("IBMPlexMono".into()));
                            let theme = match hovered(ctx, &rect) {
                                true if invalid => theme.browser_unselected_hover_button_fg_invalid,
                                true => theme.browser_unselected_hover_button_fg,
                                false if invalid => theme.browser_unselected_button_fg_invalid,
                                false => theme.browser_unselected_button_fg,
                            };

                            let name = {
                                if name.unicode_truncate(chars_to_truncate).1 == chars_to_truncate {
                                    name.unicode_truncate(chars_to_truncate).0.to_string() + "..."
                                } else {
                                    name.to_string()
                                }
                            };

                            ui.painter().text(
                                pos2(30., y + self.offset_y),
                                Align2::LEFT_TOP,
                                name,
                                font,
                                theme,
                            )
                        } else {
                            Rect {
                                min: Pos2 { x: 0., y: 0. },
                                max: Pos2 { x: 0., y: 0. },
                            }
                        }
                    });

                    if y + self.offset_y >= 90. {
                        Image::new(entry.image()).paint_at(
                            ui,
                            Rect::from_min_size(pos2(10., y + 2. + self.offset_y), vec2(14., 14.)),
                        );
                    }

                    if entry.is_audio() {
                        let is_dragging = ctx.input(|i| i.pointer.is_decidedly_dragging());

                        if let Some(cursor_pos) = ctx.input(|i| i.pointer.hover_pos()) {
                            let entry_file_name = entry
                                .path()
                                .file_name()
                                .and_then(|f| f.to_str())
                                .unwrap_or("");

                            if is_dragging
                                && rect.contains(cursor_pos)
                                && !self.dragging_audio
                                && cursor_pos.x <= self.sidebar_width - 10.
                                && !self.started_drag
                            {
                                self.dragging_audio = true;
                                self.dragging_audio_text = entry_file_name.to_owned();
                            }

                            if self.dragging_audio && self.dragging_audio_text == entry_file_name {
                                ui.painter().text(
                                    cursor_pos + vec2(5.0, 2.0),
                                    Align2::CENTER_CENTER,
                                    &self.dragging_audio_text,
                                    FontId::new(14.0, FontFamily::Name("IBMPlexMono".into())),
                                    theme.browser_selected_button_fg,
                                );
                            }
                        }

                        if !is_dragging {
                            self.dragging_audio = false;
                            self.dragging_audio_text = String::new();
                        }
                    }

                    if press_position.is_some_and(|pos| {
                        was_pressed
                            && rect.contains(pos)
                            && pos.x <= self.sidebar_width - 10.
                            && pos.y >= 90.
                    }) && !self.dragging_audio
                    {
                        match entry {
                            BrowserEntry::Directory(path) => {
                                match self.path.parent() {
                                    Some(p) if path == p => {
                                        self.path = p.to_path_buf();
                                    }
                                    _ => self.path = path.to_owned(),
                                }

                                break;
                            }
                            BrowserEntry::Audio(path) => {
                                // TODO: Proper preview implementation with cpal. This is temporary (or at least make it work well with a proper preview widget)
                                // Also, don't spawn a new thread - instead, dedicate a thread for preview
                                self.preview_tx.send(path.to_path_buf()).unwrap_or_default();
                            }
                            BrowserEntry::File(path) => {
                                that_detached(path).unwrap();
                            }
                        }
                    }
                }
            }
            BrowserCategory::Devices => {
                for (index, entry) in self.devices.iter().enumerate() {
                    #[allow(clippy::cast_precision_loss)]
                    let y = (index as f32).mul_add(16.0, 90.);
                    let rect = Rect::from_min_size(
                        pos2(0., y + self.offset_y),
                        vec2(self.sidebar_width, 16.),
                    );

                    egui::Frame::none().show(ui, |ui| {
                        ui.allocate_space(ui.available_size());

                        if y + self.offset_y >= 90. {
                            let char_width = ui
                                .painter()
                                .layout_no_wrap(
                                    "a".to_string(),
                                    FontId::new(14., FontFamily::Name("IBMPlexMono".into())),
                                    theme.browser_unselected_button_fg,
                                )
                                .rect
                                .width();

                            let chars_to_truncate = (self.sidebar_width / char_width) as usize - 10;

                            let font = FontId::new(14., FontFamily::Name("IBMPlexMono".into()));
                            let theme = match hovered(ctx, &rect) {
                                true => theme.browser_unselected_hover_button_fg,
                                false => theme.browser_unselected_button_fg,
                            };

                            let name = {
                                if entry.name().unicode_truncate(chars_to_truncate).1
                                    == chars_to_truncate
                                {
                                    entry
                                        .name()
                                        .unicode_truncate(chars_to_truncate)
                                        .0
                                        .to_string()
                                        + "..."
                                } else {
                                    entry.name().to_string()
                                }
                            };

                            ui.painter().text(
                                pos2(30., y + self.offset_y),
                                Align2::LEFT_TOP,
                                name,
                                font,
                                theme,
                            )
                        } else {
                            Rect {
                                min: Pos2 { x: 0., y: 0. },
                                max: Pos2 { x: 0., y: 0. },
                            }
                        }
                    });

                    if y + self.offset_y >= 90. {
                        Image::new(DEVICE_IMAGE).paint_at(
                            ui,
                            Rect::from_min_size(pos2(10., y + 2. + self.offset_y), vec2(14., 14.)),
                        );
                    }

                    if press_position.is_some_and(|pos| {
                        was_pressed
                            && rect.contains(pos)
                            && pos.x <= self.sidebar_width - 10.
                            && pos.y >= 90.
                    }) {
                        entry.beep(1).unwrap_or_else(|e| {
                            panic!("Error playing device: {} - {e}", entry.name())
                        })
                    }
                }
            }
        }
    }
}
