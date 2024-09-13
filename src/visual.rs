use egui::Color32;

// Expose components
pub mod switch;
pub mod navbar;

// Theming
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeColors {
    pub navbar: Color32,
    pub navbar_outline: Color32,
    pub browser: Color32,
    pub browser_outline: Color32,
    pub browser_selected_button_fg: Color32,
    pub browser_unselected_button_fg: Color32,
    pub browser_unselected_hover_button_fg: Color32,
    pub bg_text: Color32,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            navbar: Color32::from_hex("#262b3b").unwrap_or_default(),
            navbar_outline: Color32::from_hex("#00000080").unwrap_or_default(),
            browser: Color32::from_hex("#242938").unwrap_or_default(),
            browser_outline: Color32::from_hex("#00000080").unwrap_or_default(),
            browser_selected_button_fg: Color32::from_hex("#ffcf7b").unwrap_or_default(),
            browser_unselected_button_fg: Color32::from_hex("#646d88").unwrap_or_default(),
            browser_unselected_hover_button_fg: Color32::from_hex("#8591b5").unwrap_or_default(),
            bg_text: Color32::from_hex("#646987").unwrap_or_default(),
        }
    }
}