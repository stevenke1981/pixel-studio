use std::path::{Path, PathBuf};

use image::GenericImageView;

use crate::{PhotoError, Result, SupportedFormat};

#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub path: PathBuf,
    pub format: SupportedFormat,
    pub width: u32,
    pub height: u32,
    pub color_type: String,
    pub file_size: u64,
}

pub fn inspect_image(path: impl AsRef<Path>) -> Result<ImageInfo> {
    let path = path.as_ref();
    let format = SupportedFormat::from_path(path)
        .ok_or_else(|| PhotoError::UnsupportedFormat(path.to_path_buf()))?;
    let metadata = std::fs::metadata(path)?;
    let image = image::open(path)?;
    let (width, height) = image.dimensions();

    Ok(ImageInfo {
        path: path.to_path_buf(),
        format,
        width,
        height,
        color_type: format!("{:?}", image.color()),
        file_size: metadata.len(),
    })
}

pub fn load_dynamic_image(path: impl AsRef<Path>) -> Result<image::DynamicImage> {
    let path = path.as_ref();
    if SupportedFormat::from_path(path).is_none() {
        return Err(PhotoError::UnsupportedFormat(path.to_path_buf()));
    }

    Ok(image::open(path)?)
}
