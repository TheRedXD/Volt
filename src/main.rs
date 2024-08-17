use eframe::egui;
use egui::{Color32, FontId, LayerId, Pos2, Stroke, Ui};
mod visual;
mod blerp;
mod test;

use rodio::{
    Decoder, OutputStream,
    buffer::SamplesBuffer
};

fn soine() -> Vec<f64> {
    (0..44100)
        .map(|i| ((2.0 * std::f64::consts::PI) * (440.0 * (i as f64)) / 44100.0).sin())
        .collect()
}

fn soiniet(i: i32) -> f64 {
    ((2.0 * std::f64::consts::PI) * (440.0 * (i as f64)) / 44100.0).sin()
}

fn main() {
    // let processed_wave: Vec<f64> = soine().iter().map(|x| blerp::processing::effect_clipper(1.0, blerp::processing::effect_volume(0.8, *x))).collect();
    // blerp::wavefile::write_wav_file_f64(
    //     std::path::Path::new("./bungus64toPCM32.wav"),
    //     &blerp::f64_samples_mono_to_stereo(
    //         &processed_wave
    //     ),
    //     44100, 2, 32, 44100,
    //     blerp::wavefile::WaveAudioFormat::PulseCodeModulation
    // ).unwrap_or_else(|err| {
    //     println!("Error during wavefile write: {}", err);
    // });
    // return;
    // std::thread::spawn(|| {
    //     let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    //     let mut i = 0;
    //     loop {
    //         let samples = blerp::f64_size_to_f32(&vec![soiniet(i)]);
    //         stream_handle.play_raw(SamplesBuffer::new(1, 44100, samples)).unwrap();
    //         std::thread::sleep(std::time::Duration::from_micros(1_000_000/44100));
    //         i = (i + 1) % 44100;
    //     }
    // });
    test::cpaltest();
    let title = "Volt";
    let native_options = eframe::NativeOptions {
        multisampling: 8,
        vsync: false,
        ..Default::default()
    };
    let result: Result<(), eframe::Error> = eframe::run_native(
        title,
        native_options,
        Box::new(|cc| Ok(Box::new(VoltApp::new(cc)))),
    );
    match result {
        Ok(_) => {}
        Err(error) => {
            println!("{}", error);
        }
    }
}

enum BrowserCategory {
    Files,
    Devices
}

// #[derive(Default)]
struct VoltApp {
    pub browser_files_d: Vec<String>,
    pub browser_files_a: Vec<String>,
    pub browser_files_f: Vec<String>,
    pub browser_selected_category: BrowserCategory,
    pub browser_files_loaded: bool
}

impl VoltApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert("IBMPlexMono".to_owned(),
           egui::FontData::from_static(include_bytes!("../fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf")));
        fonts.families.insert(
            egui::FontFamily::Name("IBMPlexMono".into()),
            vec!["IBMPlexMono".to_owned()]
        );
        cc.egui_ctx.set_fonts(fonts);
        VoltApp {
            browser_files_d: vec![],
            browser_files_a: vec![],
            browser_files_f: vec![],
            browser_selected_category: BrowserCategory::Files,
            browser_files_loaded: false
        }
    }
}

const COLORS_NAVBAR: &str = "#262b3b";
const COLORS_NAVBAR_OUTLINE: &str = "#00000080";
const COLORS_BROWSER: &str = "#242938";
const COLORS_BROWSER_OUTLINE: &str = "#00000080";
const COLORS_BROWSER_SELECTED_BUTTON_FG: &str = "#ffcf7b";
const COLORS_BROWSER_UNSELECTED_BUTTON_FG: &str = "#646d88";
const COLORS_BROWSER_UNSELECTED_HOVER_BUTTON_FG: &str = "#8591b5";
const COLORS_BG_TEXT: &str = "#646987";

fn paint_browser_button(ctx: &egui::Context, ui: &mut Ui, frame: &mut eframe::Frame, full_rect: &egui::Rect, button_rect: &egui::Rect, x: f32, y: f32, width: f32, height: f32, selected: bool, text: &str) {
    let x2 = x+width;
    let y2 = y+height;
    // let button_rect = egui::Rect {
    //     min: egui::Pos2 { x, y },
    //     max: egui::Pos2 { x: x2, y: y2 }
    // };
    let mut color = COLORS_BROWSER_UNSELECTED_BUTTON_FG;
    if selected {
        color = COLORS_BROWSER_SELECTED_BUTTON_FG;
    } else {
        if ctx.rect_contains_pointer(ctx.layer_id_at(ctx.pointer_hover_pos().unwrap_or_default()).unwrap(), *button_rect) {
            color = COLORS_BROWSER_UNSELECTED_HOVER_BUTTON_FG;
        }
    }
    ui.painter().text(
        button_rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        FontId::new(14.0, egui::FontFamily::Name("IBMPlexMono".into())),
        // FontId::new(14.0, egui::FontFamily::Proportional),
        Color32::from_hex(color).unwrap_or(Default::default()));
    ui.painter().line_segment([
        egui::Pos2 { x: x+8.0, y: y2 },
        egui::Pos2 { x: x2-8.0, y: y2 }
    ], Stroke::new(0.5, egui::Color32::from_hex(color).unwrap_or(Default::default())));
}

fn paint_browser(ctx: &egui::Context, ui: &mut Ui, frame: &mut eframe::Frame, rect: &egui::Rect, dirs: &Vec<String>, audios: &Vec<String>, files: &Vec<String>, browser_category: &mut BrowserCategory) {
    // Browser box
    ui.painter().rect_filled(
        egui::Rect {
            min: egui::Pos2 { x: 0.0, y: 50.0 },
            max: egui::Pos2 { x: 300.0, y: rect.height() }
        },
        0.0,
        egui::Color32::from_hex(COLORS_BROWSER).unwrap_or(Default::default())
    );
    ui.painter().line_segment([
        egui::Pos2 {x: 300.0, y: 50.0},
        egui::Pos2 {x: 300.0, y: rect.height()}
    ], Stroke::new(0.5, egui::Color32::from_hex(COLORS_BROWSER_OUTLINE).unwrap_or(Default::default())));
    let files_open = match browser_category {
        BrowserCategory::Files => true,
        _ => false
    };
    let devices_open = match browser_category {
        BrowserCategory::Devices => true,
        _ => false
    };
    // Buttons
    let files_rect = egui::Rect {
        min: egui::Pos2 { x: 0.0, y: 55.0 },
        max: egui::Pos2 { x: 150.0, y: 55.0+30.0 }
    };
    let devices_rect = egui::Rect {
        min: egui::Pos2 { x: 150.0, y: 55.0 },
        max: egui::Pos2 { x: 150.0+150.0, y: 55.0+30.0 }
    };
    let was_pressed_and_where: (bool, egui::Pos2) = ctx.input(|i| {
        (i.pointer.button_released(egui::PointerButton::Primary), i.pointer.latest_pos().unwrap_or_default())
    });

    if was_pressed_and_where.0 {
        if files_rect.contains(was_pressed_and_where.1) {
            *browser_category = BrowserCategory::Files;
        } else if devices_rect.contains(was_pressed_and_where.1) {
            *browser_category = BrowserCategory::Devices;
        }
    }

    paint_browser_button(ctx, ui, frame, &rect, &files_rect, 0.0, 55.0, 150.0, 30.0, files_open, "Files");
    paint_browser_button(ctx, ui, frame, &rect, &devices_rect, 150.0, 55.0, 150.0, 30.0, devices_open, "Devices");

    if files_open {
        let mut y = 90.0;
        let mut sorted_dirs = dirs.clone();
        sorted_dirs.sort();
        sorted_dirs.iter().for_each(|name| {
            egui::Frame::none().show(ui, |ui| {
                let text = name;
                ui.painter().text(egui::Pos2 {
                    x: 30.0,
                    y
                }, egui::Align2::LEFT_TOP, text, FontId::new(14.0, egui::FontFamily::Name("IBMPlexMono".into())), egui::Color32::from_hex(COLORS_BROWSER_UNSELECTED_BUTTON_FG).unwrap_or(Default::default()));
            });

            egui::Image::new(egui::include_image!("../images/icons/folder.png"))
                .rounding(0.0)
                .texture_options(egui::TextureOptions { magnification: egui::TextureFilter::Linear, minification: egui::TextureFilter::Linear, wrap_mode: egui::TextureWrapMode::ClampToEdge })
                .paint_at(ui, egui::Rect {
                    min: egui::Pos2 { x: 10.0, y: y+2.0},
                    max: egui::Pos2 { x: 24.0, y: y+14.0+2.0 }
                });
            y += 16.0;
        });
        let mut sorted_audios = audios.clone();
        sorted_audios.sort();
        sorted_audios.iter().for_each(|name| {
            ui.painter().text(egui::Pos2 {
                x: 30.0,
                y
            }, egui::Align2::LEFT_TOP, name, FontId::new(14.0, egui::FontFamily::Name("IBMPlexMono".into())), egui::Color32::from_hex(COLORS_BROWSER_UNSELECTED_BUTTON_FG).unwrap_or(Default::default()));
            egui::Image::new(egui::include_image!("../images/icons/audio.png"))
                .rounding(0.0)
                .texture_options(egui::TextureOptions { magnification: egui::TextureFilter::Linear, minification: egui::TextureFilter::Linear, wrap_mode: egui::TextureWrapMode::ClampToEdge })
                .paint_at(ui, egui::Rect {
                    min: egui::Pos2 { x: 10.0, y: y+2.0 },
                    max: egui::Pos2 { x: 24.0, y: y+14.0+2.0 }
                });
            y += 16.0;
        });
        let mut sorted_files = files.clone();
        sorted_files.sort();
        sorted_files.iter().for_each(|name| {
            ui.painter().text(egui::Pos2 {
                x: 30.0,
                y
            }, egui::Align2::LEFT_TOP, name, FontId::new(14.0, egui::FontFamily::Name("IBMPlexMono".into())), egui::Color32::from_hex(COLORS_BROWSER_UNSELECTED_BUTTON_FG).unwrap_or(Default::default()));
            egui::Image::new(egui::include_image!("../images/icons/file.png"))
                .rounding(0.0)
                .texture_options(egui::TextureOptions { magnification: egui::TextureFilter::Linear, minification: egui::TextureFilter::Linear, wrap_mode: egui::TextureWrapMode::ClampToEdge })
                .paint_at(ui, egui::Rect {
                    min: egui::Pos2 { x: 10.0, y: y+2.0 },
                    max: egui::Pos2 { x: 24.0, y: y+14.0+2.0 }
                });
            y += 16.0;
        });
    }
}

fn paint_navbar(ctx: &egui::Context, ui: &mut Ui, frame: &mut eframe::Frame, rect: &egui::Rect) {
    ui.painter().rect_filled(
        egui::Rect {
            min: egui::Pos2 { x: 0.0, y: 0.0 },
            max: egui::Pos2 { x: rect.width(), y: 50.0 },
        },
        0.0,
        egui::Color32::from_hex(COLORS_NAVBAR).unwrap_or(Default::default()),
    );
    egui::Image::new(egui::include_image!("../icon.png"))
        .rounding(0.0)
        .texture_options(egui::TextureOptions { magnification: egui::TextureFilter::Linear, minification: egui::TextureFilter::Linear, wrap_mode: egui::TextureWrapMode::ClampToEdge })
        .paint_at(ui, egui::Rect {
           min: egui::Pos2 { x: 5.0, y: 5.0 },
           max: egui::Pos2 { x: 45.0, y: 45.0 }
        });
    ui.painter().text(
        Pos2 { x: 55.0, y: 8.0 },
        egui::Align2::LEFT_TOP,
        "Volt",
        FontId::new(20.0, egui::FontFamily::Proportional),
        Color32::from_hex("#ffffff").unwrap_or(Default::default()));
    ui.painter().text(
        Pos2 { x: 55.0, y: 28.0 },
        egui::Align2::LEFT_TOP,
        "Version INDEV",
        FontId::new(12.0, egui::FontFamily::Proportional),
        Color32::from_hex("#ffffff80").unwrap_or(Default::default()));

    ui.painter().line_segment([
        egui::Pos2 {x: 300.0, y: 50.0},
        egui::Pos2 {x: rect.width(), y: 50.0}
    ], Stroke::new(0.5, egui::Color32::from_hex(COLORS_NAVBAR_OUTLINE).unwrap_or(Default::default())));
}

impl eframe::App for VoltApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if !self.browser_files_loaded {
            let test_path = "/home/thered/Samples/";
            let readdir = std::fs::read_dir(test_path).unwrap();
            let mut dir_vector: Vec<String> = vec![];
            let mut audio_vector: Vec<String> = vec![];
            let mut file_vector: Vec<String> = vec![];
            readdir.into_iter().for_each(|item| {
                let name = String::from(item.as_ref().unwrap().file_name().to_str().unwrap());
                if item.as_ref().unwrap().metadata().unwrap().is_dir() {
                    dir_vector.push(name);
                } else {
                    if name.ends_with(".wav") || name.ends_with(".wave") || name.ends_with(".mp3") || name.ends_with(".ogg") || name.ends_with(".flac") || name.ends_with(".opus") {
                        audio_vector.push(name);
                    } else {
                        file_vector.push(name);
                    }
                }
            });
            self.browser_files_d = dir_vector;
            self.browser_files_a = audio_vector;
            self.browser_files_f = file_vector;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            let rect: egui::Rect = ctx.input(|i| i.viewport().inner_rect).unwrap();
            let width: f32 = rect.width();
            let height: f32 = rect.height();
            let formatted = format!("{}x{}", width, height);

            ui.painter().rect_filled(
                egui::Rect {
                    min: egui::Pos2 { x: 0.0, y: 0.0 },
                    max: egui::Pos2 { x: width, y: height },
                },
                0.0,
                egui::Color32::from_hex("#1e222f").unwrap_or(Default::default()),
            );

            // Paint Navbar
            paint_navbar(ctx, ui, frame, &rect);

            ui.painter().text(
                Pos2 { x: width / 2.0, y: height / 2.0 },
                egui::Align2::CENTER_CENTER,
                "In development",
                FontId::new(32.0, egui::FontFamily::Name("IBMPlexMono".into())),
                Color32::from_hex(COLORS_BG_TEXT).unwrap_or(Default::default()));

            // egui::Window::new("amogus").show(ctx, |ui| {
            //     ui.label("Hello, world!");
            //     ui.label(format!("Window size: {}", formatted));
            //     let sin: egui_plot::PlotPoints = (0..1000).map(|i| {
            //         let x = i as f64 * 0.01;
            //         [x, x.sin()]
            //     }).collect();
            //     let line = egui_plot::Line::new(sin);
            //     egui_plot::Plot::new("my_plot").view_aspect(2.0)
            //         .allow_drag(false)
            //         .allow_zoom(false)
            //         .allow_scroll(false)
            //         .allow_boxed_zoom(false)
            //         .show(ui, |plot_ui| {
            //             plot_ui.line(line);
            //         });
            // });

            // Paint the browser
            paint_browser(ctx, ui, frame, &rect, &self.browser_files_d, &self.browser_files_a, &self.browser_files_f, &mut self.browser_selected_category);
        });
    }
}
