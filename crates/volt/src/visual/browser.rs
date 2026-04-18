pub mod categories;
pub mod list;
pub mod files;

use std::{rc::Rc, sync::{Arc, RwLock}};

use egui::{Frame, Margin, Response, Ui, Widget};

use crate::{AppSignals, visual::{browser::{categories::{BrowserCategories, Category}, files::BrowserFiles, list::BrowserList}, theme::ThemeColors}};

pub struct Browser {
    pub min_width: Arc<RwLock<f32>>,
    theme: Rc<ThemeColors>,
    
    // Browser components
    categories: BrowserCategories,
    list: BrowserList,
}

impl Browser {
    pub fn new(theme: Rc<ThemeColors>) -> Browser {
        let min_width = Arc::new(RwLock::new(100.));
        
        let category = Arc::new(RwLock::new(Category::Files));
        
        Browser {
            min_width: Arc::clone(&min_width),
            theme: Rc::clone(&theme),
            
            categories: BrowserCategories::new(
                Arc::clone(&min_width),
                Rc::clone(&theme),
                Arc::clone(&category)
            ),
            list: BrowserList::new(
                Arc::clone(&min_width), 
                Rc::clone(&theme), 
                Arc::clone(&category), 
                BrowserFiles::new(Rc::clone(&theme), Arc::clone(&category))
            ),
        }
    }
    
    pub fn draw(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            self.categories.ui(ui);
            ui.add_space(4.);
            self.list.ui(ui);
        });
    }
}

impl Widget for &mut Browser {
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
