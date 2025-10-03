use std::{cmp::Ordering, iter::repeat_n, rc::Rc, sync::mpsc::Sender, time::Duration};

use egui::{Align, Color32, FontFamily, FontId, Frame, Id, Key, KeyboardShortcut, Layout, Modifiers, Stroke, TextEdit, TextStyle, Ui, hex_color};
use strsim::damerau_levenshtein;

use crate::{
    info,
    visual::{
        ThemeColors,
        notification::{Notification, NotificationDrawer},
    },
};

pub struct Palette {
    pub showing: bool,
    pub text: String,
    pub theme: Rc<ThemeColors>,
    pub field_id: Id,
}

const COMMAND_PALETTE_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::P);

impl Palette {
    pub fn new(theme: Rc<ThemeColors>) -> Self {
        Self {
            showing: false,
            text: String::new(),
            theme,
            field_id: Id::new("command_palette_text_field"),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, timings_toggle: &mut bool, notifications_tx: &Sender<Notification>) {
        if ui.ctx().input_mut(|i| i.consume_shortcut(&COMMAND_PALETTE_SHORTCUT)) {
            self.text.clear();
            ui.ctx().memory_mut(|mem| mem.request_focus(self.field_id));
            self.showing = !self.showing;
        }

        let mut commands = [
            (
                "timings",
                &mut (|| {
                    *timings_toggle = !*timings_toggle;
                }) as &mut dyn FnMut(),
            ),
            ("info", &mut || {
                info::dump();
                notifications_tx
                    .send(Notification::new("Dumped system info into console!".into(), Some(Duration::from_secs(5))))
                    .unwrap();
            }),
            ("bug", &mut || {
                println!("!!!!!!\nWhen making your bug report, add the information below!\n!!!!!!");
                info::dump();
                notifications_tx
                    .send(Notification::new(
                        "Dumped system info into console! You'll be redirected to the official Volt bug report page in ~3 seconds.".into(),
                        Some(Duration::from_secs(5)),
                    ))
                    .unwrap();
                std::thread::spawn(|| {
                    std::thread::sleep(Duration::from_secs(3));
                    info::open_link(info::BUG_REPORT_URL);
                });
            }),
        ];

        commands.sort_unstable_by_key(|&(name, _)| {
            (
                usize::MAX - name.chars().zip(self.text.chars()).take_while(|(a, b)| a == b).count(),
                damerau_levenshtein(&self.text.to_lowercase(), name),
            )
        });

        if ui.ctx().input_mut(|i| i.key_pressed(Key::Enter)) {
            self.showing = false;
            commands[0].1();
        }

        if self.showing {
            Frame::new().inner_margin(10).show(ui, |ui| {
                Frame::new()
                    .inner_margin(10)
                    .corner_radius(8)
                    .fill(self.theme.command_palette)
                    .stroke((1., self.theme.command_palette_border))
                    .show(ui, |ui| {
                        ui.style_mut().visuals.text_cursor.stroke = Stroke::new(2., hex_color!("#5c5cff"));
                        let text_edit = TextEdit::singleline(&mut self.text)
                            .background_color(Color32::TRANSPARENT)
                            .frame(false)
                            .font(FontId::new(12., FontFamily::Monospace))
                            .id(self.field_id);
                        let output = text_edit.show(ui);
                        let text_color = Color32::from_rgba_premultiplied(100, 100, 100, 100);
                        ui.painter_at(output.response.rect).galley(
                            output.galley_pos,
                            ui.painter_at(output.response.rect).layout(
                                repeat_n(' ', self.text.len()).chain(commands[0].0.chars().skip(self.text.len())).collect::<String>(),
                                FontId::new(12., FontFamily::Monospace),
                                text_color,
                                f32::INFINITY,
                            ),
                            text_color,
                        );
                        ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                            for command in commands.iter_mut().take(5) {
                                if ui.button(command.0).clicked() {
                                    command.1();
                                    self.showing = false;
                                }
                            }
                        })
                    });
            });
        }
    }
}
