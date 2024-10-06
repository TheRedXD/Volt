use std::panic::PanicHookInfo;

fn get_desktop_environment() -> String {
    if cfg!(target_os = "linux") {
        std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "None".to_string())
    } else {
        "None".to_string()
    }
}

fn get_compositor() -> String {
    if cfg!(target_os = "linux") {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            "Wayland".to_string()
        } else {
            "X11".to_string()
        }
    } else {
        "None".to_string()
    }
}

fn get_cpu_info() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(contents) = std::fs::read_to_string("/proc/cpuinfo") {
            if let Some(line) = contents.lines().find(|line| line.starts_with("model name")) {
                if let Some(cpu) = line.split(':').nth(1) {
                    return cpu.trim().to_string();
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("wmic").args(&["cpu", "get", "name"]).output() {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                if let Some(cpu) = stdout.lines().nth(1) {
                    return cpu.trim().to_string();
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
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

    "Unknown CPU".to_string()
}

fn get_gpu_info() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = std::process::Command::new("lspci").output() {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                if let Some(gpu_line) = stdout
                    .lines()
                    .find(|line| line.contains("VGA") || line.contains("3D"))
                {
                    return gpu_line
                        .split(':')
                        .nth(2)
                        .unwrap_or("Unknown GPU")
                        .trim()
                        .to_string();
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("wmic")
            .args(&["path", "win32_VideoController", "get", "name"])
            .output()
        {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                if let Some(gpu) = stdout.lines().nth(1) {
                    return gpu.trim().to_string();
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("system_profiler")
            .arg("SPDisplaysDataType")
            .output()
        {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                if let Some(gpu_line) = stdout.lines().find(|line| line.contains("Chipset Model:"))
                {
                    return gpu_line
                        .split(':')
                        .nth(1)
                        .unwrap_or("Unknown GPU")
                        .trim()
                        .to_string();
                }
            }
        }
    }

    "Unknown GPU".to_string()
}
pub fn dump() {
    let mut distro: String = "None".into();
    #[cfg(target_os = "linux")]
    {
        if let Ok(release_file) = std::fs::read_to_string("/etc/os-release") {
            if let Some(line) = release_file.lines().find(|l| l.starts_with("PRETTY_NAME=")) {
                distro = line
                    .split('=')
                    .nth(1)
                    .unwrap_or("Unknown")
                    .trim_matches('"')
                    .to_string();
            }
        }
    }

    println!("OS: {}", std::env::consts::OS);
    println!("Desktop Environment: {}", get_desktop_environment());
    println!("Compositor: {}", get_compositor());
    println!("CPU: {}", get_cpu_info());
    println!("GPU: {}", get_gpu_info());
    println!("OS Family: {}", std::env::consts::FAMILY);
    println!("OS Distribution: {distro}");
    println!("Architecture: {}", std::env::consts::ARCH);
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
}

pub fn handle() {
    if std::env::args().any(|x| x == *"--info") {
        dump();
        std::process::exit(0);
    }
    if std::env::args().any(|x| x == *"--verbose") {
        println!("Running Volt in verbose mode! Various debug logs will now get logged. For convenience, a debug.log file is also being written to.");
    }
}

// TODO: could use the `human_panic` crate
// For future reference: https://crates.io/crates/human-panic
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
        println!("Panic message: {message}");
    } else if let Some(message) = panic_info.payload().downcast_ref::<&str>() {
        println!("Panic message: {message}");
    } else {
        println!("Panic occurred, message unknown.");
    }
}
