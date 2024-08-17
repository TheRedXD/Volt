use eframe::{egui, run_native, App, CreationContext, NativeOptions, Result};
use egui::{
    ecolor::HexColor, include_image, pos2, vec2, Align2, CentralPanel, Color32, Context, FontData,
    FontDefinitions, FontFamily, FontId, Image, LayerId, Pos2, Rect, Stroke, Ui,
};
use egui_extras::install_image_loaders;
use itertools::{chain, Itertools};
use std::{fs::read_dir, sync::LazyLock};
use strum::Display;
mod blerp;
mod test;
mod visual;

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

#[derive(Display, Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserCategory {
    Files,
    Devices,
}

struct Browser {
    directories: Vec<String>,
    audio: Vec<String>,
    files: Vec<String>,
    selected_category: BrowserCategory,
    files_loaded: bool,
}

struct VoltApp {
    pub browser: Browser,
}

impl Browser {
    fn paint_button(ctx: &Context, ui: &Ui, button: &Rect, selected: bool, text: &str) {
        let color = if selected {
            *COLORS_BROWSER_SELECTED_BUTTON_FG
        } else if ctx.rect_contains_pointer(
            ctx.layer_id_at(ctx.pointer_hover_pos().unwrap_or_default())
                .unwrap_or_else(LayerId::background),
            *button,
        ) {
            *COLORS_BROWSER_UNSELECTED_HOVER_BUTTON_FG
        } else {
            *COLORS_BROWSER_UNSELECTED_BUTTON_FG
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
                    x: button.left() + 8.0,
                    y: button.bottom(),
                },
                Pos2 {
                    x: button.right() - 8.0,
                    y: button.bottom(),
                },
            ],
            Stroke::new(0.5, color),
        );
    }

    fn paint(&mut self, ctx: &Context, ui: &mut Ui, viewport: &Rect) {
        ui.painter().rect_filled(
            Rect {
                min: Pos2 { x: 0.0, y: 50.0 },
                max: Pos2 {
                    x: 300.0,
                    y: viewport.height(),
                },
            },
            0.0,
            *COLORS_BROWSER,
        );
        ui.painter().line_segment(
            [
                Pos2 { x: 300.0, y: 50.0 },
                Pos2 {
                    x: 300.0,
                    y: viewport.height(),
                },
            ],
            Stroke::new(0.5, *COLORS_BROWSER_OUTLINE),
        );
        let Some((was_pressed, press_position)) = ctx.input(|input_state| {
            Some((
                input_state
                    .pointer
                    .button_released(egui::PointerButton::Primary),
                input_state.pointer.latest_pos()?,
            ))
        }) else {
            return;
        };
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
            Self::paint_button(ctx, ui, &rect, open, category.to_string().as_str());
            if was_pressed && rect.contains(press_position) {
                self.selected_category = category;
            }
        }
        match self.selected_category {
            BrowserCategory::Files => {
                for (index, name) in chain!(
                    self.directories.iter().sorted_unstable(),
                    self.audio.iter().sorted_unstable(),
                    self.files.iter().sorted_unstable(),
                )
                .enumerate()
                {
                    #[allow(clippy::cast_precision_loss)]
                    let y = (index as f32).mul_add(16.0, 90.);
                    egui::Frame::none().show(ui, |ui| {
                        ui.painter().text(
                            pos2(30., y),
                            Align2::LEFT_TOP,
                            name,
                            FontId::new(14.0, FontFamily::Name("IBMPlexMono".into())),
                            *COLORS_BROWSER_UNSELECTED_BUTTON_FG,
                        )
                    });

                    Image::new(include_image!("images/icons/folder.png"))
                        .paint_at(ui, Rect::from_min_size(pos2(10., y + 2.), vec2(14., 14.)));
                }
            }
            BrowserCategory::Devices => {
                // TODO show some devices here!
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
                directories: Vec::new(),
                audio: Vec::new(),
                files: Vec::new(),
                selected_category: BrowserCategory::Files,
                files_loaded: false,
            },
        }
    }
}

macro_rules! colors {
    ($($name:ident $color:tt )*) => {
        $(
            static $name: LazyLock<Color32> = LazyLock::new(|| {
                HexColor::from_str_without_hash(stringify!($color)).unwrap().color()
            });
        )*
    };
}

colors! {
    COLORS_NAVBAR                               262b3b
    COLORS_NAVBAR_OUTLINE                       00000080
    COLORS_BROWSER                              242938
    COLORS_BROWSER_OUTLINE                      00000080
    COLORS_BROWSER_SELECTED_BUTTON_FG           ffcf7b
    COLORS_BROWSER_UNSELECTED_BUTTON_FG         646d88
    COLORS_BROWSER_UNSELECTED_HOVER_BUTTON_FG   8591b5
    COLORS_BG_TEXT                              646987
}

fn paint_navbar(ui: &Ui, viewport: &Rect) {
    ui.painter()
        .rect_filled(viewport.with_max_y(50.), 0.0, *COLORS_NAVBAR);
    Image::new(include_image!("images/icons/icon.png"))
        .paint_at(ui, Rect::from_min_size(pos2(5., 5.), vec2(40., 40.)));
    ui.painter().text(
        pos2(55., 8.),
        Align2::LEFT_TOP,
        "Volt",
        FontId::new(20.0, FontFamily::Proportional),
        Color32::from_hex("#ffffff").unwrap_or_default(),
    );
    ui.painter().text(
        pos2(55., 28.),
        Align2::LEFT_TOP,
        "Version INDEV",
        FontId::new(12.0, FontFamily::Proportional),
        Color32::from_hex("#ffffff80").unwrap_or_default(),
    );

    ui.painter().line_segment(
        [pos2(300., 50.), pos2(viewport.width(), 50.)],
        Stroke::new(0.5, *COLORS_NAVBAR_OUTLINE),
    );
}

impl App for VoltApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        'load: {
            if !self.browser.files_loaded {
                let Ok(entries) = read_dir(include_str!("test_path").trim()) else {
                    break 'load;
                };
                self.browser.directories.clear();
                self.browser.audio.clear();
                self.browser.files.clear();
                for item in entries {
                    let name = String::from(item.as_ref().unwrap().file_name().to_str().unwrap());
                    if item.as_ref().unwrap().metadata().unwrap().is_dir() {
                        self.browser.directories.push(name);
                    } else if [".wav", ".wave", ".mp3", ".ogg", ".flac", ".opus"]
                        .into_iter()
                        .any(|extension| name.ends_with(extension))
                    {
                        self.browser.audio.push(name);
                    } else {
                        self.browser.files.push(name);
                    }
                }
            }
        }
        CentralPanel::default().show(ctx, |ui| {
            let viewport: Rect = ctx
                .input(|input_state| input_state.viewport().inner_rect)
                .unwrap();
            ui.painter().rect_filled(
                Rect::from_min_size(Pos2::ZERO, viewport.size()),
                0.0,
                Color32::from_hex("#1e222f").unwrap_or_default(),
            );

            paint_navbar(ui, &viewport);

            ui.painter().text(
                Rect::from_min_size(Pos2::ZERO, viewport.size()).center(),
                Align2::CENTER_CENTER,
                "In development",
                FontId::new(32.0, FontFamily::Name("IBMPlexMono".into())),
                *COLORS_BG_TEXT,
            );

            self.browser.paint(ctx, ui, &viewport);
        });
    }
}
