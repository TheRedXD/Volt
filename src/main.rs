use eframe::{egui, run_native, App, CreationContext, NativeOptions};
use egui::{
    Align2, CentralPanel, Color32, Context, FontData, FontDefinitions, FontFamily, FontId, Pos2,
    Rect,
};
use egui_extras::install_image_loaders;
use std::{collections::BTreeSet, fs::read_dir};
mod blerp;
mod test;
// TODO: Move everything into components (visual)
mod browser;
mod info;
mod visual;

use browser::{Browser, BrowserCategory, BrowserEntry, BrowserEntryKind};
use visual::ThemeColors;

fn main() -> eframe::Result {
    if std::env::args().any(|x| x == *"--info") {
        info::dump();
    }
    // Panic handling
    std::panic::set_hook(Box::new(|panic_info| {
        info::panic_handler(panic_info);
    }));
    let title = "Volt";
    let native_options = NativeOptions {
        vsync: true,
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
                preview: browser::Preview {
                    preview_thread: Some(std::thread::spawn(|| {})),
                },
                offset_y: 0.,
                dragging_audio: false,
                dragging_audio_text: String::new(),
                sidebar_width: 300.,
                started_drag: false,
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
                        kind: BrowserEntryKind::Directory { expanded: false },
                    });
                } else if [".wav", ".wave", ".mp3", ".ogg", ".flac", ".opus"]
                    .into_iter()
                    .any(|extension| {
                        entry
                            .unwrap()
                            .file_name()
                            .to_str()
                            .unwrap_or_default()
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
        CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let viewport: Rect = ctx
                    .input(|input_state| input_state.viewport().inner_rect)
                    .unwrap_or_else(|| {
                        let size = ctx.screen_rect().size();
                        Rect::from_min_size(Pos2::ZERO, size)
                    });

                ui.painter().rect_filled(
                    Rect::from_min_size(Pos2::ZERO, viewport.size()),
                    0.0,
                    Color32::from_hex("#1e222f").unwrap_or_default(),
                );

                visual::navbar::paint_navbar(ui, &viewport, &self.themes);

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
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Clean up any resources
        self.browser.entries.clear();

        // Log the exit
        println!("Volt is exiting!");

        // Perform any final saves or cleanup
        // For example, you might want to save user preferences or state
        // self.save_state();

        // Close any open connections or files
        // self.close_connections();

        // You can add more cleanup code here as needed
    }
}
