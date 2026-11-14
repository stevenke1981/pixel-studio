use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use image::{DynamicImage, Rgba};
use serde_json::json;
use studio_core::{
    AutoContrast, BatchOptions, Blur, BrightnessAdjust, C2paAddOptions, C2paManifestDraft,
    C2paRemovalOptions, ContrastAdjust, ConvertOptions, ConvertResult, ExposureAdjust,
    FilterPipeline, FlipHorizontal, FlipVertical, GifOptions, Grayscale, HueAdjust, ImageInfo,
    Invert, PanoramaMode, PanoramaOptions, Posterize, Rotate180, RotateLeft, RotateRight,
    SaturationAdjust, Sepia, Sharpen, SupportedFormat, TemperatureAdjust, TintAdjust,
    TransformPipeline, Vignette, blur_region, canvas_resize, collect_supported_files,
    convert_image_with_options, crop_region, inspect_c2pa, inspect_image, load_dynamic_image,
    process_batch, process_c2pa_add_batch, process_c2pa_removal_batch, remove_c2pa_manifest,
    resize_exact, resize_to_fit, save_dynamic_image, write_animated_gif, write_panorama_jpeg,
};

#[derive(Debug, Parser)]
#[command(name = "studio-cli")]
#[command(about = "Pixel Studio command-line utilities", version)]
struct Cli {
    /// Emit structured JSON output for automation and AI agents.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Detect text offline and redact sensitive data (solid box by default).
    Redact {
        input: PathBuf,
        /// Output file (default: pixelstudio_YYYYMMDD_HHMMSS.<ext> next to the input).
        output: Option<PathBuf>,
        /// solid | mosaic | blur
        #[arg(long, default_value = "solid")]
        style: String,
        #[arg(long, default_value_t = 4)]
        padding: u32,
        /// Also redact every box containing this keyword.
        #[arg(long)]
        keyword: Vec<String>,
        /// Redact all detected text, not only sensitive matches.
        #[arg(long)]
        all: bool,
        /// Keep EXIF/ICC metadata from the source (stripped by default).
        #[arg(long)]
        keep_metadata: bool,
        #[arg(long, default_value_t = 92)]
        quality: u8,
    },
    /// Inspect image dimensions, format, and file size.
    Info { input: PathBuf },
    /// List all supported image formats and their capabilities.
    Formats,
    /// Convert an image to another format.
    Convert {
        input: PathBuf,
        /// Output file (default: pixelstudio_YYYYMMDD_HHMMSS.<ext> next to the input).
        output: Option<PathBuf>,
        /// Target output format (inferred from output extension if omitted).
        #[arg(long)]
        format: Option<SupportedFormat>,
        #[arg(long, default_value_t = 92)]
        quality: u8,
        /// Flatten background color in hex (#RRGGBB or #RRGGBBAA) when target lacks alpha.
        #[arg(long, default_value = "#FFFFFFFF")]
        background: String,
        /// Encode lossless WebP when targeting webp.
        #[arg(long, default_value_t = false)]
        lossless: bool,
        /// Keep EXIF/ICC metadata from the source (stripped by default).
        #[arg(long, default_value_t = false)]
        keep_metadata: bool,
    },
    /// Apply composable transforms and filters in a single pass.
    Filter {
        input: PathBuf,
        /// Output file (default: pixelstudio_YYYYMMDD_HHMMSS.<ext> next to the input).
        output: Option<PathBuf>,
        /// Rotate clockwise in degrees (90, 180, or 270).
        #[arg(long)]
        rotate: Option<u16>,
        /// Flip horizontally.
        #[arg(long)]
        flip_h: bool,
        /// Flip vertically.
        #[arg(long)]
        flip_v: bool,
        /// Adjust brightness (-255..255).
        #[arg(long, allow_hyphen_values = true)]
        brightness: Option<i32>,
        /// Adjust contrast (-100.0..100.0).
        #[arg(long, allow_hyphen_values = true)]
        contrast: Option<f32>,
        /// Adjust saturation (-100.0..100.0).
        #[arg(long, allow_hyphen_values = true)]
        saturation: Option<f32>,
        /// Adjust exposure in stops (e.g. -2.0..2.0).
        #[arg(long, allow_hyphen_values = true)]
        exposure: Option<f32>,
        /// Shift hue in degrees (-180.0..180.0).
        #[arg(long, allow_hyphen_values = true)]
        hue: Option<f32>,
        /// Apply sepia tone (0.0..1.0).
        #[arg(long)]
        sepia: Option<f32>,
        /// Apply edge vignette (0.0..1.0).
        #[arg(long)]
        vignette: Option<f32>,
        /// Adjust color temperature (-100.0 cooler .. 100.0 warmer).
        #[arg(long, allow_hyphen_values = true)]
        temperature: Option<f32>,
        /// Adjust green/magenta tint (-100.0 green .. 100.0 magenta).
        #[arg(long, allow_hyphen_values = true)]
        tint: Option<f32>,
        /// Stretch histogram per channel (auto levels).
        #[arg(long)]
        auto_contrast: bool,
        /// Posterize to N levels per channel (2..=32).
        #[arg(long)]
        posterize: Option<u8>,
        /// Gaussian blur sigma (> 0).
        #[arg(long)]
        blur: Option<f32>,
        /// Unsharp mask sigma (> 0).
        #[arg(long)]
        sharpen: Option<f32>,
        /// Unsharp mask threshold (used with --sharpen).
        #[arg(long, default_value_t = 5)]
        sharpen_threshold: i32,
        /// Convert to grayscale.
        #[arg(long)]
        grayscale: bool,
        /// Invert RGB channels.
        #[arg(long)]
        invert: bool,
        #[arg(long, default_value_t = 92)]
        quality: u8,
    },
    /// Resize an image (exact, fit within bounds, or canvas resize).
    Resize {
        input: PathBuf,
        /// Output file (default: pixelstudio_YYYYMMDD_HHMMSS.<ext> next to the input).
        output: Option<PathBuf>,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
        /// Preserve aspect ratio within width x height.
        #[arg(long, conflicts_with = "canvas")]
        fit: bool,
        /// Resize canvas without scaling existing pixels (top-left anchored).
        #[arg(long, conflicts_with = "fit")]
        canvas: bool,
        /// Canvas fill color in hex (#RRGGBB or #RRGGBBAA) when --canvas is set.
        #[arg(long, default_value = "#FFFFFFFF")]
        background: String,
        #[arg(long, default_value_t = 92)]
        quality: u8,
    },
    /// Crop a rectangular region.
    Crop {
        input: PathBuf,
        /// Output file (default: pixelstudio_YYYYMMDD_HHMMSS.<ext> next to the input).
        output: Option<PathBuf>,
        #[arg(long)]
        x: u32,
        #[arg(long)]
        y: u32,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
        #[arg(long, default_value_t = 92)]
        quality: u8,
    },
    /// Blur a rectangular region while preserving outside pixels.
    BlurRegion {
        input: PathBuf,
        /// Output file (default: pixelstudio_YYYYMMDD_HHMMSS.<ext> next to the input).
        output: Option<PathBuf>,
        #[arg(long)]
        x: u32,
        #[arg(long)]
        y: u32,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
        #[arg(long, default_value_t = 12.0)]
        sigma: f32,
        #[arg(long, default_value_t = 92)]
        quality: u8,
    },
    /// Convert an image to a 2:1 equirectangular JPEG with GPano XMP metadata.
    Pano360 {
        input: PathBuf,
        /// Output file (default: pixelstudio_YYYYMMDD_HHMMSS.<ext> next to the input).
        output: Option<PathBuf>,
        #[arg(long, default_value = "pad")]
        mode: PanoramaMode,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long, default_value_t = 92)]
        quality: u8,
    },
    /// Batch-convert a directory of images with optional resize and watermark.
    Batch {
        input_dir: PathBuf,
        output_dir: PathBuf,
        #[arg(long)]
        format: SupportedFormat,
        #[arg(long, default_value_t = 92)]
        quality: u8,
        /// Flatten background color in hex (#RRGGBB or #RRGGBBAA) when target lacks alpha.
        #[arg(long, default_value = "#FFFFFFFF")]
        background: String,
        /// Encode lossless WebP when targeting webp.
        #[arg(long, default_value_t = false)]
        lossless: bool,
        #[arg(long)]
        max_width: Option<u32>,
        #[arg(long)]
        max_height: Option<u32>,
        #[arg(long)]
        watermark_text: Option<String>,
        #[arg(long, default_value_t = 0.45)]
        watermark_opacity: f32,
        #[arg(long, default_value_t = 48.0)]
        watermark_font_size: f32,
    },
    /// Inspect C2PA content credentials in an image file.
    C2paInfo { input: PathBuf },
    /// Remove C2PA manifests from all supported images in a directory.
    C2paRemove {
        input_dir: PathBuf,
        output_dir: PathBuf,
    },
    /// Remove C2PA manifest from a single image file into a new copy.
    C2paRemoveFile { input: PathBuf, output: PathBuf },
    /// Sign and embed C2PA manifests into all supported images in a directory.
    C2paAdd {
        input_dir: PathBuf,
        output_dir: PathBuf,
        #[arg(long, default_value = "Pixel Studio Batch")]
        title: String,
        #[arg(long, default_value = "Pixel Studio")]
        creator: String,
        #[arg(long, default_value = "Batch C2PA manifest added")]
        action: String,
    },
    /// Assemble a directory of images into an animated GIF.
    Gif {
        input_dir: PathBuf,
        output: PathBuf,
        #[arg(long, default_value_t = 120)]
        delay_ms: u32,
        #[arg(long, default_value_t = true)]
        repeat: bool,
        #[arg(long)]
        max_width: Option<u32>,
        #[arg(long)]
        max_height: Option<u32>,
    },
}

/// Resolve an optional output path to a timestamped default next to the input.
fn resolve_output(output: Option<PathBuf>, input: &std::path::Path, ext: &str) -> PathBuf {
    output.unwrap_or_else(|| {
        let dir = input
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        studio_core::naming::timestamp_path_in(dir, ext)
    })
}

fn input_ext(input: &std::path::Path) -> String {
    input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase()
}

fn parse_rgba_hex(hex: &str) -> Result<Rgba<u8>> {
    let trimmed = hex.trim().trim_start_matches('#');
    match trimmed.len() {
        6 => {
            let r = u8::from_str_radix(&trimmed[0..2], 16)?;
            let g = u8::from_str_radix(&trimmed[2..4], 16)?;
            let b = u8::from_str_radix(&trimmed[4..6], 16)?;
            Ok(Rgba([r, g, b, 255]))
        }
        8 => {
            let r = u8::from_str_radix(&trimmed[0..2], 16)?;
            let g = u8::from_str_radix(&trimmed[2..4], 16)?;
            let b = u8::from_str_radix(&trimmed[4..6], 16)?;
            let a = u8::from_str_radix(&trimmed[6..8], 16)?;
            Ok(Rgba([r, g, b, a]))
        }
        _ => anyhow::bail!("expected #RRGGBB or #RRGGBBAA hex color, got '{hex}'"),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let json_out = cli.json;

    match cli.command {
        Command::Redact {
            input,
            output,
            style,
            padding,
            keyword,
            all,
            keep_metadata,
            quality,
        } => {
            let output = resolve_output(output, &input, &input_ext(&input));
            use studio_core::redact::{RedactStyle, apply_redactions, select_by_keyword};
            let image = load_dynamic_image(&input)?;
            let dir = studio_core::ocr::ensure_models()?;
            let detector = studio_core::ocr::TextDetector::load(&dir)?;
            let mut boxes = detector.detect(&image)?;
            for k in &keyword {
                select_by_keyword(&mut boxes, k);
            }
            if all {
                boxes.iter_mut().for_each(|b| b.selected = true);
            }
            if !json_out {
                for b in &boxes {
                    println!(
                        "{} {:>4},{:<4} {:>4}x{:<3} {:<8} {}",
                        if b.selected { "[x]" } else { "[ ]" },
                        b.x,
                        b.y,
                        b.width,
                        b.height,
                        b.kind.map(|k| k.label()).unwrap_or("-"),
                        b.text
                    );
                }
            }
            let style_enum = match style.as_str() {
                "mosaic" => RedactStyle::Mosaic(12),
                "blur" => RedactStyle::Blur(10.0),
                _ => RedactStyle::default(),
            };
            let (out, n) = apply_redactions(&image, &boxes, style_enum, padding);
            let format = studio_core::convert::require_output_format(&output)?;
            save_dynamic_image(
                &out,
                &output,
                ConvertOptions {
                    format,
                    quality,
                    ..Default::default()
                },
            )?;
            if keep_metadata {
                studio_core::metadata::copy_metadata(&input, &output)?;
            }
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "input": input,
                        "output": output,
                        "detected": boxes.len(),
                        "redacted": n,
                        "style": style,
                    }))?
                );
            } else {
                println!("detected {} text lines, redacted {n}", boxes.len());
            }
        }
        Command::Info { input } => {
            let info = inspect_image(input)?;
            if json_out {
                println!("{}", serde_json::to_string_pretty(&info_json_value(&info))?);
            } else {
                println!(
                    "{}: {}x{} {} {} bytes",
                    info.path.display(),
                    info.width,
                    info.height,
                    info.format,
                    info.file_size
                );
            }
        }
        Command::Formats => {
            if json_out {
                println!("{}", serde_json::to_string_pretty(&formats_json_value())?);
            } else {
                for fmt in SupportedFormat::ALL {
                    println!(
                        "{:<5} ext={:<12} mime={:<14} alpha={:<5} quality={:<5} c2pa={:<5} exif_icc={}",
                        fmt.label(),
                        fmt.extensions().join(","),
                        fmt.mime_type(),
                        fmt.supports_alpha(),
                        fmt.supports_quality(),
                        fmt.supports_c2pa(),
                        fmt.supports_exif_icc(),
                    );
                }
            }
        }
        Command::Convert {
            input,
            output,
            format,
            quality,
            background,
            lossless,
            keep_metadata,
        } => {
            let (result, effective_q) = run_convert(
                input,
                output,
                format,
                quality,
                &background,
                lossless,
                keep_metadata,
            )?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&convert_json_value(&result, effective_q))?
                );
            } else {
                println!(
                    "converted {} -> {} ({}x{}, {}, {} bytes)",
                    result.input.display(),
                    result.output.display(),
                    result.width,
                    result.height,
                    result.format,
                    result.file_size
                );
            }
        }
        Command::Filter {
            input,
            output,
            rotate,
            flip_h,
            flip_v,
            brightness,
            contrast,
            saturation,
            exposure,
            hue,
            sepia,
            vignette,
            temperature,
            tint,
            auto_contrast,
            posterize,
            blur,
            sharpen,
            sharpen_threshold,
            grayscale,
            invert,
            quality,
        } => {
            let output = resolve_output(output, &input, &input_ext(&input));
            let image = load_dynamic_image(&input)?;
            let mut transforms = TransformPipeline::new();
            let mut filters = FilterPipeline::new();

            if let Some(deg) = rotate {
                match deg {
                    90 => transforms.push(Box::new(RotateRight)),
                    180 => filters.push(Box::new(Rotate180)),
                    270 => transforms.push(Box::new(RotateLeft)),
                    _ => anyhow::bail!("--rotate must be 90, 180, or 270 (got {deg})"),
                }
            }
            if flip_h {
                transforms.push(Box::new(FlipHorizontal));
            }
            if flip_v {
                transforms.push(Box::new(FlipVertical));
            }
            if let Some(value) = brightness {
                transforms.push(Box::new(BrightnessAdjust { value }));
            }
            if let Some(value) = contrast {
                transforms.push(Box::new(ContrastAdjust { value }));
            }
            if let Some(value) = saturation {
                transforms.push(Box::new(SaturationAdjust { value }));
            }
            if let Some(stops) = exposure {
                transforms.push(Box::new(ExposureAdjust { stops }));
            }
            if let Some(degrees) = hue {
                filters.push(Box::new(HueAdjust { degrees }));
            }
            if let Some(strength) = sepia {
                filters.push(Box::new(Sepia { strength }));
            }
            if let Some(strength) = vignette {
                filters.push(Box::new(Vignette { strength }));
            }
            if let Some(amount) = temperature {
                filters.push(Box::new(TemperatureAdjust { amount }));
            }
            if let Some(amount) = tint {
                filters.push(Box::new(TintAdjust { amount }));
            }
            if auto_contrast {
                filters.push(Box::new(AutoContrast));
            }
            if let Some(levels) = posterize {
                filters.push(Box::new(Posterize { levels }));
            }
            if let Some(sigma) = blur {
                transforms.push(Box::new(Blur { sigma }));
            }
            if let Some(sigma) = sharpen {
                transforms.push(Box::new(Sharpen {
                    sigma,
                    threshold: sharpen_threshold,
                }));
            }
            if grayscale {
                transforms.push(Box::new(Grayscale));
            }
            if invert {
                transforms.push(Box::new(Invert));
            }

            let step_count = transforms.len() + filters.len();
            let transformed = transforms.apply_all(&image);
            let filtered = filters.apply_all(&transformed);
            let format = output_format(&output)?;
            let result = save_dynamic_image(
                &filtered,
                &output,
                ConvertOptions {
                    format,
                    quality,
                    background: [255, 255, 255, 255],
                },
            )?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "input": input,
                        "output": result.output,
                        "width": result.width,
                        "height": result.height,
                        "format": result.format.to_string(),
                        "steps_applied": step_count,
                    }))?
                );
            } else {
                println!(
                    "filtered {} -> {} ({}x{}, {}, {} steps)",
                    input.display(),
                    result.output.display(),
                    result.width,
                    result.height,
                    result.format,
                    step_count
                );
            }
        }
        Command::Resize {
            input,
            output,
            width,
            height,
            fit,
            canvas,
            background,
            quality,
        } => {
            if width == 0 || height == 0 {
                anyhow::bail!("width and height must be greater than zero");
            }
            let output = resolve_output(output, &input, &input_ext(&input));
            let image = load_dynamic_image(&input)?;
            let mode = if canvas {
                "canvas"
            } else if fit {
                "fit"
            } else {
                "exact"
            };
            let resized = if canvas {
                let bg = parse_rgba_hex(&background)?;
                DynamicImage::ImageRgba8(canvas_resize(&image.to_rgba8(), width, height, bg))
            } else if fit {
                resize_to_fit(&image, width, height)
            } else {
                resize_exact(&image, width, height)
            };
            let format = output_format(&output)?;
            let result = save_dynamic_image(
                &resized,
                &output,
                ConvertOptions {
                    format,
                    quality,
                    background: [255, 255, 255, 255],
                },
            )?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "input": input,
                        "output": result.output,
                        "width": result.width,
                        "height": result.height,
                        "mode": mode,
                        "format": result.format.to_string(),
                    }))?
                );
            } else {
                println!(
                    "resized ({mode}) {} -> {} ({}x{}, {})",
                    input.display(),
                    result.output.display(),
                    result.width,
                    result.height,
                    result.format
                );
            }
        }
        Command::Crop {
            input,
            output,
            x,
            y,
            width,
            height,
            quality,
        } => {
            let output = resolve_output(output, &input, &input_ext(&input));
            let image = load_dynamic_image(&input)?;
            let cropped = crop_region(&image, x, y, width, height)?;
            let format = output_format(&output)?;
            let result = save_dynamic_image(
                &cropped,
                &output,
                ConvertOptions {
                    format,
                    quality,
                    background: [255, 255, 255, 255],
                },
            )?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "input": input,
                        "output": result.output,
                        "width": result.width,
                        "height": result.height,
                        "format": result.format.to_string(),
                    }))?
                );
            } else {
                println!(
                    "cropped {} -> {} ({}x{}, {})",
                    input.display(),
                    result.output.display(),
                    result.width,
                    result.height,
                    result.format
                );
            }
        }
        Command::BlurRegion {
            input,
            output,
            x,
            y,
            width,
            height,
            sigma,
            quality,
        } => {
            let output = resolve_output(output, &input, &input_ext(&input));
            let image = load_dynamic_image(&input)?;
            let blurred = blur_region(&image, x, y, width, height, sigma)?;
            let format = output_format(&output)?;
            let result = save_dynamic_image(
                &blurred,
                &output,
                ConvertOptions {
                    format,
                    quality,
                    background: [255, 255, 255, 255],
                },
            )?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "input": input,
                        "output": result.output,
                        "width": result.width,
                        "height": result.height,
                        "format": result.format.to_string(),
                    }))?
                );
            } else {
                println!(
                    "blurred region in {} -> {} ({}x{}, {})",
                    input.display(),
                    result.output.display(),
                    result.width,
                    result.height,
                    result.format
                );
            }
        }
        Command::Pano360 {
            input,
            output,
            mode,
            width,
            quality,
        } => {
            let output = resolve_output(output, &input, "jpg");
            let result = write_panorama_jpeg(
                input,
                output,
                PanoramaOptions {
                    mode,
                    target_width: width,
                    quality,
                    background: [0, 0, 0, 255],
                },
            )?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "output": result.output,
                        "width": result.width,
                        "height": result.height,
                        "mode": result.mode.to_string(),
                    }))?
                );
            } else {
                println!(
                    "wrote 360-compatible JPEG {} ({}x{}, mode={})",
                    result.output.display(),
                    result.width,
                    result.height,
                    result.mode
                );
            }
        }
        Command::Batch {
            input_dir,
            output_dir,
            format,
            quality,
            background,
            lossless,
            max_width,
            max_height,
            watermark_text,
            watermark_opacity,
            watermark_font_size,
        } => {
            std::fs::create_dir_all(&output_dir)?;
            let bg = parse_rgba_hex(&background)?.0;
            let effective_q = effective_quality(format, quality, lossless);
            let files = collect_supported_files(&input_dir);
            let summary = process_batch(
                &files,
                &BatchOptions {
                    output_dir,
                    convert: ConvertOptions {
                        format,
                        quality: effective_q,
                        background: bg,
                    },
                    max_width,
                    max_height,
                    watermark_text: watermark_text.filter(|text| !text.trim().is_empty()),
                    watermark_opacity,
                    watermark_font_size,
                },
            );
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "total": summary.total,
                        "succeeded": summary.succeeded,
                        "failed": summary.failed,
                    }))?
                );
            } else {
                println!(
                    "batch complete: {} total, {} succeeded, {} failed",
                    summary.total, summary.succeeded, summary.failed
                );
                for result in summary
                    .results
                    .iter()
                    .filter(|result| result.error.is_some())
                {
                    println!(
                        "failed {}: {}",
                        result.input.display(),
                        result.error.as_deref().unwrap_or("unknown error")
                    );
                }
            }
        }
        Command::C2paInfo { input } => {
            let info = inspect_c2pa(&input)?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "path": input,
                        "present": info.present,
                        "title": info.title,
                        "creator": info.creator,
                        "action": info.action,
                        "validation_state": info.validation_state,
                        "manifest_count": info.manifest_count,
                    }))?
                );
            } else if info.present {
                println!(
                    "{}: C2PA present (manifests={}, state={}, title={}, creator={}, action={})",
                    input.display(),
                    info.manifest_count,
                    info.validation_state.as_deref().unwrap_or("unknown"),
                    info.title.as_deref().unwrap_or("-"),
                    info.creator.as_deref().unwrap_or("-"),
                    info.action.as_deref().unwrap_or("-"),
                );
            } else {
                println!("{}: no C2PA manifest found", input.display());
            }
        }
        Command::C2paRemove {
            input_dir,
            output_dir,
        } => {
            std::fs::create_dir_all(&output_dir)?;
            let files = collect_supported_files(&input_dir);
            let summary = process_c2pa_removal_batch(
                &files,
                &C2paRemovalOptions {
                    output_dir: output_dir.clone(),
                },
            );
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "total": summary.total,
                        "succeeded": summary.succeeded,
                        "failed": summary.failed,
                    }))?
                );
            } else {
                println!(
                    "C2PA batch removal complete: {} total, {} succeeded, {} failed",
                    summary.total, summary.succeeded, summary.failed
                );
                for result in summary
                    .results
                    .iter()
                    .filter(|result| result.error.is_some())
                {
                    println!(
                        "failed {}: {}",
                        result.input.display(),
                        result.error.as_deref().unwrap_or("unknown error")
                    );
                }
            }
        }
        Command::C2paRemoveFile { input, output } => {
            remove_c2pa_manifest(&input, &output)?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "input": input,
                        "output": output,
                    }))?
                );
            } else {
                println!(
                    "removed C2PA manifest {} -> {}",
                    input.display(),
                    output.display()
                );
            }
        }
        Command::C2paAdd {
            input_dir,
            output_dir,
            title,
            creator,
            action,
        } => {
            std::fs::create_dir_all(&output_dir)?;
            let files = collect_supported_files(&input_dir);
            let summary = process_c2pa_add_batch(
                &files,
                &C2paAddOptions {
                    output_dir: output_dir.clone(),
                    draft: C2paManifestDraft {
                        title,
                        creator,
                        action,
                    },
                },
            );
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "total": summary.total,
                        "succeeded": summary.succeeded,
                        "failed": summary.failed,
                    }))?
                );
            } else {
                println!(
                    "C2PA batch add complete: {} total, {} succeeded, {} failed",
                    summary.total, summary.succeeded, summary.failed
                );
                for result in summary
                    .results
                    .iter()
                    .filter(|result| result.error.is_some())
                {
                    println!(
                        "failed {}: {}",
                        result.input.display(),
                        result.error.as_deref().unwrap_or("unknown error")
                    );
                }
            }
        }
        Command::Gif {
            input_dir,
            output,
            delay_ms,
            repeat,
            max_width,
            max_height,
        } => {
            let files = collect_supported_files(&input_dir);
            let result = write_animated_gif(
                &files,
                output,
                GifOptions {
                    delay_ms,
                    repeat,
                    max_width,
                    max_height,
                    background: [255, 255, 255, 255],
                },
            )?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "output": result.output,
                        "frame_count": result.frame_count,
                        "width": result.width,
                        "height": result.height,
                        "delay_ms": result.delay_ms,
                    }))?
                );
            } else {
                println!(
                    "wrote animated GIF {} ({} frames, {}x{}, {} ms/frame)",
                    result.output.display(),
                    result.frame_count,
                    result.width,
                    result.height,
                    result.delay_ms
                );
            }
        }
    }

    Ok(())
}

fn output_format(output: &std::path::Path) -> Result<SupportedFormat> {
    SupportedFormat::from_path(output)
        .ok_or_else(|| anyhow::anyhow!("unsupported output format: {}", output.display()))
}

fn resolve_convert_target(
    input: &std::path::Path,
    output: Option<PathBuf>,
    format: Option<SupportedFormat>,
) -> Result<(PathBuf, SupportedFormat)> {
    match (output, format) {
        (Some(out), Some(fmt)) => {
            if let Some(ext_fmt) = SupportedFormat::from_path(&out)
                && ext_fmt != fmt
            {
                anyhow::bail!(
                    "output path extension '.{}' ({}) conflicts with --format {}; use a matching extension or omit the extension",
                    out.extension().and_then(|e| e.to_str()).unwrap_or(""),
                    ext_fmt.label(),
                    fmt.label(),
                );
            }
            Ok(studio_core::resolve_save_target(&out, fmt))
        }
        (Some(out), None) => {
            let fmt = SupportedFormat::from_path(&out).ok_or_else(|| {
                anyhow::anyhow!(
                    "cannot infer output format from '{}'; pass --format or use a supported image extension",
                    out.display()
                )
            })?;
            Ok((out, fmt))
        }
        (None, Some(fmt)) => Ok((resolve_output(None, input, fmt.extension()), fmt)),
        (None, None) => anyhow::bail!(
            "missing target format: specify an output path with a supported extension or pass --format"
        ),
    }
}

fn effective_quality(format: SupportedFormat, quality: u8, lossless: bool) -> u8 {
    studio_core::effective_quality(format, quality, lossless)
}

fn run_convert(
    input: PathBuf,
    output: Option<PathBuf>,
    format: Option<SupportedFormat>,
    quality: u8,
    background: &str,
    lossless: bool,
    keep_metadata: bool,
) -> Result<(ConvertResult, u8)> {
    let (output, format) = resolve_convert_target(&input, output, format)?;
    let bg = parse_rgba_hex(background)?.0;
    let effective_q = effective_quality(format, quality, lossless);
    let result = convert_image_with_options(
        input,
        output,
        ConvertOptions {
            format,
            quality: effective_q,
            background: bg,
        },
        keep_metadata,
    )?;
    Ok((result, effective_q))
}

fn convert_json_value(result: &ConvertResult, effective_quality: u8) -> serde_json::Value {
    json!({
        "input": result.input,
        "output": result.output,
        "width": result.width,
        "height": result.height,
        "format": result.format.to_string(),
        "source_format": result.source_format.map(|f| f.to_string()),
        "file_size": result.file_size,
        "quality": result.format.supports_quality().then_some(effective_quality),
        "alpha_preserved": result.format.supports_alpha(),
        "metadata_kept": result.metadata_kept,
    })
}

fn info_json_value(info: &ImageInfo) -> serde_json::Value {
    json!({
        "path": info.path,
        "width": info.width,
        "height": info.height,
        "format": info.format.to_string(),
        "file_size": info.file_size,
        "alpha_supported": info.format.supports_alpha(),
    })
}

fn formats_json_value() -> serde_json::Value {
    let entries: Vec<_> = SupportedFormat::ALL
        .iter()
        .map(|fmt| {
            json!({
                "id": fmt.extension(),
                "extension": fmt.extension(),
                "extensions": fmt.extensions(),
                "label": fmt.label(),
                "mime_type": fmt.mime_type(),
                "supports_alpha": fmt.supports_alpha(),
                "supports_quality": fmt.supports_quality(),
                "supports_c2pa": fmt.supports_c2pa(),
                "supports_exif_icc": fmt.supports_exif_icc(),
            })
        })
        .collect();
    serde_json::Value::Array(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(prefix: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "studio_cli_{prefix}_{}_{}",
                std::process::id(),
                nanos
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn parse_rgba_hex_supports_6_and_8_digits() {
        assert_eq!(
            parse_rgba_hex("#112233").unwrap(),
            Rgba([0x11, 0x22, 0x33, 0xFF])
        );
        assert_eq!(
            parse_rgba_hex("11223380").unwrap(),
            Rgba([0x11, 0x22, 0x33, 0x80])
        );
        assert!(parse_rgba_hex("#123").is_err());
    }

    #[test]
    fn cli_parses_filter_and_resize_subcommands() {
        let cli = Cli::try_parse_from([
            "studio-cli",
            "--json",
            "filter",
            "in.png",
            "out.png",
            "--rotate",
            "180",
            "--brightness",
            "-15",
            "--sepia",
            "0.5",
            "--auto-contrast",
        ])
        .unwrap();
        assert!(cli.json);
        assert!(matches!(cli.command, Command::Filter { .. }));

        let resize_cli = Cli::try_parse_from([
            "studio-cli",
            "resize",
            "in.png",
            "--width",
            "800",
            "--height",
            "600",
            "--fit",
        ])
        .unwrap();
        assert!(matches!(resize_cli.command, Command::Resize { .. }));
    }

    #[test]
    fn cli_parses_convert_formats_and_batch_flags() {
        let c1 = Cli::try_parse_from(["studio-cli", "convert", "in.png", "out.jfif"]).unwrap();
        assert!(matches!(
            c1.command,
            Command::Convert {
                format: None,
                output: Some(_),
                lossless: false,
                keep_metadata: false,
                ..
            }
        ));

        let c2 =
            Cli::try_parse_from(["studio-cli", "convert", "in.png", "--format", "jfif"]).unwrap();
        assert!(matches!(
            c2.command,
            Command::Convert {
                format: Some(SupportedFormat::Jfif),
                output: None,
                ..
            }
        ));

        let c3 = Cli::try_parse_from([
            "studio-cli",
            "convert",
            "in.png",
            "out.jpg",
            "--background",
            "#000000FF",
            "--keep-metadata",
        ])
        .unwrap();
        assert!(matches!(
            c3.command,
            Command::Convert {
                format: None,
                keep_metadata: true,
                ..
            }
        ));

        let c4 = Cli::try_parse_from(["studio-cli", "convert", "in.png", "out.webp", "--lossless"])
            .unwrap();
        assert!(matches!(
            c4.command,
            Command::Convert { lossless: true, .. }
        ));

        let f_cli = Cli::try_parse_from(["studio-cli", "--json", "formats"]).unwrap();
        assert!(f_cli.json && matches!(f_cli.command, Command::Formats));

        let b_cli = Cli::try_parse_from([
            "studio-cli",
            "batch",
            "in_dir",
            "out_dir",
            "--format",
            "qoi",
            "--background",
            "#112233FF",
            "--lossless",
        ])
        .unwrap();
        assert!(matches!(
            b_cli.command,
            Command::Batch {
                format: SupportedFormat::Qoi,
                lossless: true,
                ..
            }
        ));

        for ext in [
            "jpg", "jpeg", "jfif", "png", "webp", "bmp", "tif", "tiff", "gif", "ico", "tga", "qoi",
        ] {
            assert!(ext.parse::<SupportedFormat>().is_ok(), "failed on {ext}");
        }
    }

    #[test]
    fn convert_command_infers_format_handles_lossless_webp_and_privacy_default() {
        let dir = TestDir::new("convert");
        let src = dir.path().join("src.png");
        let mut rgba = image::RgbaImage::from_pixel(8, 6, image::Rgba([0, 0, 0, 0]));
        rgba.put_pixel(3, 2, image::Rgba([30, 120, 210, 255]));
        DynamicImage::ImageRgba8(rgba.clone()).save(&src).unwrap();

        // Missing both --format and recognized output extension returns an error.
        assert!(run_convert(src.clone(), None, None, 92, "#FFFFFFFF", false, false).is_err());
        assert!(
            run_convert(
                src.clone(),
                Some(dir.path().join("noext")),
                None,
                92,
                "#FFFFFFFF",
                false,
                false
            )
            .is_err()
        );

        // Inferred .jfif from output path + custom background flatten.
        let jfif_out = dir.path().join("out.jfif");
        let (jfif_res, jfif_q) = run_convert(
            src.clone(),
            Some(jfif_out.clone()),
            None,
            90,
            "#000000FF",
            false,
            false,
        )
        .unwrap();
        assert_eq!(jfif_res.format, SupportedFormat::Jfif);
        assert_eq!(jfif_res.source_format, Some(SupportedFormat::Png));
        assert!(!jfif_res.metadata_kept);
        let jfif_json = convert_json_value(&jfif_res, jfif_q);
        assert_eq!(jfif_json["format"], "JFIF");
        assert_eq!(jfif_json["source_format"], "PNG");
        assert_eq!(jfif_json["quality"], 90);
        assert_eq!(jfif_json["alpha_preserved"], false);
        assert_eq!(jfif_json["metadata_kept"], false);
        assert!(jfif_json["file_size"].as_u64().unwrap() > 0);
        let black_flat = load_dynamic_image(&jfif_out).unwrap().to_rgb8();
        assert!(black_flat.get_pixel(0, 0)[0] < 10);

        // Explicit --format without output path generates timestamped .tga next to input.
        let (auto_res, _) = run_convert(
            src.clone(),
            None,
            Some(SupportedFormat::Tga),
            92,
            "#FFFFFFFF",
            false,
            false,
        )
        .unwrap();
        assert_eq!(auto_res.format, SupportedFormat::Tga);
        assert_eq!(
            auto_res.output.extension().and_then(|e| e.to_str()),
            Some("tga")
        );
        assert!(auto_res.output.exists());

        // --lossless produces exact lossless WebP even when quality defaults to 92.
        let webp_lossless = dir.path().join("exact.webp");
        let (webp_res, webp_q) = run_convert(
            src.clone(),
            Some(webp_lossless.clone()),
            None,
            92,
            "#FFFFFFFF",
            true,
            false,
        )
        .unwrap();
        assert_eq!(webp_q, 100);
        assert_eq!(convert_json_value(&webp_res, webp_q)["quality"], 100);
        assert_eq!(load_dynamic_image(&webp_lossless).unwrap().to_rgba8(), rgba);

        // Lossy WebP clamps quality <= 99 when --lossless is false and quality < 100.
        assert_eq!(effective_quality(SupportedFormat::Webp, 92, false), 92);
        assert_eq!(effective_quality(SupportedFormat::Webp, 100, false), 100);
        assert_eq!(effective_quality(SupportedFormat::Webp, 80, true), 100);

        // Verify keep_metadata flag toggles metadata_kept on supported JPEG->PNG conversion.
        let jpg_src = dir.path().join("exif_src.jpg");
        DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(rgba).to_rgb8())
            .save_with_format(&jpg_src, image::ImageFormat::Jpeg)
            .unwrap();
        let png_stripped = dir.path().join("stripped.png");
        let (stripped_res, _) = run_convert(
            jpg_src.clone(),
            Some(png_stripped),
            None,
            92,
            "#FFFFFFFF",
            false,
            false,
        )
        .unwrap();
        assert!(!stripped_res.metadata_kept);

        let png_kept = dir.path().join("kept.png");
        let (kept_res, _) =
            run_convert(jpg_src, Some(png_kept), None, 92, "#FFFFFFFF", false, true).unwrap();
        assert!(kept_res.metadata_kept);
    }

    #[test]
    fn convert_target_resolves_extensionless_and_rejects_conflicting_format() {
        let dir = TestDir::new("convert_target");
        let src = dir.path().join("in.png");
        DynamicImage::ImageRgba8(image::RgbaImage::new(2, 2))
            .save(&src)
            .unwrap();

        // Extensionless output with --format gets the format extension automatically.
        let bare = dir.path().join("converted_out");
        let (resolved, fmt) =
            resolve_convert_target(&src, Some(bare), Some(SupportedFormat::Webp)).unwrap();
        assert_eq!(resolved, dir.path().join("converted_out.webp"));
        assert_eq!(fmt, SupportedFormat::Webp);

        // Unrecognized custom suffix appends the format extension.
        let custom = dir.path().join("asset.v2");
        let (resolved_custom, _) =
            resolve_convert_target(&src, Some(custom), Some(SupportedFormat::Png)).unwrap();
        assert_eq!(resolved_custom, dir.path().join("asset.v2.png"));

        // Matching extension succeeds.
        let matched = dir.path().join("ok.jpg");
        let (resolved_ok, ok_fmt) =
            resolve_convert_target(&src, Some(matched.clone()), Some(SupportedFormat::Jpeg))
                .unwrap();
        assert_eq!(resolved_ok, matched);
        assert_eq!(ok_fmt, SupportedFormat::Jpeg);

        // Conflicting explicit extension vs --format errors clearly.
        let conflicting = dir.path().join("wrong.png");
        let err = resolve_convert_target(&src, Some(conflicting), Some(SupportedFormat::Jpeg))
            .unwrap_err();
        assert!(err.to_string().contains("conflicts with --format"));
    }

    #[test]
    fn formats_and_info_json_expose_all_ten_formats_and_alpha_flag() {
        let formats = formats_json_value();
        let arr = formats.as_array().unwrap();
        assert_eq!(arr.len(), 10);
        for item in arr {
            assert!(item["id"].is_string());
            assert!(item["extension"].is_string());
            assert!(item["extensions"].is_array());
            assert!(item["label"].is_string());
            assert!(item["mime_type"].is_string());
            assert!(item["supports_alpha"].is_boolean());
            assert!(item["supports_quality"].is_boolean());
            assert!(item["supports_c2pa"].is_boolean());
            assert!(item["supports_exif_icc"].is_boolean());
        }
        assert!(arr.iter().any(|x| x["id"] == "tga"));
        assert!(arr.iter().any(|x| x["id"] == "qoi"));
        assert!(arr.iter().any(|x| x["id"] == "jfif"));

        let dir = TestDir::new("formats_info");
        let path = dir.path().join("sample.qoi");
        DynamicImage::ImageRgba8(image::RgbaImage::new(4, 3))
            .save(&path)
            .unwrap();
        let info = inspect_image(&path).unwrap();
        let info_json = info_json_value(&info);
        assert_eq!(info_json["format"], "QOI");
        assert_eq!(info_json["alpha_supported"], true);
        assert_eq!(info_json["width"], 4);
        assert_eq!(info_json["height"], 3);
    }
}
