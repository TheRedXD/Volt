use blerp::utils::zip;
use egui::{Color32, ColorImage};
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
pub mod theme;

impl Default for theme::ThemeColors {
    #[allow(clippy::cognitive_complexity, reason = "it is just colors")]
    fn default() -> Self {
        builtin_themes::DEFAULT
    }
}

pub mod builtin_themes {
    include!(concat!(env!("OUT_DIR"), "/themes.rs"));
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
