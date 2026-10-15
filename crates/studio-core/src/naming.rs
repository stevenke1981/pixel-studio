//! Default output file names: `pixelstudio_YYYYMMDD_HHMMSS.<ext>` in local time.

use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDateTime};

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
}
