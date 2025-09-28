use std::time::{Duration, Instant};

use egui::{Color32, hex_color};

#[derive(Debug, Clone)]
pub struct Notification {
    pub message: String,
    pub duration: Option<Duration>,
    pub added: Instant,
}

impl Notification {
    pub fn new(message: String, duration: Option<Duration>) -> Self {
        let add_time = Instant::now();
        Self { message, duration, added: add_time }
    }

    pub fn with_duration(message: String, duration: Duration) -> Self {
        Self::new(message, Some(duration))
    }

    pub fn without_duration(message: String) -> Self {
        Self::new(message, None)
    }
}

pub struct NotificationDrawer {
    notifications: Vec<Notification>,
}

impl Default for NotificationDrawer {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationDrawer {
    pub const fn new() -> Self {
        Self { notifications: Vec::new() }
    }

    pub fn add_notification(&mut self, notification: Notification) {
        self.notifications.push(notification);
    }

    pub fn remove_notification(&mut self, index: usize) {
        if index < self.notifications.len() {
            self.notifications.remove(index);
        }
    }

    pub const fn get_notifications(&self) -> &Vec<Notification> {
        &self.notifications
    }

    pub fn make(&mut self, message: String, duration: Option<Duration>) {
        let notification = Notification::new(message, duration);
        self.add_notification(notification);
    }
}

impl egui::Widget for &mut NotificationDrawer {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        if !self.notifications.is_empty() {
            let now = Instant::now();
            let mut indices_to_remove = Vec::new();

            for (i, notification) in self.notifications.iter().enumerate() {
                let age = notification.added.elapsed();
                let fade_duration = Duration::from_secs_f32(0.2);
                let mut opacity = if age <= fade_duration {
                    age.as_secs_f32() / fade_duration.as_secs_f32()
                } else if let Some(lifetime) = notification.duration
                    && lifetime.checked_sub(age).unwrap_or_default() <= fade_duration
                {
                    lifetime.checked_sub(age).unwrap_or_default().as_secs_f32() / fade_duration.as_secs_f32()
                } else {
                    1.0
                };

                if opacity <= 0.0 {
                    opacity = 0.01;
                }

                let color = hex_color!("#222222").gamma_multiply(opacity);

                egui::Frame::new().fill(color).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                    ui.set_min_width(ui.ctx().screen_rect().width().min(200.));
                    ui.allocate_ui(ui.available_size(), |ui| {
                        let text_color = Color32::WHITE.gamma_multiply(opacity);
                        ui.label(egui::RichText::new(&notification.message).color(text_color));
                    });
                });

                // Schedule removal if a duration is specified
                if let Some(duration) = notification.duration
                    && notification.added + duration < now
                {
                    indices_to_remove.push(i);
                }

                ui.ctx().request_repaint_after_secs(0.03);
            }

            // Remove notifications in reverse order to avoid index invalidation
            for index in indices_to_remove.into_iter().rev() {
                self.remove_notification(index);
            }

            ui.allocate_response(ui.available_size(), egui::Sense::hover());
        }

        ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
    }
}
