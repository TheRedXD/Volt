#![warn(clippy::pedantic, clippy::nursery, clippy::allow_attributes_without_reason, clippy::undocumented_unsafe_blocks, clippy::clone_on_ref_ptr)]
use std::{
    io::{BufReader, Cursor},
    rc::Rc,
    time::{Duration, Instant},
};

use eframe::{App, CreationContext, NativeOptions, egui, run_native};
use egui::{hex_color, Area, CentralPanel, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, IconData, Margin, Shadow, SidePanel, TextStyle, TopBottomPanel, Vec2, ViewportBuilder};
use egui_extras::install_image_loaders;
use human_panic::setup_panic;
use image::{ImageFormat, ImageReader};
use info::handle_args;
// TODO: Move everything into components (visual)
mod info;
mod timings;
mod visual;

use tap::{Pipe, Tap};
use visual::{ThemeColors, browser::Browser, central::Central, navbar::navbar, notification::NotificationDrawer, status::status};

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
    pub showing_command_palette: bool,
    pub command_palette_text: String,
    pub command_palette_cursor_pos: usize,
    pub command_palette_cursor_pos_end: usize,
    pub command_palette_begin: Instant,
    pub timings_toggle: bool,
    pub show_welcome: bool,
    pub show_about: bool,
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
        });
        let theme = Rc::new(ThemeColors::default());
        Self {
            browser: Browser::new(Rc::clone(&theme)),
            central: Central::new(),
            notification_drawer: NotificationDrawer::new(),
            theme,
            showing_command_palette: false,
            command_palette_text: String::new(),
            command_palette_begin: Instant::now(),
            command_palette_cursor_pos: 0,
            command_palette_cursor_pos_end: 0,
            timings_toggle: false,
            show_welcome: true,
            show_about: false,
        }
    }
}

impl App for VoltApp {
    #[allow(clippy::too_many_lines, reason = "shut")]
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        let time_render_start = Instant::now();
        // TODO: Move this (the command palette) to its own file. This is here primarily for testing purposes.

        // Keyboard shortcut handler
        if ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::P))) {
            if !self.showing_command_palette {
                self.command_palette_begin = Instant::now();
            }
            self.showing_command_palette = !self.showing_command_palette;
        }

        // Handle queries
        if ctx.input_mut(|i| i.key_pressed(egui::Key::Enter)) {
            self.showing_command_palette = false;
            // TODO: Replace this with a search query implementation rather than direct matching (after moving to palette.rs).
            match self.command_palette_text.as_str() {
                "timings" => {
                    self.timings_toggle = !self.timings_toggle;
                }
                "info" => {
                    info::dump();
                    self.notification_drawer.make("Dumped system info into console!".into(), Some(Duration::from_secs(5)));
                }
                "bug" => {
                    println!("!!!!!!\nWhen making your bug report, add the information below!\n!!!!!!");
                    info::dump();
                    self.notification_drawer.make(
                        "Dumped system info into console! You'll be redirected to the official Volt bug report page in ~3 seconds.".into(),
                        Some(Duration::from_secs(5)),
                    );
                    std::thread::spawn(|| {
                        std::thread::sleep(Duration::from_secs(3));
                        info::open_link(info::BUG_REPORT_URL);
                    });
                }
                _ => {}
            }
        }

        // Reset the command palette input
        if !self.showing_command_palette && !self.command_palette_text.is_empty() {
            self.command_palette_cursor_pos = 0;
            self.command_palette_cursor_pos_end = 0;
            self.command_palette_text.clear();
        }

        // Render the command palette and handle logic
        if self.showing_command_palette {
            // Escaping the command palette
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                self.showing_command_palette = false;
                ctx.request_repaint();
            } else {
                let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("command_palette")));
                let screen_rect = ctx.screen_rect();
                let palette_size = egui::vec2(300.0, 30.0);
                let mut center_top = screen_rect.center_top();
                center_top.y += 40.;
                let palette_rect = egui::Rect::from_center_size(center_top, palette_size);

                painter.add(
                    Shadow {
                        spread: 0,
                        blur: 14,
                        offset: [0, 4],
                        color: egui::Color32::from_black_alpha(200),
                    }
                    .as_shape(palette_rect, 8.0),
                );

                painter.rect_filled(palette_rect, 8.0, self.theme.command_palette);
                painter.rect_stroke(palette_rect, 8.0, (1.0, self.theme.command_palette_border), egui::StrokeKind::Middle);

                let palette_text_fontid = FontId::new(12., FontFamily::Monospace);
                #[allow(clippy::cast_precision_loss, reason = "shut")]
                #[allow(clippy::cast_possible_truncation, reason = "shut")]
                if let Some(text) = ctx.input_mut(|i| {
                    i.events.iter().find_map(|event| match event {
                        egui::Event::Text(text) => Some(text.clone()),
                        _ => None,
                    })
                }) {
                    if self.command_palette_cursor_pos == self.command_palette_cursor_pos_end {
                        self.command_palette_text.insert_str(self.command_palette_cursor_pos, &text);
                        self.command_palette_cursor_pos += 1;
                    } else {
                        let start = self.command_palette_cursor_pos.min(self.command_palette_cursor_pos_end);
                        let end = self.command_palette_cursor_pos.max(self.command_palette_cursor_pos_end);
                        self.command_palette_text.replace_range(start..end, &text);
                        self.command_palette_cursor_pos = start + 1;
                    }
                    self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    self.command_palette_begin = Instant::now();
                }

                if ctx.input_mut(|i| i.key_pressed(egui::Key::Backspace)) && !self.command_palette_text.is_empty() {
                    if self.command_palette_cursor_pos != self.command_palette_cursor_pos_end {
                        let start = self.command_palette_cursor_pos.min(self.command_palette_cursor_pos_end);
                        let end = self.command_palette_cursor_pos.max(self.command_palette_cursor_pos_end);
                        self.command_palette_text.replace_range(start..end, "");
                        self.command_palette_cursor_pos = start;
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    } else if self.command_palette_cursor_pos > 0 {
                        self.command_palette_text.remove(self.command_palette_cursor_pos - 1);
                        self.command_palette_cursor_pos -= 1;
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    }
                    self.command_palette_begin = Instant::now();
                }

                if ctx.input_mut(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                    if ctx.input_mut(|i| i.modifiers.shift) {
                        if self.command_palette_cursor_pos > 0 {
                            self.command_palette_cursor_pos -= 1;
                        }
                    } else if self.command_palette_cursor_pos > 0 {
                        self.command_palette_cursor_pos -= 1;
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    } else {
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    }
                    self.command_palette_begin = Instant::now();
                }

                if ctx.input_mut(|i| i.key_pressed(egui::Key::ArrowRight)) {
                    if ctx.input_mut(|i| i.modifiers.shift) {
                        if self.command_palette_cursor_pos < self.command_palette_text.len() {
                            self.command_palette_cursor_pos += 1;
                        }
                    } else if self.command_palette_cursor_pos < self.command_palette_text.len() {
                        self.command_palette_cursor_pos += 1;
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    } else {
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    }
                    self.command_palette_begin = Instant::now();
                }

                if ctx.input_mut(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowLeft)) {
                    let text_before = &self.command_palette_text[..self.command_palette_cursor_pos];
                    self.command_palette_cursor_pos = text_before.rfind(|c: char| !c.is_alphanumeric()).map_or(0, |i| i + 1);
                    if !ctx.input_mut(|i| i.modifiers.shift) {
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    }
                    self.command_palette_begin = Instant::now();
                }

                if ctx.input_mut(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::ArrowRight)) {
                    let text_after = &self.command_palette_text[self.command_palette_cursor_pos..];
                    if let Some(i) = text_after.find(|c: char| !c.is_alphanumeric()) {
                        self.command_palette_cursor_pos += i;
                    } else {
                        self.command_palette_cursor_pos = self.command_palette_text.len();
                    }
                    if !ctx.input_mut(|i| i.modifiers.shift) {
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    }
                    self.command_palette_begin = Instant::now();
                }

                if ctx.input_mut(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Backspace)) {
                    let text_before = &self.command_palette_text[..self.command_palette_cursor_pos];
                    let prev_word_end = text_before.rfind(|c: char| !c.is_alphanumeric()).map_or(0, |i| i + 1);
                    self.command_palette_text.drain(prev_word_end..self.command_palette_cursor_pos);
                    self.command_palette_cursor_pos = prev_word_end;
                    self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    self.command_palette_begin = Instant::now();
                }

                if ctx.input_mut(|i| i.key_pressed(egui::Key::Delete)) {
                    if self.command_palette_cursor_pos != self.command_palette_cursor_pos_end {
                        let start = self.command_palette_cursor_pos.min(self.command_palette_cursor_pos_end);
                        let end = self.command_palette_cursor_pos.max(self.command_palette_cursor_pos_end);
                        self.command_palette_text.replace_range(start..end, "");
                        self.command_palette_cursor_pos = start;
                        self.command_palette_cursor_pos_end = self.command_palette_cursor_pos;
                    } else if self.command_palette_cursor_pos < self.command_palette_text.len() {
                        self.command_palette_text.remove(self.command_palette_cursor_pos);
                    }
                    self.command_palette_begin = Instant::now();
                }

                if ctx.input_mut(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Delete)) && self.command_palette_cursor_pos < self.command_palette_text.len() {
                    let text_after = &self.command_palette_text[self.command_palette_cursor_pos..];
                    let next_word_start = text_after
                        .find(|c: char| !c.is_alphanumeric())
                        .map_or(self.command_palette_text.len(), |i| self.command_palette_cursor_pos + i);
                    self.command_palette_text.drain(self.command_palette_cursor_pos..next_word_start);
                    self.command_palette_begin = Instant::now();
                }

                if ctx.input_mut(|i| i.modifiers.shift && i.key_pressed(egui::Key::Delete)) {
                    self.command_palette_text.clear();
                    self.command_palette_cursor_pos = 0;
                    self.command_palette_cursor_pos_end = 0;
                    self.command_palette_begin = Instant::now();
                }

                if ctx.input_mut(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::A)) {
                    self.command_palette_cursor_pos = self.command_palette_text.len();
                    self.command_palette_cursor_pos_end = 0;
                    self.command_palette_begin = Instant::now();
                }

                let cptext_x_offset = 10.;
                let cursor_width = 2.;

                if self.command_palette_text.is_empty() {
                    painter.text(
                        {
                            let mut lc = palette_rect.left_center();
                            lc.x += cptext_x_offset;
                            lc
                        },
                        egui::Align2::LEFT_CENTER,
                        "Type a command...",
                        palette_text_fontid.clone(),
                        self.theme.command_palette_placeholder_text,
                    );
                    // Draw cursor
                    let cursor_pos = painter
                        .text(
                            {
                                let mut lc = palette_rect.left_center();
                                lc.x += cptext_x_offset;
                                lc
                            },
                            egui::Align2::LEFT_CENTER,
                            &self.command_palette_text[..self.command_palette_cursor_pos],
                            palette_text_fontid,
                            self.theme.command_palette_text,
                        )
                        .right();
                    // Only show cursor every 500ms
                    if self.command_palette_begin.elapsed().as_secs_f32().fract() < 0.5 {
                        painter.rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(cursor_pos, palette_rect.center().y - 8.),
                                egui::pos2(cursor_pos + cursor_width, palette_rect.center().y + 8.),
                            ),
                            0.0,
                            egui::Color32::from_rgb(0x5c, 0x5c, 0xff),
                        );
                    }
                } else {
                    let (start_pos, end_pos) = if self.command_palette_cursor_pos < self.command_palette_cursor_pos_end {
                        (self.command_palette_cursor_pos, self.command_palette_cursor_pos_end)
                    } else {
                        (self.command_palette_cursor_pos_end, self.command_palette_cursor_pos)
                    };

                    // Draw text before selection
                    let selection_start = painter
                        .text(
                            {
                                let mut lc = palette_rect.left_center();
                                lc.x += cptext_x_offset;
                                lc
                            },
                            egui::Align2::LEFT_CENTER,
                            &self.command_palette_text[..start_pos],
                            palette_text_fontid.clone(),
                            self.theme.command_palette_text,
                        )
                        .right();

                    // Draw selection
                    let selection_end = painter
                        .text(
                            egui::pos2(selection_start, palette_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &self.command_palette_text[start_pos..end_pos],
                            palette_text_fontid.clone(),
                            hex_color!("8c8cff"),
                        )
                        .right();

                    painter.rect_filled(
                        egui::Rect::from_min_max(egui::pos2(selection_start, palette_rect.center().y - 8.), egui::pos2(selection_end, palette_rect.center().y + 8.)),
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(0x5c, 0x5c, 0xff, 0x20),
                    );

                    // Draw text after selection
                    painter.text(
                        egui::pos2(selection_end, palette_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &self.command_palette_text[end_pos..],
                        palette_text_fontid,
                        self.theme.command_palette_text,
                    );

                    // Only show cursor every 500ms
                    if self.command_palette_begin.elapsed().as_secs_f32().fract() < 0.5 {
                        let cursor_pos = if self.command_palette_cursor_pos <= self.command_palette_cursor_pos_end {
                            selection_start
                        } else {
                            selection_end
                        };

                        painter.rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(cursor_pos, palette_rect.center().y - 8.),
                                egui::pos2(cursor_pos + cursor_width, palette_rect.center().y + 8.),
                            ),
                            0.0,
                            egui::Color32::from_rgb(0x5c, 0x5c, 0xff),
                        );
                    }
                }

                ctx.request_repaint_after_secs(0.1);
            }
        }

        Area::new("center_area".into()).anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(ctx, |ui| {
            if self.show_welcome {
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
                                let close_btn = egui::Button::new("Ok").fill(self.theme.command_palette).stroke(egui::Stroke::new(1., self.theme.playlist_bar));
                                if ui.add(close_btn).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                    self.show_welcome = false;
                                }
                            });
                        });
                    });
            }
        });

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
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::Vec2::new(ctx.screen_rect().max.x, ctx.screen_rect().max.y))
            .show(ctx, |ui| {
                egui::Frame {
                    inner_margin: egui::Margin::same(0),
                    outer_margin: egui::Margin::same(0),
                    corner_radius: egui::CornerRadius::same(0),
                    shadow: Shadow::NONE,
                    fill: egui::Color32::TRANSPARENT,
                    stroke: egui::Stroke::NONE,
                }
                .show(ui, |ui| {
                    ui.add(&mut self.notification_drawer);
                });
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
