#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
pub mod capture;
mod fonts;
pub mod i18n;
pub mod paint;
mod theme;

use anyhow::Result;

fn main() -> Result<()> {
    let initial_path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Pixel Paint & Viewer 像素畫家與圖片檢視器")
            .with_inner_size([1180.0, 720.0])
            .with_min_inner_size([960.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Pixel Paint & Viewer 像素畫家與圖片檢視器",
        options,
        Box::new(|creation_context| {
            fonts::configure_fonts(&creation_context.egui_ctx);
            theme::configure(&creation_context.egui_ctx);
            Ok(Box::new(app::PixelStudioApp::new(initial_path)))
        }),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))
}
