use std::io::BufRead;
use std::panic::PanicHookInfo;

#[cfg(target_os = "linux")]
fn get_cpu_info() -> Option<String> {
    if let Ok(contents) = std::fs::read_to_string("/proc/cpuinfo") {
        if let Some(line) = contents.lines().find(|line| line.starts_with("model name")) {
            if let Some(cpu) = line.split(':').nth(1) {
                return cpu.trim().to_string();
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn get_cpu_info() -> Option<String> {
    use std::process::Command;

    Command::new("wmic")
        .args(["cpu", "get", "name"])
        .output()
        .ok()
        .and_then(|out| out.stdout.lines().nth(1))?
        .map(|r| r.trim().to_owned())
        .ok()
}

#[cfg(target_os = "macos")]
fn get_cpu_info() -> Option<String> {
    use std::process::Command;
    if let Ok(output) = Command::new("sysctl")
        .arg("-n")
        .arg("machdep.cpu.brand_string")
        .output()
    {
        if let Ok(cpu) = String::from_utf8(output.stdout) {
            return cpu.trim().to_string();
        }
    }
}

#[cfg(target_os = "linux")]
fn get_compositor() -> Option<String> {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return Some("Wayland".to_owned());
    }

    Some("X11".to_owned())
}

#[cfg(target_os = "windows")]
fn get_compositor() -> Option<String> {
    None
}

#[cfg(target_os = "linux")]
fn get_desktop_environment() -> Option<String> {
    std::env::var("XDG_CURRENT_DESKTOP").ok()
}

#[cfg(target_os = "windows")]
fn get_desktop_environment() -> Option<String> {
    None
}

#[cfg(target_os = "linux")]
fn get_gpu_info() -> Option<String> {
    std::process::Command::new("lspci")
        .output()
        .ok()
        .and_then(|out| {
            out.stdout
                .lines()
                .flatten()
                .find(|line| line.contains("VGA") || line.contains("3D"))?
                .split(':')
                .nth(2)
                .map(|r| r.trim().to_string())
        })
}

#[cfg(target_os = "windows")]
fn get_gpu_info() -> Option<String> {
    std::process::Command::new("wmic")
        .args(["path", "win32_VideoController", "get", "name"])
        .output()
        .ok()
        .and_then(|out| out.stdout.lines().nth(1))?
        .map(|r| r.trim().to_owned())
        .ok()
}

#[cfg(target_os = "macos")]
fn get_gpu_info() -> Option<String> {
    std::process::Command::new("system_profiler")
        .arg("SPDisplaysDataType")
        .output()
        .ok()
        .and_then(|out| {
            out.stdout
                .lines()
                .flatten()
                .find(|line| line.contains("Chipset Model:"))?
                .split_once(':')
                .map(|(_, r)| r.trim().to_string())
        })
}

#[cfg(not(target_os = "windows"))]
fn print_distro() -> crate::Result<()> {
    let release_file = std::fs::read_to_string("/etc/os-release")?;
    if let Some(distro) = release_file
        .lines()
        .find(|l| l.starts_with("PRETTY_NAME="))
        .and_then(|l| l.split('=').nth(1))
        .map(|l| l.trim_matches('"'))
    {
        println!("OS Distribution: {}", distro);
    }
    Ok(())
}

pub fn output_info() -> crate::Result<()> {
    println!("OS: {}", std::env::consts::OS);
    println!(
        "Desktop Environment: {}",
        get_desktop_environment().as_deref().unwrap_or("None")
    );
    println!(
        "Compositor: {}",
        get_compositor().as_deref().unwrap_or("None")
    );
    println!("CPU: {}", get_cpu_info().as_deref().unwrap_or("None"));
    println!("GPU: {}", get_gpu_info().as_deref().unwrap_or("None"));

    println!("OS Family: {}", std::env::consts::FAMILY);
    #[cfg(not(target_os = "windows"))]
    print_distro()?;
    println!("Architecture: {}", std::env::consts::ARCH);
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}

pub fn panic_handler(panic_info: &PanicHookInfo<'_>) {
    if let Some(location) = panic_info.location() {
        println!(
            "Panic occurred in file '{}' at line {}!",
            location.file(),
            location.line(),
        );

        // Read the file and display the line
        if let Ok(content) = std::fs::read_to_string(location.file()) {
            let lines: Vec<&str> = content.lines().collect();
            if let Some(line) = lines.get((location.line() - 1) as usize) {
                println!("\n{:>4} | {}", location.line(), line);
                println!(
                    "     | {: >width$}^",
                    "",
                    width = (location.column() - 1) as usize
                );
            }
        }
    }

    if let Some(message) = panic_info.payload().downcast_ref::<String>() {
        println!("Panic message: {}", message);
    } else if let Some(message) = panic_info.payload().downcast_ref::<&str>() {
        println!("Panic message: {}", message);
    } else {
        println!("Panic occurred, message unknown.");
    }
}
