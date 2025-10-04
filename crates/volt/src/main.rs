#![warn(clippy::pedantic, clippy::nursery, clippy::allow_attributes_without_reason, clippy::undocumented_unsafe_blocks, clippy::clone_on_ref_ptr)]
use std::{
    io::{BufReader, Cursor},
    rc::Rc,
    sync::mpsc::{Sender, channel},
    time::Instant,
};

use eframe::{App, CreationContext, NativeOptions, egui, run_native};
use egui::{
    Align2, Area, CentralPanel, Context, CornerRadius, CursorIcon, FontData, FontDefinitions, FontFamily, FontId, IconData, Margin, SidePanel, TextStyle, TopBottomPanel, Vec2, ViewportBuilder,
};
use egui_extras::install_image_loaders;
use human_panic::setup_panic;
use image::{ImageFormat, ImageReader};
use info::handle_args;

mod info;
mod timings;
mod visual;

use tap::{Pipe, Tap};
use visual::{ThemeColors, browser::Browser, central::Central, navbar::navbar, notification::NotificationDrawer, palette::Palette, status::status};

use crate::visual::notification::Notification;

fn main() -> eframe::Result {
    setup_panic!();
    if handle_args().is_break() {
        return Ok(());
    }
    run_native(
        "Volt",
        NativeOptions {
            vsync: true,
            wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
                present_mode: eframe::wgpu::PresentMode::Immediate,
                ..Default::default()
            },
            viewport: ViewportBuilder::default().with_drag_and_drop(true).with_icon(
                ImageReader::new(BufReader::new(Cursor::new(include_bytes!("images/icons/icon.png").as_ref())))
                    .tap_mut(|reader| reader.set_format(ImageFormat::Png))
                    .decode()
                    .unwrap()
                    .pipe(|image| IconData {
                        rgba: image.to_rgb8().into_raw(),
                        height: image.height(),
                        width: image.width(),
                    }),
            ),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(VoltApp::new(cc)))),
    )
}

struct VoltApp {
    pub browser: Browser,
    pub central: Central,
    pub notification_drawer: NotificationDrawer,
    pub theme: Rc<ThemeColors>,
    pub timings_toggle: bool,
    pub show_welcome: bool,
    pub palette: Palette,
    pub notifications_tx: Sender<Notification>,
}

impl VoltApp {
    fn new(cc: &CreationContext<'_>) -> Self {
        const MONO_FONT_NAME: &str = "IBMPlexMono";
        const PROP_FONT_NAME: &str = "Inter";
        install_image_loaders(&cc.egui_ctx);
        cc.egui_ctx.set_fonts({
            let mut fonts = FontDefinitions::default();
            fonts
                .font_data
                .insert(MONO_FONT_NAME.to_string(), FontData::from_static(include_bytes!("fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf")).into());
            fonts.families.insert(FontFamily::Monospace, vec![MONO_FONT_NAME.to_string()]);
            fonts
                .font_data
                .insert(PROP_FONT_NAME.to_string(), FontData::from_static(include_bytes!("fonts/inter/Inter.ttf")).into());
            fonts.families.insert(FontFamily::Proportional, vec![PROP_FONT_NAME.to_string()]);
            fonts
        });
        let theme = Rc::new(ThemeColors::default());
        cc.egui_ctx.all_styles_mut(|style| {
            const BODY_TEXT_SIZE: f32 = 12.;
            let id = FontId::new(BODY_TEXT_SIZE, FontFamily::Proportional);
            style.override_font_id = Some(id);
            style.text_styles = [
                (TextStyle::Heading, BODY_TEXT_SIZE * 1.5),
                (TextStyle::Body, BODY_TEXT_SIZE),
                (TextStyle::Button, BODY_TEXT_SIZE),
                (TextStyle::Small, BODY_TEXT_SIZE * 0.8),
                (TextStyle::Monospace, BODY_TEXT_SIZE),
            ]
            .map(|(text_style, size)| (text_style, FontId::new(size, FontFamily::Proportional)))
            .into();
            style.visuals.interact_cursor = Some(CursorIcon::PointingHand);
            style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1., theme.playlist_bar);
            style.visuals.widgets.inactive.weak_bg_fill = theme.command_palette;
        });
        let (tx, rx) = channel();
        Self {
            browser: Browser::new(Rc::clone(&theme)),
            central: Central::new(),
            notification_drawer: NotificationDrawer::new(rx, Rc::clone(&theme)),
            timings_toggle: false,
            show_welcome: true,
            palette: Palette::new(Rc::clone(&theme)),
            theme,
            notifications_tx: tx,
        }
    }
}

impl App for VoltApp {
    #[allow(clippy::too_many_lines, reason = "shut")]
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        let time_render_start = Instant::now();
        if self.show_welcome {
            Area::new("center_area".into()).anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(ctx, |ui| {
                egui::Frame::new()
                    .fill(self.theme.central_background)
                    .stroke(egui::Stroke::new(1., self.theme.playlist_bar))
                    .corner_radius(CornerRadius::ZERO.at_least(5))
                    .inner_margin(Margin::same(10))
                    .show(ui, |ui| {
                        ui.label("Welcome to Volt!");
                        ui.label("This is extremely work-in-progress and is not finished at all!");
                        ui.label("If you can, please check out our GitHub repository:");
                        ui.hyperlink_to("github.com/TheRedXD/Volt", "https://github.com/TheRedXD/Volt");
                        ui.style_mut().spacing.item_spacing = Vec2::ZERO;
                        let mut margin = Margin::ZERO;
                        margin.top = 5;
                        egui::Frame::new().inner_margin(margin).show(ui, |ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let close_btn = egui::Button::new("Ok");
                                if ui.add(close_btn).clicked() {
                                    self.show_welcome = false;
                                }
                            });
                        });
                    });
            });
        }

        TopBottomPanel::top("navbar").frame(egui::Frame::default()).show_separator_line(false).show(ctx, |ui| {
            ui.add(navbar(&self.theme));
        });
        TopBottomPanel::bottom("status").frame(egui::Frame::default()).show_separator_line(false).show(ctx, |ui| {
            ui.add(status(&self.theme));
        });
        SidePanel::left("browser")
            .default_width(300.)
            .frame(egui::Frame::default().fill(self.theme.browser))
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.add(&mut self.browser);
            });
        CentralPanel::default().frame(egui::Frame::default().fill(self.theme.central_background)).show(ctx, |ui| {
            ui.add(&mut self.central);
        });

        Area::new("notifications_area".into())
            .interactable(false)
            .pivot(Align2::RIGHT_BOTTOM)
            .fixed_pos(ctx.screen_rect().right_bottom())
            .default_size(Vec2::ZERO)
            .show(ctx, |ui| {
                ui.add(&mut self.notification_drawer);
            });

        Area::new("command_palette".into())
            .pivot(Align2::CENTER_TOP)
            .default_pos(ctx.screen_rect().center_top())
            .show(ctx, |ui| {
                ui.allocate_ui(Vec2::X * ctx.screen_rect().width() * 0.5, |ui| {
                    self.palette.ui(ui, &mut self.timings_toggle, &self.notifications_tx);
                })
            });

        timings::set_render_time(time_render_start.elapsed());

        if self.timings_toggle {
            timings::show_timings(ctx, "Timings");
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
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
