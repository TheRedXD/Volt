pub mod macros {
    macro_rules! get_icon_image {
        ($path:expr) => {
            egui::Image::new(egui::include_image!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/images/icons/", $path)))
        };
    }
    
    pub(crate) use get_icon_image;
}