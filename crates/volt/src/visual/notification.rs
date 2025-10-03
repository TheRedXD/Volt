use std::{
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

use egui::{Align, Layout, Vec2, hex_color};
use tap::Pipe;

#[derive(Debug, Clone)]
pub struct Notification {
    pub message: String,
    pub duration: Option<Duration>,
    pub added: Instant,
}

impl Notification {
    pub fn new(message: String, duration: Option<Duration>) -> Self {
        Self {
            message,
            duration,
            added: Instant::now(),
        }
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
    rx: Receiver<Notification>,
}

impl NotificationDrawer {
    pub const fn new(rx: Receiver<Notification>) -> Self {
        Self { notifications: Vec::new(), rx }
    }

    pub fn add_notification(&mut self, notification: Notification) {
        self.notifications.push(notification);
    }

    pub fn get_notifications(&self) -> &[Notification] {
        &self.notifications
    }

    pub fn notify(&mut self, message: String, duration: Option<Duration>) {
        self.add_notification(Notification::new(message, duration));
    }
}

impl egui::Widget for &mut NotificationDrawer {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.allocate_ui_with_layout(Vec2::ZERO, Layout::bottom_up(Align::RIGHT), |ui| {
            self.notifications.extend(self.rx.try_iter());
            self.notifications.retain(|notification| {
                let Some(opacity) = notification.duration.map_or(Some(1.), |duration| {
                    (notification.added + duration).checked_duration_since(Instant::now()).as_ref().map(Duration::as_secs_f32)
                }) else {
                    return false;
                };
                ui.set_opacity(opacity);
                egui::Frame::new().fill(hex_color!("#222222")).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                    ui.scope(|ui| {
                        ui.multiply_opacity(0.5);
                        ui.label(format!("{:?} ago", notification.added.elapsed().as_secs_f32().round().pipe(Duration::from_secs_f32)));
                    });
                    ui.label(&notification.message);
                });
                ui.ctx().request_repaint();
                true
            });
        })
        .response
    }
}
