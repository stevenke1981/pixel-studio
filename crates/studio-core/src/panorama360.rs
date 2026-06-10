use std::path::{Path, PathBuf};

use image::imageops::{FilterType, overlay};
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

use crate::convert::{encode_jpeg_bytes, flatten_alpha};
use crate::gpano_xmp::{build_gpano_xmp, inject_xmp_into_jpeg};
use crate::{Result, load_dynamic_image};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanoramaMode {
    Pad,
    Stretch,
    Crop,
}

impl std::str::FromStr for PanoramaMode {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "pad" => Ok(Self::Pad),
            "stretch" => Ok(Self::Stretch),
            "crop" | "center-crop" => Ok(Self::Crop),
            _ => Err(format!("unsupported panorama mode: {value}")),
        }
    }
}

impl std::fmt::Display for PanoramaMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Pad => "pad",
            Self::Stretch => "stretch",
            Self::Crop => "crop",
        };
        formatter.write_str(label)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PanoramaOptions {
    pub mode: PanoramaMode,
    pub target_width: Option<u32>,
    pub quality: u8,
    pub background: [u8; 4],
}

impl Default for PanoramaOptions {
    fn default() -> Self {
        Self {
            mode: PanoramaMode::Pad,
            target_width: None,
            quality: 92,
            background: [0, 0, 0, 255],
        }
    }
}

#[derive(Debug, Clone)]
pub struct PanoramaResult {
    pub input: PathBuf,
    pub output: PathBuf,
    pub width: u32,
    pub height: u32,
    pub mode: PanoramaMode,
}

pub fn make_equirectangular(image: &DynamicImage, options: &PanoramaOptions) -> DynamicImage {
    let (target_width, target_height) = target_dimensions(image, options.target_width);

    match options.mode {
        PanoramaMode::Pad => pad_to_equirectangular(image, target_width, target_height, options),
        PanoramaMode::Stretch => {
            image.resize_exact(target_width, target_height, FilterType::Lanczos3)
        }
        PanoramaMode::Crop => crop_to_equirectangular(image, target_width, target_height),
    }
}

pub fn write_panorama_jpeg(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: PanoramaOptions,
) -> Result<PanoramaResult> {
    let input = input.as_ref();
    let output = output.as_ref();
    let source = load_dynamic_image(input)?;
    write_panorama_dynamic_jpeg(&source, output, options, input.to_path_buf())
}

pub fn write_panorama_dynamic_jpeg(
    source: &DynamicImage,
    output: impl AsRef<Path>,
    options: PanoramaOptions,
    input: PathBuf,
) -> Result<PanoramaResult> {
    let output = output.as_ref();
    let panorama = make_equirectangular(source, &options);
    let flattened = flatten_alpha(&panorama, options.background);
    let jpeg = encode_jpeg_bytes(&flattened, options.quality)?;
    let xmp = build_gpano_xmp(flattened.width(), flattened.height());
    let injected = inject_xmp_into_jpeg(&jpeg, &xmp)?;
    std::fs::write(output, injected)?;

    Ok(PanoramaResult {
        input,
        output: output.to_path_buf(),
        width: flattened.width(),
        height: flattened.height(),
        mode: options.mode,
    })
}

fn target_dimensions(image: &DynamicImage, requested_width: Option<u32>) -> (u32, u32) {
    let natural_width = image.width().max(image.height().saturating_mul(2)).max(2);
    let mut width = requested_width.unwrap_or(natural_width).max(2);
    if width % 2 == 1 {
        width += 1;
    }
    (width, width / 2)
}

fn pad_to_equirectangular(
    image: &DynamicImage,
    target_width: u32,
    target_height: u32,
    options: &PanoramaOptions,
) -> DynamicImage {
    let mut canvas = RgbaImage::from_pixel(target_width, target_height, Rgba(options.background));
    let scale = (target_width as f32 / image.width() as f32)
        .min(target_height as f32 / image.height() as f32)
        .max(0.0001);
    let width = ((image.width() as f32 * scale).round() as u32).max(1);
    let height = ((image.height() as f32 * scale).round() as u32).max(1);
    let resized = image.resize(width, height, FilterType::Lanczos3).to_rgba8();
    let x = ((target_width - width) / 2) as i64;
    let y = ((target_height - height) / 2) as i64;
    overlay(&mut canvas, &resized, x, y);
    DynamicImage::ImageRgba8(canvas)
}

fn crop_to_equirectangular(
    image: &DynamicImage,
    target_width: u32,
    target_height: u32,
) -> DynamicImage {
    let (width, height) = image.dimensions();
    let current_ratio = width as f32 / height as f32;

    let (crop_x, crop_y, crop_width, crop_height) = if current_ratio > 2.0 {
        let crop_width = height.saturating_mul(2).min(width).max(1);
        ((width - crop_width) / 2, 0, crop_width, height)
    } else {
        let crop_height = (width / 2).min(height).max(1);
        (0, (height - crop_height) / 2, width, crop_height)
    };

    image
        .crop_imm(crop_x, crop_y, crop_width, crop_height)
        .resize_exact(target_width, target_height, FilterType::Lanczos3)
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, RgbaImage};

    use super::{PanoramaMode, PanoramaOptions, make_equirectangular};

    fn sample_image() -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::new(80, 60))
    }

    #[test]
    fn pad_mode_outputs_two_to_one() {
        let image = make_equirectangular(
            &sample_image(),
            &PanoramaOptions {
                mode: PanoramaMode::Pad,
                target_width: Some(100),
                ..Default::default()
            },
        );
        assert_eq!((image.width(), image.height()), (100, 50));
    }

    #[test]
    fn stretch_mode_outputs_two_to_one() {
        let image = make_equirectangular(
            &sample_image(),
            &PanoramaOptions {
                mode: PanoramaMode::Stretch,
                target_width: Some(100),
                ..Default::default()
            },
        );
        assert_eq!((image.width(), image.height()), (100, 50));
    }

    #[test]
    fn crop_mode_outputs_two_to_one() {
        let image = make_equirectangular(
            &sample_image(),
            &PanoramaOptions {
                mode: PanoramaMode::Crop,
                target_width: Some(100),
                ..Default::default()
            },
        );
        assert_eq!((image.width(), image.height()), (100, 50));
    }
}
