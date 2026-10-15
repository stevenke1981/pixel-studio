use std::path::{Path, PathBuf};

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
    .chain(
        std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join(r"Microsoft\Windows\Fonts")),
    )
    .collect();
    for entry in dirs
        .iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flat_map(|e| e.flatten())
    {
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

/// UI font family for the Slint window. On Windows the default fallback can pick a
/// Japanese/Simplified face that lacks Traditional characters (e.g. 啟), so name a
/// CJK UI font that is actually installed. Elsewhere the platform default is fine.
pub fn ui_font_family() -> &'static str {
    ui_font_family_from(|p| Path::new(p).exists(), cfg!(windows))
}

pub fn ui_font_family_from(exists: impl Fn(&str) -> bool, windows: bool) -> &'static str {
    if !windows {
        return "";
    }
    const CANDIDATES: [(&str, &str); 4] = [
        (r"C:\Windows\Fonts\msjh.ttc", "Microsoft JhengHei UI"),
        (r"C:\Windows\Fonts\msyh.ttc", "Microsoft YaHei UI"),
        (r"C:\Windows\Fonts\mingliu.ttc", "PMingLiU"),
        (r"C:\Windows\Fonts\simsun.ttc", "SimSun"),
    ];
    CANDIDATES
        .iter()
        .find(|(file, _)| exists(file))
        .map(|(_, family)| *family)
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::ui_font_family_from;

    #[test]
    fn windows_prefers_jhenghei_then_yahei() {
        assert_eq!(ui_font_family_from(|_| true, true), "Microsoft JhengHei UI");
        assert_eq!(
            ui_font_family_from(|p| p.ends_with("msyh.ttc"), true),
            "Microsoft YaHei UI"
        );
        assert_eq!(ui_font_family_from(|_| false, true), "");
        assert_eq!(ui_font_family_from(|_| true, false), "");
    }
}
