use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
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

    // Symbol fallbacks for toolbar icons (appended after egui's own emoji font).
    let symbol_fonts = [
        r"C:\Windows\Fonts\seguisym.ttf",
        r"C:\Windows\Fonts\seguiemj.ttf",
        "/System/Library/Fonts/Apple Symbols.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ];
    for (i, path) in symbol_fonts.iter().enumerate() {
        if let Ok(bytes) = std::fs::read(path) {
            let name = format!("symbol_fallback_{i}");
            fonts.font_data.insert(name.clone(), egui::FontData::from_owned(bytes).into());
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push(name.clone());
            }
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
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJKtc-Regular.otf",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
    ]
    .into_iter()
    .find_map(|path| std::fs::read(path).ok())
}

pub fn list_system_font_paths() -> Vec<PathBuf> {
    let mut fonts = Vec::new();
    let dirs: Vec<PathBuf> = [
        PathBuf::from(r"C:\Windows\Fonts"),
        PathBuf::from("/System/Library/Fonts"),
        PathBuf::from("/Library/Fonts"),
        PathBuf::from("/usr/share/fonts/truetype/dejavu"),
        PathBuf::from("/usr/share/fonts/opentype/noto"),
    ]
    .into_iter()
    .chain(std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join(r"Microsoft\Windows\Fonts")))
    .collect();
    for entry in dirs.iter().filter_map(|d| std::fs::read_dir(d).ok()).flat_map(|e| e.flatten()) {
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

pub fn preview_family_name(path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    format!("studio_app_font_preview_{:x}", hasher.finish())
}
