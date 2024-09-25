use eframe::{egui, run_native, App, CreationContext, NativeOptions};
use egui::{
    Align2, CentralPanel, Color32, Context, FontData, FontDefinitions, FontFamily, FontId, Pos2,
    Rect,
};
use egui_extras::install_image_loaders;
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use volt::blerp::device::load_system_devices;
use volt::browser::{Browser, BrowserCategory, BrowserEntry, Preview};
use volt::info::{output_info, panic_handler};
use volt::visual::{self, ThemeColors};
use volt::Result;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--info".to_string()) {
        return output_info();
    }

    // Panic handling
    std::panic::set_hook(Box::new(|panic_info| {
        panic_handler(panic_info);
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
    )?;

    Ok(())
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
                "../fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf"
            )),
        );
        fonts.families.insert(
            FontFamily::Name("IBMPlexMono".into()),
            vec!["IBMPlexMono".to_owned()],
        );
        cc.egui_ctx.set_fonts(fonts);
        let (tx, rx) = std::sync::mpsc::channel::<PathBuf>();
        let mut previewer = Preview::new(rx);

        std::thread::spawn(move || previewer.start_sample_loop());

        Self {
            browser: Browser {
                entries: Default::default(),
                selected_category: BrowserCategory::Files,
                path: dirs_next::document_dir().unwrap_or_default(),
                preview_tx: tx,
                offset_y: 0.,
                began_scroll: false,
                dragging_audio: false,
                dragging_audio_text: "".into(),
                sidebar_width: 300.,
                started_drag: false,
                devices: Default::default(),
            },
            themes: ThemeColors::default(),
        }
    }
}

fn load_entries<P: AsRef<Path>>(path: P, browser_entries: &mut Vec<BrowserEntry>) {
    browser_entries.clear();

    if let Ok(entries) = read_dir(path) {
        entries
            .flatten()
            .for_each(|entry| browser_entries.push(BrowserEntry::from(entry.path())));
    }
}

impl App for VoltApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        load_entries(&self.browser.path, &mut self.browser.entries);
        load_system_devices(&mut self.browser.devices);

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
