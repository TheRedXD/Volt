use std::process::Command;

fn main() {
    // automatic mold linker usage
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "linux" {
        if Command::new("mold").arg("--version").status().is_ok() {
            println!("cargo:rustc-link-arg=-fuse-ld=mold");
            println!("cargo:rustc-link-arg=-Wl,--no-rosegment");
        }
    }
}