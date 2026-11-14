use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SupportedFormat {
    Jpeg,
    Jfif,
    Png,
    Webp,
    Bmp,
    Tiff,
    Gif,
    Ico,
    Tga,
    Qoi,
}

impl SupportedFormat {
    pub const ALL: [SupportedFormat; 10] = [
        SupportedFormat::Jpeg,
        SupportedFormat::Jfif,
        SupportedFormat::Png,
        SupportedFormat::Webp,
        SupportedFormat::Bmp,
        SupportedFormat::Tiff,
        SupportedFormat::Gif,
        SupportedFormat::Ico,
        SupportedFormat::Tga,
        SupportedFormat::Qoi,
    ];

    const ALL_EXTENSIONS: &'static [&'static str] = &[
        "jpg", "jpeg", "jfif", "png", "webp", "bmp", "tif", "tiff", "gif", "ico", "tga", "qoi",
    ];

    const C2PA_EXTENSIONS: &'static [&'static str] =
        &["jpg", "jpeg", "jfif", "png", "webp", "tif", "tiff"];

    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "jfif" => Some(Self::Jfif),
            "png" => Some(Self::Png),
            "webp" => Some(Self::Webp),
            "bmp" => Some(Self::Bmp),
            "tif" | "tiff" => Some(Self::Tiff),
            "gif" => Some(Self::Gif),
            "ico" => Some(Self::Ico),
            "tga" => Some(Self::Tga),
            "qoi" => Some(Self::Qoi),
            _ => None,
        }
    }

    pub fn from_path(path: impl AsRef<Path>) -> Option<Self> {
        path.as_ref()
            .extension()
            .and_then(|value| value.to_str())
            .and_then(Self::from_extension)
    }

    pub fn image_format(self) -> image::ImageFormat {
        match self {
            Self::Jpeg | Self::Jfif => image::ImageFormat::Jpeg,
            Self::Png => image::ImageFormat::Png,
            Self::Webp => image::ImageFormat::WebP,
            Self::Bmp => image::ImageFormat::Bmp,
            Self::Tiff => image::ImageFormat::Tiff,
            Self::Gif => image::ImageFormat::Gif,
            Self::Ico => image::ImageFormat::Ico,
            Self::Tga => image::ImageFormat::Tga,
            Self::Qoi => image::ImageFormat::Qoi,
        }
    }

    pub fn from_image_format(fmt: image::ImageFormat) -> Option<Self> {
        match fmt {
            image::ImageFormat::Jpeg => Some(Self::Jpeg),
            image::ImageFormat::Png => Some(Self::Png),
            image::ImageFormat::WebP => Some(Self::Webp),
            image::ImageFormat::Bmp => Some(Self::Bmp),
            image::ImageFormat::Tiff => Some(Self::Tiff),
            image::ImageFormat::Gif => Some(Self::Gif),
            image::ImageFormat::Ico => Some(Self::Ico),
            image::ImageFormat::Tga => Some(Self::Tga),
            image::ImageFormat::Qoi => Some(Self::Qoi),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Jpeg => "JPEG",
            Self::Jfif => "JFIF",
            Self::Png => "PNG",
            Self::Webp => "WEBP",
            Self::Bmp => "BMP",
            Self::Tiff => "TIFF",
            Self::Gif => "GIF",
            Self::Ico => "ICO",
            Self::Tga => "TGA",
            Self::Qoi => "QOI",
        }
    }

    pub fn filter_label(self, zh: bool) -> &'static str {
        match (self, zh) {
            (Self::Jpeg, true) => "JPEG 影像",
            (Self::Jpeg, false) => "JPEG Image",
            (Self::Jfif, true) => "JFIF 影像",
            (Self::Jfif, false) => "JFIF Image",
            (Self::Png, true) => "PNG 影像",
            (Self::Png, false) => "PNG Image",
            (Self::Webp, true) => "WebP 影像",
            (Self::Webp, false) => "WebP Image",
            (Self::Bmp, true) => "BMP 點陣圖",
            (Self::Bmp, false) => "BMP Image",
            (Self::Tiff, true) => "TIFF 影像",
            (Self::Tiff, false) => "TIFF Image",
            (Self::Gif, true) => "GIF 影像",
            (Self::Gif, false) => "GIF Image",
            (Self::Ico, true) => "ICO 圖示",
            (Self::Ico, false) => "ICO Icon",
            (Self::Tga, true) => "TGA 影像",
            (Self::Tga, false) => "TGA Image",
            (Self::Qoi, true) => "QOI 影像",
            (Self::Qoi, false) => "QOI Image",
        }
    }

    pub fn filter_display_name(self, zh: bool) -> String {
        let patterns = self
            .extensions()
            .iter()
            .map(|ext| format!("*.{ext}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{} ({patterns})", self.filter_label(zh))
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Jfif => "jfif",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
            Self::Gif => "gif",
            Self::Ico => "ico",
            Self::Tga => "tga",
            Self::Qoi => "qoi",
        }
    }

    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Jpeg => &["jpg", "jpeg"],
            Self::Jfif => &["jfif"],
            Self::Png => &["png"],
            Self::Webp => &["webp"],
            Self::Bmp => &["bmp"],
            Self::Tiff => &["tif", "tiff"],
            Self::Gif => &["gif"],
            Self::Ico => &["ico"],
            Self::Tga => &["tga"],
            Self::Qoi => &["qoi"],
        }
    }

    pub const fn all_extensions() -> &'static [&'static str] {
        Self::ALL_EXTENSIONS
    }

    pub const fn c2pa_extensions() -> &'static [&'static str] {
        Self::C2PA_EXTENSIONS
    }

    pub fn mime_types(self) -> &'static [&'static str] {
        match self {
            Self::Jpeg | Self::Jfif => &["image/jpeg"],
            Self::Png => &["image/png"],
            Self::Webp => &["image/webp"],
            Self::Bmp => &["image/bmp", "image/x-bmp"],
            Self::Tiff => &["image/tiff"],
            Self::Gif => &["image/gif"],
            Self::Ico => &["image/x-icon", "image/vnd.microsoft.icon"],
            Self::Tga => &["image/x-targa", "image/x-tga"],
            Self::Qoi => &["image/x-qoi"],
        }
    }

    pub fn mime_type(self) -> &'static str {
        self.mime_types()[0]
    }

    pub fn all_mime_types() -> Vec<&'static str> {
        let mut out = Vec::new();
        for fmt in Self::ALL {
            for mime in fmt.mime_types() {
                if !out.contains(mime) {
                    out.push(*mime);
                }
            }
        }
        out
    }

    pub fn uti(self) -> &'static str {
        match self {
            Self::Jpeg | Self::Jfif => "public.jpeg",
            Self::Png => "public.png",
            Self::Webp => "org.webmproject.webp",
            Self::Bmp => "com.microsoft.bmp",
            Self::Tiff => "public.tiff",
            Self::Gif => "com.compuserve.gif",
            Self::Ico => "com.microsoft.ico",
            Self::Tga => "com.truevision.tga-image",
            Self::Qoi => "org.qoiformat.qoi",
        }
    }

    pub fn all_utis() -> Vec<&'static str> {
        let mut out = Vec::new();
        for fmt in Self::ALL {
            let uti = fmt.uti();
            if !out.contains(&uti) {
                out.push(uti);
            }
        }
        out
    }

    pub fn supports_alpha(self) -> bool {
        matches!(
            self,
            Self::Png | Self::Webp | Self::Tiff | Self::Gif | Self::Ico | Self::Tga | Self::Qoi
        )
    }

    pub fn supports_quality(self) -> bool {
        matches!(self, Self::Jpeg | Self::Jfif | Self::Webp)
    }

    pub fn supports_c2pa(self) -> bool {
        matches!(
            self,
            Self::Jpeg | Self::Jfif | Self::Png | Self::Webp | Self::Tiff
        )
    }

    pub fn supports_exif_icc(self) -> bool {
        matches!(self, Self::Jpeg | Self::Jfif | Self::Png | Self::Webp)
    }

    /// Build ordered file-dialog filter descriptors `(label, extensions)` for save/export/convert.
    /// Places `preferred` first, followed by "All Supported Images", followed by remaining formats.
    pub fn save_filter_entries(
        preferred: Self,
        zh: bool,
        c2pa_only: bool,
    ) -> Vec<(String, &'static [&'static str])> {
        let mut entries = Vec::with_capacity(Self::ALL.len() + 1);
        let first = if c2pa_only && !preferred.supports_c2pa() {
            Self::Jpeg
        } else {
            preferred
        };
        entries.push((first.filter_display_name(zh), first.extensions()));
        let all_label = if zh {
            if c2pa_only {
                "所有支援 C2PA 的圖片"
            } else {
                "所有支援的圖片"
            }
        } else if c2pa_only {
            "All C2PA-Supported Images"
        } else {
            "All Supported Images"
        };
        entries.push((
            all_label.to_owned(),
            if c2pa_only {
                Self::c2pa_extensions()
            } else {
                Self::all_extensions()
            },
        ));
        for fmt in Self::ALL {
            if fmt == first || (c2pa_only && !fmt.supports_c2pa()) {
                continue;
            }
            entries.push((fmt.filter_display_name(zh), fmt.extensions()));
        }
        entries
    }

    /// Build ordered file-dialog filter descriptors `(label, extensions)` for opening/picking images.
    pub fn open_filter_entries(zh: bool) -> Vec<(String, &'static [&'static str])> {
        let mut entries = Vec::with_capacity(Self::ALL.len() + 1);
        entries.push((
            if zh {
                "所有支援的圖片"
            } else {
                "All Supported Images"
            }
            .to_owned(),
            Self::all_extensions(),
        ));
        for fmt in Self::ALL {
            entries.push((fmt.filter_display_name(zh), fmt.extensions()));
        }
        entries
    }
}

/// Compute the normalized output quality for `format`.
///
/// For WebP, `lossless == true` or `quality == 100` yields lossless encoding (`100`),
/// while lossy encoding is clamped to `1..=99`.
pub fn effective_quality(format: SupportedFormat, quality: u8, lossless: bool) -> u8 {
    let clamped = quality.clamp(1, 100);
    if format == SupportedFormat::Webp {
        if lossless || clamped == 100 {
            100
        } else {
            clamped.min(99)
        }
    } else {
        clamped
    }
}

/// Resolve the final output path and effective format from a user-chosen save path.
///
/// - If `chosen_path` already has a recognized image extension, honor it.
/// - If `chosen_path` has no extension (or an empty trailing-dot extension), set `chosen_format.extension()`.
/// - If `chosen_path` has an unrecognized extension, append `.{chosen_format.extension()}`
///   to avoid clobbering arbitrary suffixes like `photo.v2` or writing mismatched container bytes.
pub fn resolve_save_target(
    chosen_path: impl AsRef<Path>,
    chosen_format: SupportedFormat,
) -> (PathBuf, SupportedFormat) {
    let chosen_path = chosen_path.as_ref();
    if let Some(explicit_format) = SupportedFormat::from_path(chosen_path) {
        return (chosen_path.to_path_buf(), explicit_format);
    }

    let mut path = chosen_path.to_path_buf();
    let has_nonempty_extension = chosen_path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| !ext.is_empty());

    if !has_nonempty_extension {
        path.set_extension(chosen_format.extension());
    } else {
        let mut os = path.into_os_string();
        os.push(".");
        os.push(chosen_format.extension());
        path = PathBuf::from(os);
    }
    (path, chosen_format)
}

impl std::str::FromStr for SupportedFormat {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        Self::from_extension(value).ok_or_else(|| format!("unsupported format: {value}"))
    }
}

impl std::fmt::Display for SupportedFormat {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{SupportedFormat, resolve_save_target};

    #[test]
    fn detects_common_extensions() {
        assert_eq!(
            SupportedFormat::from_extension("jpg"),
            Some(SupportedFormat::Jpeg)
        );
        assert_eq!(
            SupportedFormat::from_extension("JPEG"),
            Some(SupportedFormat::Jpeg)
        );
        assert_eq!(
            SupportedFormat::from_extension("jfif"),
            Some(SupportedFormat::Jfif)
        );
        assert_eq!(
            SupportedFormat::from_extension(".JFIF"),
            Some(SupportedFormat::Jfif)
        );
        assert_eq!(
            SupportedFormat::from_extension(".png"),
            Some(SupportedFormat::Png)
        );
        assert_eq!(
            SupportedFormat::from_extension("tga"),
            Some(SupportedFormat::Tga)
        );
        assert_eq!(
            SupportedFormat::from_extension(".QOI"),
            Some(SupportedFormat::Qoi)
        );
        assert_eq!(SupportedFormat::from_extension("unknown"), None);
    }

    #[test]
    fn all_ten_formats_have_consistent_metadata() {
        assert_eq!(SupportedFormat::ALL.len(), 10);
        for fmt in SupportedFormat::ALL {
            assert_eq!(SupportedFormat::from_extension(fmt.extension()), Some(fmt));
            for ext in fmt.extensions() {
                assert_eq!(SupportedFormat::from_extension(ext), Some(fmt));
                assert!(
                    SupportedFormat::all_extensions().contains(ext),
                    "missing {ext} from all_extensions"
                );
            }
            assert!(!fmt.mime_type().is_empty());
            assert!(!fmt.filter_label(false).is_empty());
            assert!(!fmt.filter_label(true).is_empty());
            assert!(fmt.filter_display_name(false).contains("*."));
            assert_eq!(fmt.to_string().parse::<SupportedFormat>().unwrap(), fmt);
            if fmt != SupportedFormat::Jfif {
                assert_eq!(
                    SupportedFormat::from_image_format(fmt.image_format()),
                    Some(fmt)
                );
            }
        }
        assert_eq!(
            SupportedFormat::from_image_format(image::ImageFormat::Jpeg),
            Some(SupportedFormat::Jpeg)
        );
    }

    #[test]
    fn capability_flags_match_format_specs() {
        assert!(!SupportedFormat::Jpeg.supports_alpha());
        assert!(!SupportedFormat::Jfif.supports_alpha());
        assert!(!SupportedFormat::Bmp.supports_alpha());
        for fmt in [
            SupportedFormat::Png,
            SupportedFormat::Webp,
            SupportedFormat::Tiff,
            SupportedFormat::Gif,
            SupportedFormat::Ico,
            SupportedFormat::Tga,
            SupportedFormat::Qoi,
        ] {
            assert!(fmt.supports_alpha(), "{fmt} should support alpha");
        }

        for fmt in [
            SupportedFormat::Jpeg,
            SupportedFormat::Jfif,
            SupportedFormat::Webp,
        ] {
            assert!(fmt.supports_quality(), "{fmt} should support quality");
        }
        assert!(!SupportedFormat::Png.supports_quality());
        assert!(!SupportedFormat::Tga.supports_quality());
        assert!(!SupportedFormat::Qoi.supports_quality());

        for fmt in [
            SupportedFormat::Jpeg,
            SupportedFormat::Jfif,
            SupportedFormat::Png,
            SupportedFormat::Webp,
            SupportedFormat::Tiff,
        ] {
            assert!(fmt.supports_c2pa(), "{fmt} should support c2pa");
        }
        for fmt in [
            SupportedFormat::Bmp,
            SupportedFormat::Gif,
            SupportedFormat::Ico,
            SupportedFormat::Tga,
            SupportedFormat::Qoi,
        ] {
            assert!(!fmt.supports_c2pa(), "{fmt} should not support c2pa");
        }

        for fmt in [
            SupportedFormat::Jpeg,
            SupportedFormat::Jfif,
            SupportedFormat::Png,
            SupportedFormat::Webp,
        ] {
            assert!(fmt.supports_exif_icc(), "{fmt} should support exif/icc");
        }
        assert!(!SupportedFormat::Tiff.supports_exif_icc());
        assert!(!SupportedFormat::Tga.supports_exif_icc());
        assert!(!SupportedFormat::Qoi.supports_exif_icc());
    }

    #[test]
    fn resolves_save_target_extensions() {
        assert_eq!(
            resolve_save_target("dir/photo.jfif", SupportedFormat::Png),
            (PathBuf::from("dir/photo.jfif"), SupportedFormat::Jfif)
        );
        assert_eq!(
            resolve_save_target("dir/photo.JPEG", SupportedFormat::Png),
            (PathBuf::from("dir/photo.JPEG"), SupportedFormat::Jpeg)
        );
        assert_eq!(
            resolve_save_target("dir/photo", SupportedFormat::Jfif),
            (PathBuf::from("dir/photo.jfif"), SupportedFormat::Jfif)
        );
        assert_eq!(
            resolve_save_target("dir/photo.", SupportedFormat::Qoi),
            (PathBuf::from("dir/photo.qoi"), SupportedFormat::Qoi)
        );
        assert_eq!(
            resolve_save_target("dir/photo.v2", SupportedFormat::Tga),
            (PathBuf::from("dir/photo.v2.tga"), SupportedFormat::Tga)
        );
    }

    #[test]
    fn builds_save_filter_entries_with_preferred_first() {
        let entries = SupportedFormat::save_filter_entries(SupportedFormat::Jfif, false, false);
        assert_eq!(entries.len(), SupportedFormat::ALL.len() + 1);
        assert_eq!(entries[0].1, &["jfif"]);
        assert!(entries[0].0.starts_with("JFIF Image"));
        assert_eq!(entries[1].0, "All Supported Images");
        assert_eq!(entries[1].1, SupportedFormat::all_extensions());

        let c2pa_entries = SupportedFormat::save_filter_entries(SupportedFormat::Tga, false, true);
        assert_eq!(c2pa_entries[0].1, &["jpg", "jpeg"]);
        assert_eq!(c2pa_entries[1].1, SupportedFormat::c2pa_extensions());
        assert_eq!(c2pa_entries.len(), 6); // 5 C2PA formats + All C2PA-Supported Images

        let open_entries = SupportedFormat::open_filter_entries(false);
        assert_eq!(open_entries.len(), SupportedFormat::ALL.len() + 1);
        assert_eq!(open_entries[0].0, "All Supported Images");
        assert_eq!(open_entries[0].1, SupportedFormat::all_extensions());
        assert!(SupportedFormat::all_utis().contains(&"org.qoiformat.qoi"));
        assert!(SupportedFormat::all_mime_types().contains(&"image/x-qoi"));
        assert_eq!(
            super::effective_quality(SupportedFormat::Webp, 92, false),
            92
        );
        assert_eq!(
            super::effective_quality(SupportedFormat::Webp, 100, false),
            100
        );
        assert_eq!(
            super::effective_quality(SupportedFormat::Webp, 75, true),
            100
        );
        assert_eq!(
            super::effective_quality(SupportedFormat::Jpeg, 100, true),
            100
        );
    }
}
