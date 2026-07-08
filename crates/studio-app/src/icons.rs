use anyhow::Result;

pub fn app_icon() -> Result<egui::IconData> {
    let bytes = include_bytes!("../../../assets/icon/pack/png/256x256.png");
    eframe::icon_data::from_png_bytes(bytes)
        .map_err(|error| anyhow::anyhow!("failed to load embedded app icon: {error}"))
}
