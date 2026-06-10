use std::fs::File;
use std::io::{BufWriter, Cursor};
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageEncoder, Rgba};

use crate::{PhotoError, Result, SupportedFormat, inspect_image, load_dynamic_image};

#[derive(Debug, Clone, Copy)]
pub struct ConvertOptions {
    pub format: SupportedFormat,
    pub quality: u8,
    pub background: [u8; 4],
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            format: SupportedFormat::Jpeg,
            quality: 92,
            background: [255, 255, 255, 255],
        }
    }
}

impl ConvertOptions {
    pub fn normalized_quality(self) -> u8 {
        self.quality.clamp(1, 100)
    }
}

#[derive(Debug, Clone)]
pub struct ConvertResult {
    pub input: PathBuf,
    pub output: PathBuf,
    pub format: SupportedFormat,
    pub width: u32,
    pub height: u32,
}

pub fn convert_image(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: ConvertOptions,
) -> Result<ConvertResult> {
    let input = input.as_ref();
    let output = output.as_ref();
    let image = load_dynamic_image(input)?;
    let prepared = prepare_for_format(&image, options.format, options.background);
    encode_image(
        &prepared,
        output,
        options.format,
        options.normalized_quality(),
    )?;
    let info = inspect_image(output)?;

    Ok(ConvertResult {
        input: input.to_path_buf(),
        output: output.to_path_buf(),
        format: options.format,
        width: info.width,
        height: info.height,
    })
}

pub fn save_dynamic_image(
    image: &DynamicImage,
    output: impl AsRef<Path>,
    options: ConvertOptions,
) -> Result<ConvertResult> {
    let output = output.as_ref();
    let prepared = prepare_for_format(image, options.format, options.background);
    encode_image(
        &prepared,
        output,
        options.format,
        options.normalized_quality(),
    )?;
    let info = inspect_image(output)?;

    Ok(ConvertResult {
        input: PathBuf::new(),
        output: output.to_path_buf(),
        format: options.format,
        width: info.width,
        height: info.height,
    })
}

pub fn flatten_alpha(image: &DynamicImage, background: [u8; 4]) -> DynamicImage {
    let rgba = image.to_rgba8();
    let bg = Rgba(background);
    let mut out = image::RgbImage::new(rgba.width(), rgba.height());

    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = f32::from(pixel[3]) / 255.0;
        let inv_alpha = 1.0 - alpha;
        let red = (f32::from(pixel[0]) * alpha + f32::from(bg[0]) * inv_alpha).round() as u8;
        let green = (f32::from(pixel[1]) * alpha + f32::from(bg[1]) * inv_alpha).round() as u8;
        let blue = (f32::from(pixel[2]) * alpha + f32::from(bg[2]) * inv_alpha).round() as u8;
        out.put_pixel(x, y, image::Rgb([red, green, blue]));
    }

    DynamicImage::ImageRgb8(out)
}

pub fn prepare_for_format(
    image: &DynamicImage,
    format: SupportedFormat,
    background: [u8; 4],
) -> DynamicImage {
    if format.supports_alpha() {
        image.clone()
    } else {
        flatten_alpha(image, background)
    }
}

pub fn encode_image(
    image: &DynamicImage,
    output: impl AsRef<Path>,
    format: SupportedFormat,
    quality: u8,
) -> Result<()> {
    let output = output.as_ref();
    let file = File::create(output)?;

    if format == SupportedFormat::Jpeg {
        let rgb = image.to_rgb8();
        let encoder = JpegEncoder::new_with_quality(BufWriter::new(file), quality.clamp(1, 100));
        encoder.write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )?;
        return Ok(());
    }

    let mut writer = BufWriter::new(file);
    image.write_to(&mut writer, format.image_format())?;
    Ok(())
}

pub fn encode_jpeg_bytes(image: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
    let rgb = image.to_rgb8();
    let mut bytes = Vec::new();
    let cursor = Cursor::new(&mut bytes);
    let encoder = JpegEncoder::new_with_quality(cursor, quality.clamp(1, 100));
    encoder.write_image(
        rgb.as_raw(),
        rgb.width(),
        rgb.height(),
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(bytes)
}

pub fn require_output_format(path: impl AsRef<Path>) -> Result<SupportedFormat> {
    let path = path.as_ref();
    SupportedFormat::from_path(path)
        .ok_or_else(|| PhotoError::UnsupportedFormat(path.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, RgbaImage};

    use super::{ConvertOptions, flatten_alpha};
    use crate::SupportedFormat;

    #[test]
    fn clamps_quality() {
        assert_eq!(
            ConvertOptions {
                quality: 0,
                ..Default::default()
            }
            .normalized_quality(),
            1
        );
        assert_eq!(
            ConvertOptions {
                quality: 255,
                ..Default::default()
            }
            .normalized_quality(),
            100
        );
    }

    #[test]
    fn flattens_transparent_pixel_against_background() {
        let mut image = RgbaImage::new(1, 1);
        image.put_pixel(0, 0, image::Rgba([255, 0, 0, 128]));
        let flattened = flatten_alpha(&DynamicImage::ImageRgba8(image), [255, 255, 255, 255]);
        let rgb = flattened.to_rgb8();
        let pixel = rgb.get_pixel(0, 0);
        assert!(pixel[0] >= 250);
        assert!(pixel[1] >= 120);
        assert!(pixel[2] >= 120);
    }

    #[test]
    fn jpeg_does_not_support_alpha() {
        assert!(!SupportedFormat::Jpeg.supports_alpha());
        assert!(SupportedFormat::Png.supports_alpha());
    }
}
