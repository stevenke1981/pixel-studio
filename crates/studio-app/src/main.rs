#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
pub mod capture;
mod fonts;
pub mod i18n;
pub mod paint;
pub mod shell;
mod theme;

use anyhow::Result;

const TITLE: &str = "Pixel Studio 像素工作室";

/// Parsed command line: `pixel-studio [--edit|--view] [FILE]`,
/// `--register-shell`, `--unregister-shell`, `--version`.
#[derive(Debug, Default, PartialEq)]
struct Args {
    file: Option<std::path::PathBuf>,
    view: bool,
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

fn main() -> Result<()> {
    let mut args = Args::default();
    for a in std::env::args_os().skip(1) {
        match a.to_str() {
            Some("--register-shell") => {
                let zh = std::env::var("PIXEL_STUDIO_LANG")
                    .map(|l| l != "en")
                    .unwrap_or(true);
                let msg = shell::register(zh)?;
                console_print(&msg);
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

    let make_options = |renderer| eframe::NativeOptions {
        renderer,
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_app_id("pixel-studio")
            .with_icon(app_icon())
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 640.0]),
        ..Default::default()
    };
    // wgpu (DX12/Metal/Vulkan) first; fall back to OpenGL if no adapter is available.
    let order = match std::env::var("PIXEL_STUDIO_RENDERER").as_deref() {
        Ok("glow") => [eframe::Renderer::Glow, eframe::Renderer::Wgpu],
        _ => [eframe::Renderer::Wgpu, eframe::Renderer::Glow],
    };
    let mut last_err = None;
    for renderer in order {
        let file = args.file.clone();
        let view = args.view;
        match eframe::run_native(
            TITLE,
            make_options(renderer),
            Box::new(move |cc| {
                fonts::configure_fonts(&cc.egui_ctx);
                theme::configure(&cc.egui_ctx);
                Ok(Box::new(app::PixelStudioApp::new_with_mode(file, view)))
            }),
        ) {
            Ok(()) => return Ok(()),
            Err(e) => {
                eprintln!("renderer {renderer:?} failed: {e}");
                last_err = Some(e);
            }
        }
    }
    Err(anyhow::anyhow!(
        last_err.map(|e| e.to_string()).unwrap_or_default()
    ))
}

fn app_icon() -> egui::IconData {
    let bytes = include_bytes!("../assets/icon-256.png");
    let img = image::load_from_memory(bytes)
        .map(|i| i.to_rgba8())
        .unwrap_or_default();
    egui::IconData {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    }
}
