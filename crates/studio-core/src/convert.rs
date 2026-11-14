use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, Write};
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageEncoder, Rgba};

use crate::{PhotoError, Result, SupportedFormat, load_dynamic_image_with_format};

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
    pub file_size: u64,
    pub source_format: Option<SupportedFormat>,
    pub metadata_kept: bool,
}

pub fn convert_image(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: ConvertOptions,
) -> Result<ConvertResult> {
    convert_image_with_options(input, output, options, false)
}

pub fn convert_image_with_options(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: ConvertOptions,
    keep_metadata: bool,
) -> Result<ConvertResult> {
    let input = input.as_ref();
    let output = output.as_ref();
    let (image, source_format) = load_dynamic_image_with_format(input)?;
    let prepared = prepare_for_format(&image, options.format, options.background);
    let quality = options.normalized_quality();
    let mut metadata_kept = false;

    crate::atomic_file::write_with(output, |staged| -> Result<()> {
        encode_staged(&prepared, staged, options.format, quality)?;
        if keep_metadata && options.format.supports_exif_icc() {
            metadata_kept = crate::metadata::copy_metadata(input, staged).map_err(|err| {
                PhotoError::InvalidInput(format!("failed to copy metadata: {err}"))
            })?;
        }
        Ok(())
    })?;

    let (width, height, file_size) = inspect_encoded_header(output, options.format)?;

    Ok(ConvertResult {
        input: input.to_path_buf(),
        output: output.to_path_buf(),
        format: options.format,
        width,
        height,
        file_size,
        source_format: Some(source_format),
        metadata_kept,
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
    let (width, height, file_size) = inspect_encoded_header(output, options.format)?;

    Ok(ConvertResult {
        input: PathBuf::new(),
        output: output.to_path_buf(),
        format: options.format,
        width,
        height,
        file_size,
        source_format: None,
        metadata_kept: false,
    })
}

fn inspect_encoded_header(output: &Path, format: SupportedFormat) -> Result<(u32, u32, u64)> {
    let file_size = std::fs::metadata(output)?.len();
    let mut reader = image::ImageReader::new(BufReader::new(File::open(output)?));
    reader.set_format(format.image_format());
    let (width, height) = reader.into_dimensions()?;
    Ok((width, height, file_size))
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
        normalize_for_format(image, format).into_owned()
    } else {
        flatten_alpha(image, background)
    }
}

fn normalize_for_format(
    image: &DynamicImage,
    format: SupportedFormat,
) -> std::borrow::Cow<'_, DynamicImage> {
    use std::borrow::Cow;
    match format {
        SupportedFormat::Png => match image {
            DynamicImage::ImageRgb32F(_) => Cow::Owned(DynamicImage::ImageRgb16(image.to_rgb16())),
            DynamicImage::ImageRgba32F(_) => {
                Cow::Owned(DynamicImage::ImageRgba16(image.to_rgba16()))
            }
            _ => Cow::Borrowed(image),
        },
        SupportedFormat::Tiff => match image {
            DynamicImage::ImageLumaA8(_) => Cow::Owned(DynamicImage::ImageRgba8(image.to_rgba8())),
            DynamicImage::ImageLumaA16(_) => {
                Cow::Owned(DynamicImage::ImageRgba16(image.to_rgba16()))
            }
            _ => Cow::Borrowed(image),
        },
        SupportedFormat::Webp | SupportedFormat::Qoi | SupportedFormat::Tga => match image {
            DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_) => Cow::Borrowed(image),
            _ if image.color().has_alpha() => {
                Cow::Owned(DynamicImage::ImageRgba8(image.to_rgba8()))
            }
            _ => Cow::Owned(DynamicImage::ImageRgb8(image.to_rgb8())),
        },
        SupportedFormat::Bmp => match image {
            DynamicImage::ImageRgb8(_) | DynamicImage::ImageLuma8(_) => Cow::Borrowed(image),
            _ => Cow::Owned(DynamicImage::ImageRgb8(image.to_rgb8())),
        },
        _ => Cow::Borrowed(image),
    }
}

pub fn encode_image(
    image: &DynamicImage,
    output: impl AsRef<Path>,
    format: SupportedFormat,
    quality: u8,
) -> Result<()> {
    let output = output.as_ref();
    crate::atomic_file::write_with(output, |staged| {
        encode_staged(image, staged, format, quality)
    })
}

fn encode_staged(
    image: &DynamicImage,
    output: &Path,
    format: SupportedFormat,
    quality: u8,
) -> Result<()> {
    let file = File::create(output)?;
    let mut writer = BufWriter::new(file);

    if matches!(format, SupportedFormat::Jpeg | SupportedFormat::Jfif) {
        let rgb = image.to_rgb8();
        let encoder = JpegEncoder::new_with_quality(&mut writer, quality.clamp(1, 100));
        encoder.write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )?;
        writer.flush()?;
        return Ok(());
    }

    match format {
        SupportedFormat::Webp if quality < 100 => {
            use std::io::Write;
            let rgba = image.to_rgba8();
            let encoded = webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height())
                .encode(f32::from(quality.clamp(1, 99)));
            writer.write_all(&encoded)?;
        }
        SupportedFormat::Ico => {
            // ICO frames are limited to 256x256.
            let img = if image.width() > 256 || image.height() > 256 {
                image.resize(256, 256, image::imageops::FilterType::Lanczos3)
            } else {
                image.clone()
            };
            DynamicImage::ImageRgba8(img.to_rgba8())
                .write_to(&mut writer, format.image_format())?;
        }
        SupportedFormat::Gif => {
            DynamicImage::ImageRgba8(image.to_rgba8())
                .write_to(&mut writer, format.image_format())?;
        }
        _ => {
            let normalized = normalize_for_format(image, format);
            normalized.write_to(&mut writer, format.image_format())?;
        }
    }
    writer.flush()?;
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
    use image::{DynamicImage, ImageBuffer, Luma, Rgb, Rgba, RgbaImage};

    use super::{
        ConvertOptions, convert_image, convert_image_with_options, flatten_alpha,
        save_dynamic_image,
    };
    use crate::{PhotoError, SupportedFormat, load_dynamic_image, metadata};

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
        assert!(!SupportedFormat::Jfif.supports_alpha());
        assert!(SupportedFormat::Png.supports_alpha());
    }

    #[test]
    fn qoi_and_tga_normalize_non_rgba8_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let luma = DynamicImage::ImageLuma8(ImageBuffer::from_pixel(6, 4, Luma([180u8])));
        let rgba16 = DynamicImage::ImageRgba16(ImageBuffer::from_pixel(
            6,
            4,
            Rgba([40000u16, 10000, 5000, 32768]),
        ));
        let rgb32f =
            DynamicImage::ImageRgb32F(ImageBuffer::from_pixel(6, 4, Rgb([0.5f32, 0.25, 0.75])));

        for (idx, img) in [luma, rgba16, rgb32f].iter().enumerate() {
            for fmt in [SupportedFormat::Qoi, SupportedFormat::Tga] {
                let out = dir.path().join(format!("norm_{idx}.{}", fmt.extension()));
                let res = save_dynamic_image(
                    img,
                    &out,
                    ConvertOptions {
                        format: fmt,
                        ..Default::default()
                    },
                )
                .unwrap();
                assert_eq!(res.format, fmt);
                assert_eq!((res.width, res.height), (6, 4));
                assert!(res.file_size > 0);
                let back = load_dynamic_image(&out).unwrap();
                assert_eq!((back.width(), back.height()), (6, 4));
            }
        }
    }

    #[test]
    fn configurable_alpha_composite_background_applies_to_non_alpha_formats() {
        let dir = tempfile::tempdir().unwrap();
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0])));
        let src = dir.path().join("transparent.png");
        img.save(&src).unwrap();

        for fmt in [
            SupportedFormat::Jpeg,
            SupportedFormat::Jfif,
            SupportedFormat::Bmp,
        ] {
            let black_out = dir.path().join(format!("black.{}", fmt.extension()));
            convert_image(
                &src,
                &black_out,
                ConvertOptions {
                    format: fmt,
                    quality: 95,
                    background: [0, 0, 0, 255],
                },
            )
            .unwrap();
            let black_px = load_dynamic_image(&black_out).unwrap().to_rgb8();
            assert!(black_px.get_pixel(0, 0)[0] < 10);
            assert!(black_px.get_pixel(0, 0)[1] < 10);
            assert!(black_px.get_pixel(0, 0)[2] < 10);

            let red_out = dir.path().join(format!("red.{}", fmt.extension()));
            convert_image(
                &src,
                &red_out,
                ConvertOptions {
                    format: fmt,
                    quality: 95,
                    background: [255, 0, 0, 255],
                },
            )
            .unwrap();
            let red_px = load_dynamic_image(&red_out).unwrap().to_rgb8();
            assert!(red_px.get_pixel(0, 0)[0] > 240);
            assert!(red_px.get_pixel(0, 0)[1] < 15);
            assert!(red_px.get_pixel(0, 0)[2] < 15);
        }
    }

    #[test]
    fn convert_image_with_options_respects_privacy_default_and_atomic_errors() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("with_exif.jpg");
        let base = DynamicImage::ImageRgba8(RgbaImage::from_pixel(12, 8, Rgba([40, 80, 120, 255])));
        save_dynamic_image(
            &base,
            &src,
            ConvertOptions {
                format: SupportedFormat::Jpeg,
                ..Default::default()
            },
        )
        .unwrap();
        {
            use img_parts::{Bytes, ImageEXIF, jpeg::Jpeg};
            let mut j = Jpeg::from_bytes(Bytes::from(std::fs::read(&src).unwrap())).unwrap();
            j.set_exif(Some(Bytes::from_static(b"Exif\0\0MM\0*\0\0\0\x08\0\0")));
            let mut out = Vec::new();
            j.encoder().write_to(&mut out).unwrap();
            std::fs::write(&src, out).unwrap();
        }
        assert!(metadata::has_exif(&src));

        // Privacy default: convert_image strips EXIF.
        let stripped = dir.path().join("stripped.jfif");
        let stripped_res = convert_image(
            &src,
            &stripped,
            ConvertOptions {
                format: SupportedFormat::Jfif,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!stripped_res.metadata_kept);
        assert_eq!(stripped_res.source_format, Some(SupportedFormat::Jpeg));
        assert!(stripped_res.file_size > 0);
        assert!(!metadata::has_exif(&stripped));

        // Opt-in metadata retention works across EXIF-capable formats and extensionless outputs.
        for (name, fmt) in [
            ("kept_jfif.jfif", SupportedFormat::Jfif),
            ("kept_png.png", SupportedFormat::Png),
            ("kept_webp.webp", SupportedFormat::Webp),
            ("extensionless_out", SupportedFormat::Png),
        ] {
            let out = dir.path().join(name);
            let res = convert_image_with_options(
                &src,
                &out,
                ConvertOptions {
                    format: fmt,
                    quality: 90,
                    ..Default::default()
                },
                true,
            )
            .unwrap();
            assert!(res.metadata_kept);
            assert_eq!((res.width, res.height), (12, 8));
            assert!(metadata::has_exif(&out));
        }

        // Unsupported metadata target (TGA) succeeds with metadata_kept = false.
        let tga_out = dir.path().join("out.tga");
        let tga_res = convert_image_with_options(
            &src,
            &tga_out,
            ConvertOptions {
                format: SupportedFormat::Tga,
                ..Default::default()
            },
            true,
        )
        .unwrap();
        assert!(!tga_res.metadata_kept);

        // Failed conversion preserves pre-existing destination file untouched.
        let existing = dir.path().join("existing.png");
        std::fs::write(&existing, b"original-bytes").unwrap();
        let bad_input = dir.path().join("not_an_image.txt");
        std::fs::write(&bad_input, b"plain text").unwrap();
        let err = convert_image_with_options(
            &bad_input,
            &existing,
            ConvertOptions {
                format: SupportedFormat::Png,
                ..Default::default()
            },
            false,
        )
        .unwrap_err();
        assert!(matches!(err, PhotoError::UnsupportedFormat(_)));
        assert_eq!(std::fs::read(&existing).unwrap(), b"original-bytes");
    }

    #[test]
    fn encodes_all_dynamic_image_color_types_to_every_supported_format() {
        let dir = tempfile::tempdir().unwrap();
        let variants: Vec<(&str, DynamicImage)> = vec![
            (
                "l8",
                DynamicImage::ImageLuma8(image::GrayImage::from_pixel(4, 4, image::Luma([120]))),
            ),
            (
                "la8",
                DynamicImage::ImageLumaA8(image::GrayAlphaImage::from_pixel(
                    4,
                    4,
                    image::LumaA([120, 200]),
                )),
            ),
            (
                "rgb8",
                DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
                    4,
                    4,
                    image::Rgb([10, 20, 30]),
                )),
            ),
            (
                "rgba8",
                DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                    4,
                    4,
                    image::Rgba([10, 20, 30, 180]),
                )),
            ),
            (
                "l16",
                DynamicImage::ImageLuma16(image::ImageBuffer::from_pixel(
                    4,
                    4,
                    image::Luma([32000u16]),
                )),
            ),
            (
                "la16",
                DynamicImage::ImageLumaA16(image::ImageBuffer::from_pixel(
                    4,
                    4,
                    image::LumaA([32000u16, 50000u16]),
                )),
            ),
            (
                "rgb16",
                DynamicImage::ImageRgb16(image::ImageBuffer::from_pixel(
                    4,
                    4,
                    image::Rgb([1000u16, 20000u16, 50000u16]),
                )),
            ),
            (
                "rgba16",
                DynamicImage::ImageRgba16(image::ImageBuffer::from_pixel(
                    4,
                    4,
                    image::Rgba([1000u16, 20000u16, 50000u16, 40000u16]),
                )),
            ),
            (
                "rgb32f",
                DynamicImage::ImageRgb32F(image::ImageBuffer::from_pixel(
                    4,
                    4,
                    image::Rgb([0.25f32, 0.5f32, 0.75f32]),
                )),
            ),
            (
                "rgba32f",
                DynamicImage::ImageRgba32F(image::ImageBuffer::from_pixel(
                    4,
                    4,
                    image::Rgba([0.25f32, 0.5f32, 0.75f32, 0.8f32]),
                )),
            ),
        ];

        for (label, img) in &variants {
            for fmt in SupportedFormat::ALL {
                let qualities: &[u8] = if fmt == SupportedFormat::Webp {
                    &[85, 100]
                } else {
                    &[90]
                };
                for &quality in qualities {
                    let out = dir
                        .path()
                        .join(format!("{label}_q{quality}.{}", fmt.extension()));
                    let res = save_dynamic_image(
                        img,
                        &out,
                        ConvertOptions {
                            format: fmt,
                            quality,
                            background: [255, 255, 255, 255],
                        },
                    )
                    .unwrap_or_else(|e| {
                        panic!("failed saving {label} -> {fmt:?} (q={quality}): {e}")
                    });
                    assert_eq!((res.width, res.height), (4, 4));
                }
            }
        }

        // Regression: grayscale() on RGBA produces La8, which must convert cleanly to TIFF & lossless WebP.
        let rgba = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            6,
            5,
            image::Rgba([80, 120, 200, 160]),
        ));
        let gray = crate::transform::grayscale(&rgba);
        for (fmt, q, name) in [
            (SupportedFormat::Tiff, 90, "gray.tif"),
            (SupportedFormat::Webp, 100, "gray_lossless.webp"),
        ] {
            let out = dir.path().join(name);
            let res = save_dynamic_image(
                &gray,
                &out,
                ConvertOptions {
                    format: fmt,
                    quality: q,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!((res.width, res.height), (6, 5));
        }

        // Regression: 32-bit float TIFF file -> PNG conversion via convert_image_with_options.
        let f32_tiff = dir.path().join("hdr.tif");
        variants
            .iter()
            .find(|(k, _)| *k == "rgba32f")
            .unwrap()
            .1
            .save_with_format(&f32_tiff, image::ImageFormat::Tiff)
            .unwrap();
        let png_out = dir.path().join("hdr.png");
        let res = convert_image_with_options(
            &f32_tiff,
            &png_out,
            ConvertOptions {
                format: SupportedFormat::Png,
                ..Default::default()
            },
            false,
        )
        .unwrap();
        assert_eq!((res.width, res.height), (4, 4));
    }
}
