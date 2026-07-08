#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod fonts;
mod i18n;
mod icons;
mod ui;

use anyhow::Result;

fn main() -> Result<()> {
    let initial_path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        wgpu_options: wgpu_options(),
        viewport: egui::ViewportBuilder::default()
            .with_title("Pixel Studio 像素工作室")
            .with_inner_size([1440.0, 960.0])
            .with_min_inner_size([1100.0, 760.0])
            .with_icon(icons::app_icon()?),
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

fn wgpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
    let mut options = eframe::egui_wgpu::WgpuConfiguration::default();
    if std::env::var_os("WGPU_BACKEND").is_none() {
        let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_setup else {
            return options;
        };
        setup.instance_descriptor.backends = eframe::wgpu::Backends::GL;
    }
    options
}
