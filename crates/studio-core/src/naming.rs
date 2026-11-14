//! Default output file names: `pixelstudio_YYYYMMDD_HHMMSS.<ext>` in local time.

use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDateTime};

use crate::SupportedFormat;

/// Name for a given moment, e.g. `pixelstudio_20261015_164305.png`.
pub fn timestamp_name_at(at: NaiveDateTime, ext: &str) -> String {
    format!(
        "pixelstudio_{}.{}",
        at.format("%Y%m%d_%H%M%S"),
        ext.trim_start_matches('.')
    )
}

/// Name for "now" in local time.
pub fn timestamp_name(ext: &str) -> String {
    timestamp_name_at(Local::now().naive_local(), ext)
}

/// A timestamped path in `dir`; adds `_2`, `_3`… if that file already exists.
pub fn timestamp_path_in(dir: &Path, ext: &str) -> PathBuf {
    let base = timestamp_name(ext);
    let mut p = dir.join(&base);
    let stem = base
        .trim_end_matches(&format!(".{}", ext.trim_start_matches('.')))
        .to_owned();
    let mut n = 2;
    while p.exists() {
        p = dir.join(format!("{stem}_{n}.{}", ext.trim_start_matches('.')));
        n += 1;
    }
    p
}

/// Suggest an output filename for saving or converting to `target_format`.
/// Preserves the source file's UTF-8 stem when available, falling back to a timestamped name.
pub fn suggested_output_name(source: Option<&Path>, target_format: SupportedFormat) -> String {
    if let Some(stem) = source
        .and_then(|path| path.file_stem())
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
    {
        return format!("{stem}.{}", target_format.extension());
    }
    timestamp_name(target_format.extension())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn formats_local_timestamp_name() {
        let at = NaiveDate::from_ymd_opt(2026, 10, 15)
            .unwrap()
            .and_hms_opt(16, 43, 5)
            .unwrap();
        assert_eq!(
            timestamp_name_at(at, "png"),
            "pixelstudio_20261015_164305.png"
        );
        assert_eq!(
            timestamp_name_at(at, ".webp"),
            "pixelstudio_20261015_164305.webp"
        );
    }

    #[test]
    fn now_name_shape_and_unique_path() {
        let n = timestamp_name("jpg");
        assert!(
            n.starts_with("pixelstudio_")
                && n.ends_with(".jpg")
                && n.len() == "pixelstudio_20261015_164305.jpg".len()
        );
        let dir = std::env::temp_dir().join(format!("ps-naming-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = timestamp_path_in(&dir, "png");
        std::fs::write(&a, b"x").unwrap();
        let b = timestamp_path_in(&dir, "png");
        assert_ne!(a, b);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn suggests_output_name_from_source_stem_or_timestamp() {
        assert_eq!(
            suggested_output_name(Some(Path::new("a/photo.png")), SupportedFormat::Jfif),
            "photo.jfif"
        );
        assert_eq!(
            suggested_output_name(Some(Path::new("dir/shot.tar.gz")), SupportedFormat::Qoi),
            "shot.tar.qoi"
        );
        let fallback = suggested_output_name(None, SupportedFormat::Jpeg);
        assert!(fallback.starts_with("pixelstudio_") && fallback.ends_with(".jpg"));
        let empty_fallback = suggested_output_name(Some(Path::new("")), SupportedFormat::Tga);
        assert!(empty_fallback.starts_with("pixelstudio_") && empty_fallback.ends_with(".tga"));
    }
}
