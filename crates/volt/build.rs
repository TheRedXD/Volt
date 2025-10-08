use proc_macro2::{Ident, Span};
use quote::quote;
use serde_json::{Map, Value, from_str};
use std::{
    env::var,
    fs::{read_dir, read_to_string, write},
    path::PathBuf,
};

include!("src/visual/theme.rs");

#[allow(clippy::many_single_char_names)]
fn main() {
    let themes_dir = PathBuf::from(var("CARGO_MANIFEST_DIR").unwrap()).join("src/themes");
    println!("cargo:rerun-if-changed={}", themes_dir.display());
    let themes = read_dir(themes_dir)
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().unwrap().is_file())
        .map(|entry| {
            let file = read_to_string(entry.path()).unwrap();
            let theme_json = from_str::<Map<String, Value>>(&file).unwrap();
            let name = entry.path();
            let name = Ident::new(&name.file_stem().unwrap().to_str().unwrap().to_uppercase(), Span::call_site());
            let colors = theme_json
                .iter()
                .filter_map(|(key, value)| {
                    let key = Ident::new(key, Span::call_site());
                    let [r, g, b, a] = Color32::from_hex(value.as_str()?).unwrap().to_array();
                    Some(quote! {#key: egui::Color32::from_rgba_premultiplied(#r, #g, #b, #a)})
                })
                .collect::<Vec<_>>();
            let shadow = theme_json.get("shadow").unwrap().as_object().unwrap();
            #[allow(unused_variables, reason = "used in the following `quote!`")]
            let offset: [i8; 2] = shadow
                .get("offset")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_number().unwrap().as_i64().unwrap().try_into().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let [x, y] = offset;
            let blur: u8 = shadow.get("blur").unwrap().as_number().unwrap().as_u64().unwrap().try_into().unwrap();
            let spread: u8 = shadow.get("spread").unwrap().as_number().unwrap().as_u64().unwrap().try_into().unwrap();
            let [r, g, b, a] = Color32::from_hex(shadow.get("color").unwrap().as_str().unwrap()).unwrap().to_array();
            quote! {
                pub const #name: crate::visual::theme::ThemeColors = crate::visual::theme::ThemeColors {
                    #(#colors),*,
                    shadow: egui::Shadow {
                        offset: [#x, #y],
                        blur: #blur,
                        spread: #spread,
                        color: egui::Color32::from_rgba_premultiplied(#r, #g, #b, #a),
                    },
                };
            }
        })
        .collect::<Vec<_>>();
    write(
        var("OUT_DIR").unwrap() + "/themes.rs",
        quote! {
            #(#themes),*
        }
        .to_string(),
    )
    .unwrap();
}
