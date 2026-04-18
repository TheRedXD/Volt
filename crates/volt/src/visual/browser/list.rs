use std::{rc::Rc, sync::{Arc, RwLock, atomic::Ordering}};

use egui::{Color32, Frame, Image, Label, Margin, Response, ScrollArea, TextureOptions, Ui, Vec2, Widget};

use crate::{AppSignals, visual::{browser::{categories::Category, files::BrowserFiles}, theme::ThemeColors}};

pub struct BrowserList {
    min_width: Arc<RwLock<f32>>,
    theme: Rc<ThemeColors>,
    category: Arc<RwLock<Category>>,
    files: BrowserFiles,
}

impl BrowserList {
    pub fn new(
        min_width: Arc<RwLock<f32>>,
        theme: Rc<ThemeColors>,
        category: Arc<RwLock<Category>>,
        files: BrowserFiles
    ) -> BrowserList {
        BrowserList {
            min_width: min_width,
            theme: theme,
            category: category,
            files: files,
        }
    }

    pub fn draw(&mut self, ui: &mut Ui) {
        match *self.category.read().unwrap() {
            Category::Files => {
                self.files.ui(ui);
            },
            _ => {
                ui.label("other");
            }
        }
    }
}

impl Widget for &mut BrowserList {
    fn ui(self, ui: &mut Ui) -> Response {
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, egui::Sense::hover());
        let builder = egui::UiBuilder::new()
            .max_rect(rect);
        let response = ui.scope_builder(builder, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.spacing_mut().window_margin = Margin::ZERO;
            Frame{
                inner_margin: Margin::ZERO,
                outer_margin: Margin::ZERO,
                ..Default::default()
            }.show(ui, |ui| {
                self.draw(ui);
            }).response
        }).response;
        
        ui.allocate_rect(response.rect, egui::Sense::hover());
        
        response
    }
}