use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SupportedFormat {
    Jpeg,
    Png,
    Webp,
    Bmp,
    Tiff,
}

impl SupportedFormat {
    pub const ALL: [SupportedFormat; 5] = [
        SupportedFormat::Jpeg,
        SupportedFormat::Png,
        SupportedFormat::Webp,
        SupportedFormat::Bmp,
        SupportedFormat::Tiff,
    ];

    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "png" => Some(Self::Png),
            "webp" => Some(Self::Webp),
            "bmp" => Some(Self::Bmp),
            "tif" | "tiff" => Some(Self::Tiff),
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
            Self::Jpeg => image::ImageFormat::Jpeg,
            Self::Png => image::ImageFormat::Png,
            Self::Webp => image::ImageFormat::WebP,
            Self::Bmp => image::ImageFormat::Bmp,
            Self::Tiff => image::ImageFormat::Tiff,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Jpeg => "JPEG",
            Self::Png => "PNG",
            Self::Webp => "WEBP",
            Self::Bmp => "BMP",
            Self::Tiff => "TIFF",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
        }
    }

    pub fn supports_alpha(self) -> bool {
        matches!(self, Self::Png | Self::Webp | Self::Tiff)
    }
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
    use super::SupportedFormat;

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
            SupportedFormat::from_extension(".png"),
            Some(SupportedFormat::Png)
        );
        assert_eq!(SupportedFormat::from_extension("unknown"), None);
    }
}
