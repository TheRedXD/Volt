use blerp::utils::zip;
use egui::{Color32, ColorImage, hex_color};
use itertools::Itertools;

// Expose components
pub mod browser;
pub mod central;
pub mod dialog;
pub mod navbar;
pub mod notification;
pub mod palette;
pub mod status;
pub mod switch;

// Theming
#[derive(Debug, PartialEq, Eq)]
pub struct ThemeColors {
    pub navbar_background_gradient_top: Color32,
    pub navbar_background_gradient_bottom: Color32,
    pub navbar_outline: Color32,
    pub navbar_widget: Color32,
    pub notification_background: Color32,
    pub notification_border: Color32,
    pub central_background: Color32,
    pub browser: Color32,
    pub browser_outline: Color32,
    pub browser_selected_button_fg: Color32,
    pub browser_unselected_button_fg: Color32,
    pub browser_unselected_hover_button_fg: Color32,
    pub browser_invalid_name_bg: Color32,
    pub browser_unselected_hover_button_fg_invalid: Color32,
    pub browser_unselected_button_fg_invalid: Color32,
    pub browser_folder_text: Color32,
    pub browser_folder_hover_text: Color32,
    pub playlist_bar: Color32,
    pub playlist_beat: Color32,
    pub bg_text: Color32,
    pub command_palette: Color32,
    pub command_palette_border: Color32,
    pub command_palette_text: Color32,
    pub command_palette_placeholder_text: Color32,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            navbar_background_gradient_top: hex_color!("#1e2132"),
            navbar_background_gradient_bottom: hex_color!("#171825"),
            navbar_outline: hex_color!("#453f67"),
            navbar_widget: hex_color!("#07081580"),
            notification_background: hex_color!("#1d1b2b"),
            notification_border: hex_color!("#3d3b4b"),
            central_background: hex_color!("#171825"),
            browser: hex_color!("#171825"),
            browser_outline: hex_color!("#28243e"),
            browser_selected_button_fg: hex_color!("#ffcf7b"),
            browser_unselected_button_fg: hex_color!("#646d88"),
            browser_unselected_hover_button_fg: hex_color!("#8591b5"),
            browser_invalid_name_bg: hex_color!("#ff000010"),
            browser_unselected_button_fg_invalid: hex_color!("#a46d88"),
            browser_unselected_hover_button_fg_invalid: hex_color!("#f591b5"),
            browser_folder_text: hex_color!("#928ea7"),
            browser_folder_hover_text: hex_color!("#ece9ff"),
            playlist_bar: hex_color!("#5e5a75"),
            playlist_beat: hex_color!("#2e2b3f"),
            bg_text: hex_color!("#646987"),
            command_palette: hex_color!("#1d1b2b"),
            command_palette_border: hex_color!("#3d3b4b"),
            command_palette_text: hex_color!("#928ea7"),
            command_palette_placeholder_text: hex_color!("#928ea740"),
        }
    }
}

/// Create a vertical gradient of the specified height (in pixels).
pub fn build_gradient(height: usize, top: Color32, bottom: Color32) -> ColorImage {
    ColorImage::from_rgba_unmultiplied(
        [1, height],
        &(0..height)
            .flat_map(|y| {
                #[allow(clippy::cast_precision_loss, reason = "rounding errors are negligible because this is a visual effect")]
                let factor = y as f32 / (height - 1) as f32;
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "the `f32`s are within the `u8` range")]
                zip(top.to_array(), bottom.to_array()).map(|(a, b)| f32::from(a).mul_add(1.0 - factor, f32::from(b) * factor) as u8)
            })
            .collect_vec(),
    )
}
