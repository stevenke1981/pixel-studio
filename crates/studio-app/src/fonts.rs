use std::path::{Path, PathBuf};

pub fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    if let Some(bytes) = load_system_font_bytes() {
        fonts.font_data.insert(
            "windows_cjk".to_owned(),
            egui::FontData::from_owned(bytes).into(),
        );

        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, "windows_cjk".to_owned());
        }
    }

    ctx.set_fonts(fonts);
}

pub fn load_system_font_bytes() -> Option<Vec<u8>> {
    [
        r"C:\Windows\Fonts\NotoSansTC-VF.ttf",
        r"C:\Windows\Fonts\msjh.ttc",
        r"C:\Windows\Fonts\mingliu.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
    ]
    .into_iter()
    .find_map(|path| std::fs::read(path).ok())
}

pub fn list_system_font_paths() -> Vec<PathBuf> {
    let mut fonts = Vec::new();
    let Ok(entries) = std::fs::read_dir(r"C:\Windows\Fonts") else {
        return fonts;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        if matches!(
            extension.to_ascii_lowercase().as_str(),
            "ttf" | "ttc" | "otf"
        ) {
            fonts.push(path);
        }
    }

    fonts.sort();
    fonts
}

pub fn display_font_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .or_else(|| path.file_name().and_then(|value| value.to_str()))
        .unwrap_or("Font")
        .to_owned()
}
