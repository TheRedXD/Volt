#![warn(clippy::pedantic, clippy::nursery, clippy::allow_attributes_without_reason, clippy::undocumented_unsafe_blocks, clippy::clone_on_ref_ptr)]
use eframe::{App, CreationContext, NativeOptions, egui, run_native};
use egui::{
    Align2, Area, CentralPanel, Color32, Context, CursorIcon, FontData, FontDefinitions, FontFamily, FontId, Frame, IconData, Label, Modifiers, Popup, RichText, SidePanel, Stroke, TextStyle, TopBottomPanel, Vec2, ViewportBuilder, hex_color
};
use egui_extras::install_image_loaders;
use human_panic::setup_panic;
use image::{ImageFormat, ImageReader};
use info::handle_args;
use std::{
    io::{BufReader, Cursor},
    rc::Rc,
    sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc::{Sender, channel}},
    time::Instant,
};
use tap::{Pipe, Tap};
use visual::{
    browser::Browser,
    central::Central,
    navbar::navbar,
    notification::NotificationDrawer,
    status::status,
};

use crate::visual::{dialog::dialog, icons::macros::get_icon_image, popups::{about::render_about, settings::render_settings}, theme::ThemeColors};
use crate::visual::notification::Notification;
use crate::visual::palette::Palette;
use volt_waveform;

mod audio;
mod info;
mod shortcuts;
mod timings;
mod visual;

pub struct AppSignals {
    should_restart: AtomicBool,
    use_glow: AtomicBool,
    greeter: AtomicBool,
}

fn load_icon() -> egui::IconData {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(include_bytes!("./images/icons/app-icon.png"))
            .expect("Failed to open icon path")
            .into_rgba8();
        
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };

    egui::IconData {
        rgba: icon_rgba,
        width: icon_width,
        height: icon_height,
    }
}

fn main() -> eframe::Result {
    setup_panic!();
    if handle_args().is_break() {
        return Ok(());
    }
    
    let app_signals = Arc::new(AppSignals {
        should_restart: AtomicBool::new(false),
        use_glow: AtomicBool::new(false),
        greeter: AtomicBool::new(true),
    });
    
    loop {
        app_signals.should_restart.store(false, Ordering::SeqCst);
        
        let native_options = NativeOptions {
            vsync: true,
            renderer: match app_signals.use_glow.load(Ordering::SeqCst) {
                true => eframe::Renderer::Glow,
                false => eframe::Renderer::Wgpu,
            },
            wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
                present_mode: eframe::wgpu::PresentMode::Immediate,
                ..Default::default()
            },
            viewport: ViewportBuilder::default()
                .with_drag_and_drop(true)
                .with_app_id("sh.thered.Volt")
                .with_icon(load_icon()),
            ..Default::default()
        };
        
        let result = run_native(
            "Volt",
            native_options,
            Box::new(|cc| Ok(Box::new(VoltApp::new(cc, app_signals.clone())))),
        );
        
        if app_signals.should_restart.load(Ordering::SeqCst) {
            // TODO: replace with proper log
            println!("Restarting app...");
            continue;
        }
        
        return result;
    }
}

struct VoltApp {
    pub browser: Browser,
    pub central: Central,
    pub notification_drawer: NotificationDrawer,
    pub theme: Rc<ThemeColors>,
    pub palette: Palette,
    pub notifications_tx: Sender<Notification>,
    pub app_signals: Arc<AppSignals>,
}

impl VoltApp {
    fn new(cc: &CreationContext<'_>, app_signals: Arc<AppSignals>) -> Self {
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
            style.visuals.widgets.inactive.bg_stroke = Stroke::new(1., theme.playlist_bar);
            style.visuals.widgets.inactive.weak_bg_fill = theme.command_palette;
            style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, theme.playlist_bar);
        });
        let theme = Rc::new(ThemeColors::default());
        Popup::open_id(&cc.egui_ctx, "welcome".into());
        let (tx, rx) = channel();

        Self {
            browser: Browser::new(Rc::clone(&theme)),
            central: Central::new(Rc::clone(&theme)),
            notification_drawer: NotificationDrawer::new(rx, Rc::clone(&theme)),
            palette: Palette::new(Rc::clone(&theme)),
            theme,
            notifications_tx: tx,
            app_signals: app_signals,
        }
    }
}

impl App for VoltApp {
    #[allow(clippy::too_many_lines, reason = "shut")]
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().minimized.unwrap_or(false)) {
            return;
        }
        
        // if self.app_signals.greeter.load(Ordering::SeqCst) {
        //     CentralPanel::default().frame(egui::Frame::default().fill(self.theme.central_background)).show(ctx, |ui| {
        //         ui.with_layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight), |ui| {
        //             ui.horizontal(|ui| {
        //                 ui.add(
        //                     get_icon_image!("navbar-icon.svg")
        //                         .fit_to_exact_size(Vec2::new(128., 128.))
        //                 );
        //                 ui.vertical(|ui| {
        //                     ui.label("testing");
        //                     ui.label("testing");
        //                     ui.label("testing");
        //                 });
        //             });
        //         })
        //     });
        //     return;
        // }
        
        let time_render_start = Instant::now();
        dialog(ctx, &self.theme, |ui| {
            ui.label("Welcome to Volt!");
            ui.label("This is extremely work-in-progress and is not finished at all!");
            ui.label("If you can, please check out our GitHub repository:");
            ui.hyperlink_to("github.com/TheRedXD/Volt", "https://github.com/TheRedXD/Volt");
            ui.add_space(5.);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.style_mut().spacing.button_padding = Vec2 { x: 12., y: 4. };
                ui.style_mut().visuals.widgets.hovered.weak_bg_fill = hex_color!("#ffffff10");
                ui.style_mut().visuals.widgets.active.weak_bg_fill = hex_color!("#ffffff20");
                if ui.add(egui::Button::new("Ok").corner_radius(10.)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    ui.close();
                }
            });
        });
        
        render_about(ctx, &self.theme);
        render_settings(ctx, &self.theme);
        
        TopBottomPanel::top("navbar").frame(egui::Frame::default()).show_separator_line(false).show(ctx, |ui| {
            ui.add(navbar(&self.theme, &mut self.central));
        });
        TopBottomPanel::bottom("status").frame(egui::Frame::default()).show_separator_line(false).show(ctx, |ui| {
            ui.add(status(&self.theme, &mut self.central.mode));
        });

        let browser_id = egui::Id::new("browser");
        if ctx.memory_mut(|mem| *mem.data.get_temp_mut_or(browser_id, true)) {
            let min_width = *self.browser.min_width.read().unwrap();
            SidePanel::left(browser_id)
                .min_width(min_width)
                .default_width(300.)
                .frame(egui::Frame::default().fill(self.theme.browser))
                .show_separator_line(true)
                .show(ctx, |ui| {
                    ui.add(&mut self.browser);
                });
        }
        let ctrl_b_pressed = ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut {
                modifiers: Modifiers { ctrl: true, ..Default::default() },
                logical_key: egui::Key::B,
            })
        });
        if ctrl_b_pressed {
            ctx.memory_mut(|mem| *mem.data.get_temp_mut_or(browser_id, true) ^= true);
            ctx.request_repaint();
        }
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
                    self.palette.ui(ui, &self.notifications_tx);
                })
            });

        timings::set_render_time(time_render_start.elapsed());

        if ctx.memory_mut(|mem| *mem.data.get_temp_mut_or_default("timings".into())) {
            timings::show_timings(ctx, "Timings");
        }
        
        if self.app_signals.should_restart.load(Ordering::SeqCst) {
            Area::new("restart_overlay".into())
                .fixed_pos(ctx.screen_rect().left_top())
                .order(egui::Order::Foreground)
                .interactable(false)
                .show(ctx, |ui| {
                    let screen_rect = ctx.screen_rect();
                    ui.painter().rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(180));

                    let text = "Restarting...";
                    let font_id = FontId::new(24.0, FontFamily::Proportional);
                    let text_color = egui::Color32::WHITE;
                    ui.painter().text(
                        screen_rect.center(),
                        Align2::CENTER_CENTER,
                        text,
                        font_id,
                        text_color,
                    );

                    ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                });
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
