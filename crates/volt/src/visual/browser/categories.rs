use std::{rc::Rc, sync::{Arc, RwLock}};

use egui::{Color32, Frame, Margin, Rect, Response, Sense, Stroke, Ui, Vec2, Widget};

use crate::visual::{icons, theme::ThemeColors};

#[derive(PartialEq, Clone)]
pub enum Category {
    Files,
    Plugins
}

pub struct BrowserCategories {
    min_width: Arc<RwLock<f32>>,
    theme: Rc<ThemeColors>,
    category: Arc<RwLock<Category>>,
}

impl BrowserCategories {
    pub fn new(min_width: Arc<RwLock<f32>>, theme: Rc<ThemeColors>, category: Arc<RwLock<Category>>) -> BrowserCategories {
        BrowserCategories {
            min_width: min_width,
            theme: theme,
            category: category,
        }
    }
    
    pub fn button_files(&mut self, ui: &mut Ui) {
        let color = if *self.category.read().unwrap() == Category::Files {
            self.theme.browser_selected_button_fg
        } else {
            self.theme.browser_unselected_button_fg
        };
        
        let frame = Frame{
            inner_margin: Margin::symmetric(8, 4),
            outer_margin: Margin::ZERO,
            corner_radius: egui::CornerRadius::same(4),
            fill: Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 20),
            stroke: Stroke::new(1., color),
            ..Default::default()
        }.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                ui.add(icons::macros::get_icon_image!("browser/files.svg").tint(color));
                ui.add_space(4.);
                ui.add(egui::Label::new("Files").selectable(false));
            });
        });
        
        let response = ui.interact(
            frame.response.rect,
            ui.next_auto_id(),
            Sense::click()
        ).on_hover_cursor(egui::CursorIcon::PointingHand);
        
        if response.hovered() {
            ui.painter().rect_filled(
                response.rect,
                4.0,
                Color32::WHITE.linear_multiply(0.05)
            );
        }
        
        if response.clicked() {
            *self.category.write().unwrap() = Category::Files;
        }
    }
    
    pub fn button_plugins(&mut self, ui: &mut Ui) {
        let color = if *self.category.read().unwrap() == Category::Plugins {
            self.theme.browser_selected_button_fg
        } else {
            self.theme.browser_unselected_button_fg
        };
        
        let frame = Frame{
            inner_margin: Margin::symmetric(8, 4),
            outer_margin: Margin::ZERO,
            corner_radius: egui::CornerRadius::same(4),
            fill: Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 20),
            stroke: Stroke::new(1., color),
            ..Default::default()
        }.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                ui.add(icons::macros::get_icon_image!("browser/plugins.svg").tint(color));
                ui.add_space(4.);
                ui.add(egui::Label::new("Plugins").selectable(false));
            });
        });
        
        let response = ui.interact(
            frame.response.rect,
            ui.next_auto_id(),
            Sense::click()
        ).on_hover_cursor(egui::CursorIcon::PointingHand);
        
        if response.hovered() {
            ui.painter().rect_filled(
                response.rect,
                4.0,
                Color32::WHITE.linear_multiply(0.05)
            );
        }
        
        if response.clicked() {
            *self.category.write().unwrap() = Category::Plugins;
        }
    }
    
    pub fn draw(&mut self, ui: &mut Ui) {
        let response = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.add_space(5.);
            self.button_files(ui);
            ui.add_space(4.);
            self.button_plugins(ui);
            ui.add_space(5.);
        }).response;
        *self.min_width.write().unwrap() = f32::max(100., response.rect.width());
    }
}

impl Widget for &mut BrowserCategories {
    fn ui(self, ui: &mut Ui) -> Response {
        Frame{
            inner_margin: Margin::ZERO,
            outer_margin: Margin::ZERO,
            ..Default::default()
        }.show(ui, |ui| {
            self.draw(ui);
        }).response
    }
}