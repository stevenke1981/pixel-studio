#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod capture;
mod doc;
mod fonts;
pub mod shell;
mod ui;

use anyhow::Result;

/// Parsed command line: `pixel-studio [--edit|--view] [FILE]`,
/// `--register-shell`, `--unregister-shell`, `--version`.
#[derive(Debug, Default, PartialEq)]
pub struct Args {
    pub file: Option<std::path::PathBuf>,
    pub view: bool,
}

fn console_print(msg: &str) {
    #[cfg(windows)]
    unsafe {
        // GUI-subsystem exe: attach to the parent console so CLI flags can print.
        unsafe extern "system" {
            fn AttachConsole(pid: u32) -> i32;
        }
        AttachConsole(u32::MAX);
    }
    println!("{msg}");
}

/// On Windows prefer the Skia renderer: the software renderer's partial presentation
/// can leave regions of the window unpainted (desktop shows through). Fall back to the
/// software renderer if Skia cannot initialise (e.g. no GPU / RDP without GL).
fn select_renderer() {
    #[cfg(windows)]
    if std::env::var_os("SLINT_BACKEND").is_none() {
        let skia = slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("skia".into())
            .select();
        if skia.is_err() {
            let _ = slint::BackendSelector::new()
                .backend_name("winit".into())
                .renderer_name("software".into())
                .select();
        }
    }
}

fn main() -> Result<()> {
    let mut args = Args::default();
    for a in std::env::args_os().skip(1) {
        match a.to_str() {
            Some("--register-shell") => {
                let zh = std::env::var("PIXEL_STUDIO_LANG")
                    .map(|l| l != "en")
                    .unwrap_or(true);
                console_print(&shell::register(zh)?);
                return Ok(());
            }
            Some("--unregister-shell") => {
                console_print(&shell::unregister()?);
                return Ok(());
            }
            Some("--version" | "-V") => {
                console_print(&format!("pixel-studio {}", env!("CARGO_PKG_VERSION")));
                return Ok(());
            }
            Some("--view") => args.view = true,
            Some("--edit") => args.view = false,
            _ => args.file = Some(std::path::PathBuf::from(a)),
        }
    }
    select_renderer();
    ui::run(args)
}
