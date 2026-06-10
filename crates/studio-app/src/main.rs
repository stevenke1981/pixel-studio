#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod fonts;
mod i18n;

use anyhow::Result;

fn main() -> Result<()> {
    let initial_path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Pixel Studio 像素工作室")
            .with_inner_size([1440.0, 960.0])
            .with_min_inner_size([1100.0, 760.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Pixel Studio 像素工作室",
        options,
        Box::new(|creation_context| {
            fonts::configure_fonts(&creation_context.egui_ctx);
            Ok(Box::new(app::PixelStudioApp::new(initial_path)))
        }),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))
}
