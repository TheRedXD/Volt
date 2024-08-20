use eframe::{egui, run_native, App, CreationContext, NativeOptions, Result};
use egui::{
    include_image, pos2, vec2, Align2, CentralPanel, Color32, Context, FontData, FontDefinitions,
    FontFamily, FontId, Image, LayerId, PointerButton, Pos2, Rect, Stroke, Ui,
};
use egui_extras::install_image_loaders;
use open::that_detached;
use std::{collections::BTreeSet, fs::read_dir};
mod blerp;
mod test;
// TODO: Move everything into components (visual)
mod visual;
mod paint;
mod browser;

use visual::ThemeColors;
use browser::{ Browser, BrowserCategory, BrowserEntry, BrowserEntryKind };

fn main() -> Result {
    let title = "Volt";
    let native_options = NativeOptions {
        vsync: false,
        ..Default::default()
    };
    run_native(
        title,
        native_options,
        Box::new(|cc| Ok(Box::new(VoltApp::new(cc)))),
    )
}

struct VoltApp {
    pub browser: Browser,
    pub themes: ThemeColors,
}

fn hovered(ctx: &Context, rect: &Rect) -> bool {
    ctx.rect_contains_pointer(
        ctx.layer_id_at(ctx.pointer_hover_pos().unwrap_or_default())
            .unwrap_or_else(LayerId::background),
        *rect,
    )
}

impl Browser {
    fn paint_button(
        ctx: &Context,
        ui: &Ui,
        button: &Rect,
        selected: bool,
        text: &str,
        theme: &ThemeColors,
    ) {
        let color = if selected {
            theme.browser_selected_button_fg
        } else if hovered(ctx, button) {
            theme.browser_unselected_hover_button_fg
        } else {
            theme.browser_unselected_button_fg
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

    fn paint(&mut self, ctx: &Context, ui: &mut Ui, viewport: &Rect, theme: &ThemeColors) {
        ui.painter().rect_filled(
            Rect {
                min: Pos2 { x: 0., y: 50. },
                max: Pos2 {
                    x: 300.,
                    y: viewport.height(),
                },
            },
            0.0,
            theme.browser,
        );
        ui.painter().line_segment(
            [
                Pos2 { x: 300., y: 50. },
                Pos2 {
                    x: 300.,
                    y: viewport.height(),
                },
            ],
            Stroke::new(0.5, theme.browser_outline),
        );
        let (was_pressed, press_position) = ctx
            .input(|input_state| {
                Some((
                    input_state.pointer.button_released(PointerButton::Primary),
                    Some(input_state.pointer.latest_pos()?),
                ))
            })
            .unwrap_or((false, None));
        for (category, rect) in [
            (
                BrowserCategory::Files,
                Rect::from_min_size(pos2(0., 55.), vec2(150., 30.)),
            ),
            (
                BrowserCategory::Devices,
                Rect::from_min_size(pos2(150., 55.), vec2(150., 30.)),
            ),
        ] {
            let open = self.selected_category == category;
            Self::paint_button(ctx, ui, &rect, open, category.to_string().as_str(), theme);
            if press_position
                .is_some_and(|press_position| was_pressed && rect.contains(press_position))
            {
                self.selected_category = category;
            }
        }
        match self.selected_category {
            BrowserCategory::Files => {
                for (index, entry) in self.entries.iter().enumerate() {
                    #[allow(clippy::cast_precision_loss)]
                    let y = (index as f32).mul_add(16.0, 90.);
                    let rect = &Rect::from_min_size(pos2(0., y), vec2(300., 16.));
                    egui::Frame::none().show(ui, |ui| {
                        let name = entry.path.file_name().unwrap().to_str().unwrap();
                        ui.painter().text(
                            pos2(30., y),
                            Align2::LEFT_TOP,
                            if name.len() > 30 {
                                name[..30].to_string() + "..."
                            } else {
                                name.to_string()
                            },
                            FontId::new(14., FontFamily::Name("IBMPlexMono".into())),
                            if hovered(ctx, rect) {
                                theme.browser_unselected_hover_button_fg
                            } else {
                                theme.browser_unselected_button_fg
                            },
                        )
                    });

                    Image::new(match entry.kind {
                        BrowserEntryKind::Directory => include_image!("images/icons/folder.png"),
                        BrowserEntryKind::Audio => include_image!("images/icons/audio.png"),
                        BrowserEntryKind::File => include_image!("images/icons/file.png"),
                    })
                    .paint_at(ui, Rect::from_min_size(pos2(10., y + 2.), vec2(14., 14.)));
                    if press_position
                        .is_some_and(|press_position| was_pressed && rect.contains(press_position))
                    {
                        match entry.kind {
                            BrowserEntryKind::Directory => {
                                self.path.clone_from(&entry.path);
                                break;
                            }
                            BrowserEntryKind::Audio => {
                                // TODO play some audio
                            }
                            BrowserEntryKind::File => {
                                that_detached(entry.path.clone()).unwrap();
                            }
                        }
                    }
                }
            }
            BrowserCategory::Devices => {
                // TODO: Show some devices here!
            }
        }
    }
}

impl VoltApp {
    fn new(cc: &CreationContext<'_>) -> Self {
        install_image_loaders(&cc.egui_ctx);
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            "IBMPlexMono".to_owned(),
            FontData::from_static(include_bytes!(
                "fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf"
            )),
        );
        fonts.families.insert(
            FontFamily::Name("IBMPlexMono".into()),
            vec!["IBMPlexMono".to_owned()],
        );
        cc.egui_ctx.set_fonts(fonts);
        Self {
            browser: Browser {
                entries: BTreeSet::new(),
                selected_category: BrowserCategory::Files,
                path: "/".into(),
            },
            themes: ThemeColors::default(),
        }
    }
}

impl App for VoltApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        'load: {
            let Ok(entries) = read_dir(&self.browser.path) else {
                break 'load;
            };
            self.browser.entries.clear();
            for entry in entries {
                let entry = entry.as_ref();
                let path = entry.unwrap().path();
                if entry.unwrap().metadata().unwrap().is_dir() {
                    self.browser.entries.insert(BrowserEntry {
                        path,
                        kind: BrowserEntryKind::Directory,
                    });
                } else if [".wav", ".wave", ".mp3", ".ogg", ".flac", ".opus"]
                    .into_iter()
                    .any(|extension| {
                        entry
                            .unwrap()
                            .file_name()
                            .to_str()
                            .unwrap()
                            .ends_with(extension)
                    })
                {
                    self.browser.entries.insert(BrowserEntry {
                        path,
                        kind: BrowserEntryKind::Audio,
                    });
                } else {
                    self.browser.entries.insert(BrowserEntry {
                        path,
                        kind: BrowserEntryKind::File,
                    });
                }
            }
        }
        CentralPanel::default().show(ctx, |ui| {
            // FIXME: This is a hack to get Volt temporarily in a somewhat working state.
            let temp_w = 1920.;
            let temp_h = 1080.;
            // ^^^^^^^^^^^^^^^ Not production code!

            let viewport: Rect = ctx
                .input(|input_state| input_state.viewport().inner_rect)
                // Likely should have much better handling of this particular error here
                .unwrap_or(Rect {
                    min: Pos2 { x: 0., y: 0. },
                    max: Pos2 {
                        x: temp_w,
                        y: temp_h,
                    },
                });
            ui.painter().rect_filled(
                Rect::from_min_size(Pos2::ZERO, viewport.size()),
                0.0,
                Color32::from_hex("#1e222f").unwrap_or_default(),
            );

            paint::paint_navbar(ui, &viewport, &self.themes);

            ui.painter().text(
                Rect::from_min_size(Pos2::ZERO, viewport.size()).center(),
                Align2::CENTER_CENTER,
                "In development",
                FontId::new(32.0, FontFamily::Name("IBMPlexMono".into())),
                self.themes.bg_text,
            );

            self.browser.paint(ctx, ui, &viewport, &self.themes);
        });
    }
}
