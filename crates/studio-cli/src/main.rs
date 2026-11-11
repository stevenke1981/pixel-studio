use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use image::{DynamicImage, Rgba};
use serde_json::json;
use studio_core::{
    AutoContrast, BatchOptions, Blur, BrightnessAdjust, C2paAddOptions, C2paManifestDraft,
    C2paRemovalOptions, ContrastAdjust, ConvertOptions, ExposureAdjust, FilterPipeline,
    FlipHorizontal, FlipVertical, GifOptions, Grayscale, HueAdjust, Invert, PanoramaMode,
    PanoramaOptions, Posterize, Rotate180, RotateLeft, RotateRight, SaturationAdjust, Sepia,
    Sharpen, SupportedFormat, TemperatureAdjust, TintAdjust, TransformPipeline, Vignette,
    blur_region, canvas_resize, collect_supported_files, convert_image, crop_region, inspect_c2pa,
    inspect_image, load_dynamic_image, process_batch, process_c2pa_add_batch,
    process_c2pa_removal_batch, remove_c2pa_manifest, resize_exact, resize_to_fit,
    save_dynamic_image, write_animated_gif, write_panorama_jpeg,
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
    /// Convert an image to another format.
    Convert {
        input: PathBuf,
        /// Output file (default: pixelstudio_YYYYMMDD_HHMMSS.<ext> next to the input).
        output: Option<PathBuf>,
        #[arg(long)]
        format: SupportedFormat,
        #[arg(long, default_value_t = 92)]
        quality: u8,
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
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "path": info.path,
                        "width": info.width,
                        "height": info.height,
                        "format": info.format.to_string(),
                        "file_size": info.file_size,
                    }))?
                );
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
        Command::Convert {
            input,
            output,
            format,
            quality,
        } => {
            let output = resolve_output(output, &input, format.extension());
            let result = convert_image(
                input,
                output,
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
                        "input": result.input,
                        "output": result.output,
                        "width": result.width,
                        "height": result.height,
                        "format": result.format.to_string(),
                    }))?
                );
            } else {
                println!(
                    "converted {} -> {} ({}x{}, {})",
                    result.input.display(),
                    result.output.display(),
                    result.width,
                    result.height,
                    result.format
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
            max_width,
            max_height,
            watermark_text,
            watermark_opacity,
            watermark_font_size,
        } => {
            std::fs::create_dir_all(&output_dir)?;
            let files = collect_supported_files(&input_dir);
            let summary = process_batch(
                &files,
                &BatchOptions {
                    output_dir,
                    convert: ConvertOptions {
                        format,
                        quality,
                        background: [255, 255, 255, 255],
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
