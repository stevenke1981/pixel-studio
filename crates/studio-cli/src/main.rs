use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use image::{DynamicImage, Rgba};
use serde_json::json;
use studio_core::{
    AutoContrast, BatchOptions, Blur, BrightnessAdjust, C2paAddOptions, C2paManifestDraft,
    C2paRemovalOptions, ClarityAdjust, ColorBalance, ColorLook, ContrastAdjust, ConvertOptions,
    ConvertResult, EdgeDetect, Emboss, ExposureAdjust, FilmGrain, FilterPipeline, FlipHorizontal,
    FlipVertical, FrameCardOptions, GifOptions, Grayscale, HistogramData, HueAdjust, ImageInfo,
    Invert, LevelsAdjust, LookFilter, OilPaint, PanoramaMode, PanoramaOptions, Pixelate, Posterize,
    Rotate180, RotateLeft, RotateRight, SaturationAdjust, Sepia, ShadowsHighlights, Sharpen,
    SupportedFormat, TemperatureAdjust, Threshold, TiltShift, TintAdjust, TransformPipeline,
    TrimMode, TrimResult, VibranceAdjust, Vignette, add_border_frame, auto_trim, blur_region,
    canvas_resize, collect_supported_files, compute_histogram, convert_image_with_options,
    crop_region, inspect_c2pa, inspect_image, load_dynamic_image, process_batch,
    process_c2pa_add_batch, process_c2pa_removal_batch, remove_background_color,
    remove_c2pa_manifest, resize_exact, resize_to_fit, save_dynamic_image, write_animated_gif,
    write_panorama_jpeg,
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
    /// Compute RGB and BT.709 luminance histogram statistics for an image.
    Histogram {
        input: PathBuf,
        /// Include full 256-bin arrays (r, g, b, luma) in JSON output.
        #[arg(long)]
        bins: bool,
    },
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
        /// Remap tonal levels (`IN_BLACK,GAMMA,IN_WHITE` or `IN_BLACK,GAMMA,IN_WHITE,OUT_BLACK,OUT_WHITE`).
        #[arg(long)]
        levels: Option<String>,
        /// Adjust shadow tones (-100.0..=100.0).
        #[arg(long, allow_hyphen_values = true)]
        shadows: Option<f32>,
        /// Adjust highlight tones (-100.0..=100.0).
        #[arg(long, allow_hyphen_values = true)]
        highlights: Option<f32>,
        /// Adjust smart saturation / vibrance (-100.0..=100.0).
        #[arg(long, allow_hyphen_values = true)]
        vibrance: Option<f32>,
        /// Adjust midtone local contrast / clarity (-100.0..=100.0).
        #[arg(long, allow_hyphen_values = true)]
        clarity: Option<f32>,
        /// Shift RGB channel balance (`R,G,B`, each in -100.0..=100.0).
        #[arg(long, allow_hyphen_values = true)]
        color_balance: Option<String>,
        /// Convert to binary black & white at BT.709 luminance threshold (0..=255).
        #[arg(long)]
        threshold: Option<u8>,
        /// Apply a curated color look (vivid, cinematic, matte, moody-bw, warm, cool, high-key, cyberpunk).
        #[arg(long)]
        look: Option<String>,
        /// Add deterministic monochrome film grain (0.0..=1.0).
        #[arg(long)]
        grain: Option<f32>,
        /// Pixelate into square blocks (>= 2).
        #[arg(long)]
        pixelate: Option<u32>,
        /// Apply directional 3x3 emboss relief (0.0..=2.0).
        #[arg(long)]
        emboss: Option<f32>,
        /// Detect Sobel luminance edges.
        #[arg(long)]
        edge_detect: bool,
        /// Render dark ink edges on a white background (inverted Sobel edges).
        #[arg(long)]
        sketch: bool,
        /// Apply painterly oil-paint filter (radius 1..=5).
        #[arg(long)]
        oil_paint: Option<u32>,
        /// Apply miniature tilt-shift blur (`FOCUS_Y,BAND,SIGMA`, e.g. `0.5,0.25,6.0`).
        #[arg(long)]
        tilt_shift: Option<String>,
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
    /// Remove a solid background color by fading matching pixels to transparent.
    RemoveBg {
        input: PathBuf,
        /// Output file (default: pixelstudio_YYYYMMDD_HHMMSS.png next to the input).
        output: Option<PathBuf>,
        /// Target background color in hex (#RRGGBB or #RRGGBBAA).
        #[arg(long, default_value = "#FFFFFF")]
        color: String,
        /// Color distance tolerance (>= 0.0).
        #[arg(long, default_value_t = 32.0)]
        tolerance: f32,
        /// Smooth edge feather width (>= 0.0).
        #[arg(long, default_value_t = 8.0)]
        feather: f32,
        /// Only remove background regions connected to image edges.
        #[arg(long)]
        edge_only: bool,
        #[arg(long, default_value_t = 92)]
        quality: u8,
    },
    /// Auto-trim transparent or uniform corner borders from an image.
    Trim {
        input: PathBuf,
        /// Output file (default: pixelstudio_YYYYMMDD_HHMMSS.<ext> next to the input).
        output: Option<PathBuf>,
        /// Trim outer borders matching top-left corner color within tolerance (0..=255); defaults to transparent trim.
        #[arg(long)]
        corner_tolerance: Option<u8>,
        #[arg(long, default_value_t = 92)]
        quality: u8,
    },
    /// Frame an image on a padded card with rounded corners, border, and drop shadow.
    Frame {
        input: PathBuf,
        /// Output file (default: pixelstudio_YYYYMMDD_HHMMSS.<ext> next to the input).
        output: Option<PathBuf>,
        #[arg(long, default_value_t = 24)]
        padding: u32,
        #[arg(long, default_value_t = 16.0)]
        round_corners: f32,
        #[arg(long, default_value_t = 12.0)]
        shadow_sigma: f32,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        shadow_x: i32,
        #[arg(long, default_value_t = 6, allow_hyphen_values = true)]
        shadow_y: i32,
        #[arg(long, default_value = "#F4F6F9FF")]
        background: String,
        #[arg(long, default_value_t = 0)]
        border_width: u32,
        #[arg(long, default_value = "#D0D7DEFF")]
        border_color: String,
        #[arg(long, default_value_t = 92)]
        quality: u8,
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
        Command::Histogram { input, bins } => {
            let hist = run_histogram(&input)?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&histogram_json_value(&input, &hist, bins))?
                );
            } else {
                println!(
                    "{}: pixels={}, mean_luma={:.2}, std_dev_luma={:.2}, shadow_clip={:.2}%, highlight_clip={:.2}%",
                    input.display(),
                    hist.pixel_count,
                    hist.mean_luma,
                    hist.std_dev_luma,
                    hist.shadow_clip_pct,
                    hist.highlight_clip_pct
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
            levels,
            shadows,
            highlights,
            vibrance,
            clarity,
            color_balance,
            threshold,
            look,
            grain,
            pixelate,
            emboss,
            edge_detect,
            sketch,
            oil_paint,
            tilt_shift,
            quality,
        } => {
            let opts = FilterOptions {
                input: input.clone(),
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
                levels,
                shadows,
                highlights,
                vibrance,
                clarity,
                color_balance,
                threshold,
                look,
                grain,
                pixelate,
                emboss,
                edge_detect,
                sketch,
                oil_paint,
                tilt_shift,
                quality,
            };
            let (result, step_count) = run_filter(opts)?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&filter_json_value(&input, &result, step_count))?
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
        Command::RemoveBg {
            input,
            output,
            color,
            tolerance,
            feather,
            edge_only,
            quality,
        } => {
            let result = run_remove_bg(&RemoveBgOptions {
                input: &input,
                output,
                color: &color,
                tolerance,
                feather,
                edge_only,
                quality,
            })?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&remove_bg_json_value(
                        &input, &result, &color, tolerance, feather, edge_only,
                    ))?
                );
            } else {
                println!(
                    "removed background {} -> {} ({}x{}, {}, alpha={})",
                    input.display(),
                    result.output.display(),
                    result.width,
                    result.height,
                    result.format,
                    result.format.supports_alpha()
                );
            }
        }
        Command::Trim {
            input,
            output,
            corner_tolerance,
            quality,
        } => {
            let (result, trim_rect, mode) = run_trim(&input, output, corner_tolerance, quality)?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&trim_json_value(
                        &input, &result, trim_rect, mode,
                    ))?
                );
            } else {
                println!(
                    "trimmed {} -> {} (crop {}x{}+{}+{}, {})",
                    input.display(),
                    result.output.display(),
                    trim_rect.width,
                    trim_rect.height,
                    trim_rect.x,
                    trim_rect.y,
                    result.format
                );
            }
        }
        Command::Frame {
            input,
            output,
            padding,
            round_corners,
            shadow_sigma,
            shadow_x,
            shadow_y,
            background,
            border_width,
            border_color,
            quality,
        } => {
            let (result, card_opts) = run_frame(&FrameCliOptions {
                input: &input,
                output,
                padding,
                round_corners,
                shadow_sigma,
                shadow_x,
                shadow_y,
                background: &background,
                border_width,
                border_color: &border_color,
                quality,
            })?;
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&frame_json_value(&input, &result, &card_opts))?
                );
            } else {
                println!(
                    "framed {} -> {} ({}x{}, {})",
                    input.display(),
                    result.output.display(),
                    result.width,
                    result.height,
                    result.format
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

fn ensure_finite(value: f32, label: &str) -> Result<f32> {
    if value.is_finite() {
        Ok(value)
    } else {
        anyhow::bail!("{label} must be a finite number, got {value}");
    }
}

fn parse_finite_f32(raw: &str, label: &str) -> Result<f32> {
    let parsed = raw
        .parse::<f32>()
        .map_err(|e| anyhow::anyhow!("invalid {label} '{raw}': {e}"))?;
    ensure_finite(parsed, label)
}

fn parse_levels(spec: &str) -> Result<LevelsAdjust> {
    let parts: Vec<&str> = spec.split(',').map(str::trim).collect();
    let (in_black, gamma, in_white, out_black, out_white) = match parts.as_slice() {
        [ib, g, iw] => (
            ib.parse::<u8>()
                .map_err(|e| anyhow::anyhow!("invalid --levels in_black '{ib}': {e}"))?,
            parse_finite_f32(g, "--levels gamma")?,
            iw.parse::<u8>()
                .map_err(|e| anyhow::anyhow!("invalid --levels in_white '{iw}': {e}"))?,
            0u8,
            255u8,
        ),
        [ib, g, iw, ob, ow] => (
            ib.parse::<u8>()
                .map_err(|e| anyhow::anyhow!("invalid --levels in_black '{ib}': {e}"))?,
            parse_finite_f32(g, "--levels gamma")?,
            iw.parse::<u8>()
                .map_err(|e| anyhow::anyhow!("invalid --levels in_white '{iw}': {e}"))?,
            ob.parse::<u8>()
                .map_err(|e| anyhow::anyhow!("invalid --levels out_black '{ob}': {e}"))?,
            ow.parse::<u8>()
                .map_err(|e| anyhow::anyhow!("invalid --levels out_white '{ow}': {e}"))?,
        ),
        _ => anyhow::bail!(
            "--levels expects IN_BLACK,GAMMA,IN_WHITE or IN_BLACK,GAMMA,IN_WHITE,OUT_BLACK,OUT_WHITE, got '{spec}'"
        ),
    };
    if in_black >= in_white {
        anyhow::bail!("--levels requires in_black ({in_black}) < in_white ({in_white})");
    }
    if gamma <= 0.0 {
        anyhow::bail!("--levels gamma must be > 0.0, got {gamma}");
    }
    Ok(LevelsAdjust {
        in_black,
        gamma,
        in_white,
        out_black,
        out_white,
    })
}

fn parse_color_balance(spec: &str) -> Result<ColorBalance> {
    let parts: Vec<&str> = spec.split(',').map(str::trim).collect();
    let [r, g, b] = parts.as_slice() else {
        anyhow::bail!("--color-balance expects R,G,B (e.g. \"15,-5,-10\"), got '{spec}'");
    };
    Ok(ColorBalance {
        r_shift: parse_finite_f32(r, "--color-balance R")?,
        g_shift: parse_finite_f32(g, "--color-balance G")?,
        b_shift: parse_finite_f32(b, "--color-balance B")?,
    })
}

fn parse_tilt_shift(spec: &str) -> Result<TiltShift> {
    let parts: Vec<&str> = spec.split(',').map(str::trim).collect();
    let [fy, bw, s] = parts.as_slice() else {
        anyhow::bail!(
            "--tilt-shift expects FOCUS_Y,BAND,SIGMA (e.g. \"0.5,0.25,6.0\"), got '{spec}'"
        );
    };
    let focus_y = parse_finite_f32(fy, "--tilt-shift FOCUS_Y")?;
    let band_width = parse_finite_f32(bw, "--tilt-shift BAND")?;
    let blur_sigma = parse_finite_f32(s, "--tilt-shift SIGMA")?;
    if !(0.0..=1.0).contains(&focus_y) {
        anyhow::bail!("--tilt-shift focus_y must be in 0.0..=1.0, got {focus_y}");
    }
    if band_width <= 0.0 || band_width > 1.0 {
        anyhow::bail!("--tilt-shift band must be in (0.0, 1.0], got {band_width}");
    }
    if blur_sigma < 0.0 {
        anyhow::bail!("--tilt-shift sigma must be >= 0.0, got {blur_sigma}");
    }
    Ok(TiltShift {
        focus_y,
        band_width,
        blur_sigma,
    })
}

fn parse_color_look(name: &str) -> Result<ColorLook> {
    if let Some(look) = ColorLook::from_name(name) {
        return Ok(look);
    }
    let normalized: String = name
        .trim()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    match normalized.as_str() {
        "cinematic" => Ok(ColorLook::CinematicTealOrange),
        "matte" => Ok(ColorLook::MatteFilm),
        "warm" => Ok(ColorLook::WarmGolden),
        "cool" => Ok(ColorLook::CrispCool),
        "bw" | "moody" => Ok(ColorLook::MoodyBw),
        _ => anyhow::bail!(
            "unknown color look '{name}'; expected one of: vivid, cinematic (cinematic-teal-orange), matte (matte-film), moody-bw, warm (warm-golden), cool (crisp-cool), high-key, cyberpunk"
        ),
    }
}

fn run_histogram(input: &std::path::Path) -> Result<HistogramData> {
    let image = load_dynamic_image(input)?;
    Ok(compute_histogram(&image))
}

fn histogram_json_value(
    input: &std::path::Path,
    hist: &HistogramData,
    bins: bool,
) -> serde_json::Value {
    let mut obj = json!({
        "command": "histogram",
        "input": input,
        "pixel_count": hist.pixel_count,
        "mean_luma": hist.mean_luma,
        "std_dev_luma": hist.std_dev_luma,
        "shadow_clip_pct": hist.shadow_clip_pct,
        "highlight_clip_pct": hist.highlight_clip_pct,
    });
    if bins && let Some(map) = obj.as_object_mut() {
        map.insert("r".to_string(), json!(hist.r.to_vec()));
        map.insert("g".to_string(), json!(hist.g.to_vec()));
        map.insert("b".to_string(), json!(hist.b.to_vec()));
        map.insert("luma".to_string(), json!(hist.luma.to_vec()));
    }
    obj
}

#[derive(Debug)]
struct FilterOptions {
    input: PathBuf,
    output: Option<PathBuf>,
    rotate: Option<u16>,
    flip_h: bool,
    flip_v: bool,
    brightness: Option<i32>,
    contrast: Option<f32>,
    saturation: Option<f32>,
    exposure: Option<f32>,
    hue: Option<f32>,
    sepia: Option<f32>,
    vignette: Option<f32>,
    temperature: Option<f32>,
    tint: Option<f32>,
    auto_contrast: bool,
    posterize: Option<u8>,
    blur: Option<f32>,
    sharpen: Option<f32>,
    sharpen_threshold: i32,
    grayscale: bool,
    invert: bool,
    levels: Option<String>,
    shadows: Option<f32>,
    highlights: Option<f32>,
    vibrance: Option<f32>,
    clarity: Option<f32>,
    color_balance: Option<String>,
    threshold: Option<u8>,
    look: Option<String>,
    grain: Option<f32>,
    pixelate: Option<u32>,
    emboss: Option<f32>,
    edge_detect: bool,
    sketch: bool,
    oil_paint: Option<u32>,
    tilt_shift: Option<String>,
    quality: u8,
}

impl Default for FilterOptions {
    fn default() -> Self {
        Self {
            input: PathBuf::new(),
            output: None,
            rotate: None,
            flip_h: false,
            flip_v: false,
            brightness: None,
            contrast: None,
            saturation: None,
            exposure: None,
            hue: None,
            sepia: None,
            vignette: None,
            temperature: None,
            tint: None,
            auto_contrast: false,
            posterize: None,
            blur: None,
            sharpen: None,
            sharpen_threshold: 5,
            grayscale: false,
            invert: false,
            levels: None,
            shadows: None,
            highlights: None,
            vibrance: None,
            clarity: None,
            color_balance: None,
            threshold: None,
            look: None,
            grain: None,
            pixelate: None,
            emboss: None,
            edge_detect: false,
            sketch: false,
            oil_paint: None,
            tilt_shift: None,
            quality: 92,
        }
    }
}

fn run_filter(opts: FilterOptions) -> Result<(ConvertResult, usize)> {
    let output = resolve_output(opts.output, &opts.input, &input_ext(&opts.input));
    let image = load_dynamic_image(&opts.input)?;
    let mut transforms = TransformPipeline::new();
    let mut filters = FilterPipeline::new();

    if let Some(deg) = opts.rotate {
        match deg {
            90 => transforms.push(Box::new(RotateRight)),
            180 => filters.push(Box::new(Rotate180)),
            270 => transforms.push(Box::new(RotateLeft)),
            _ => anyhow::bail!("--rotate must be 90, 180, or 270 (got {deg})"),
        }
    }
    if opts.flip_h {
        transforms.push(Box::new(FlipHorizontal));
    }
    if opts.flip_v {
        transforms.push(Box::new(FlipVertical));
    }
    if let Some(value) = opts.brightness {
        transforms.push(Box::new(BrightnessAdjust { value }));
    }
    if let Some(value) = opts.contrast {
        transforms.push(Box::new(ContrastAdjust { value }));
    }
    if let Some(value) = opts.saturation {
        transforms.push(Box::new(SaturationAdjust { value }));
    }
    if let Some(stops) = opts.exposure {
        transforms.push(Box::new(ExposureAdjust { stops }));
    }
    if let Some(degrees) = opts.hue {
        filters.push(Box::new(HueAdjust { degrees }));
    }
    if let Some(strength) = opts.sepia {
        filters.push(Box::new(Sepia { strength }));
    }
    if let Some(strength) = opts.vignette {
        filters.push(Box::new(Vignette { strength }));
    }
    if let Some(amount) = opts.temperature {
        filters.push(Box::new(TemperatureAdjust { amount }));
    }
    if let Some(amount) = opts.tint {
        filters.push(Box::new(TintAdjust { amount }));
    }
    if opts.auto_contrast {
        filters.push(Box::new(AutoContrast));
    }
    if let Some(levels) = opts.posterize {
        filters.push(Box::new(Posterize { levels }));
    }
    if let Some(sigma) = opts.blur {
        transforms.push(Box::new(Blur { sigma }));
    }
    if let Some(sigma) = opts.sharpen {
        transforms.push(Box::new(Sharpen {
            sigma,
            threshold: opts.sharpen_threshold,
        }));
    }
    if opts.grayscale {
        transforms.push(Box::new(Grayscale));
    }
    if opts.invert {
        transforms.push(Box::new(Invert));
    }

    if let Some(ref spec) = opts.levels {
        filters.push(Box::new(parse_levels(spec)?));
    }
    if opts.shadows.is_some() || opts.highlights.is_some() {
        let shadows = opts
            .shadows
            .map(|v| ensure_finite(v, "--shadows"))
            .transpose()?
            .unwrap_or(0.0);
        let highlights = opts
            .highlights
            .map(|v| ensure_finite(v, "--highlights"))
            .transpose()?
            .unwrap_or(0.0);
        filters.push(Box::new(ShadowsHighlights {
            shadows,
            highlights,
        }));
    }
    if let Some(amount) = opts.vibrance {
        filters.push(Box::new(VibranceAdjust {
            amount: ensure_finite(amount, "--vibrance")?,
        }));
    }
    if let Some(amount) = opts.clarity {
        filters.push(Box::new(ClarityAdjust {
            amount: ensure_finite(amount, "--clarity")?,
        }));
    }
    if let Some(ref spec) = opts.color_balance {
        filters.push(Box::new(parse_color_balance(spec)?));
    }
    if let Some(ref name) = opts.look {
        filters.push(Box::new(LookFilter {
            look: parse_color_look(name)?,
        }));
    }
    if let Some(level) = opts.threshold {
        filters.push(Box::new(Threshold { level }));
    }
    if let Some(radius) = opts.oil_paint {
        if !(1..=5).contains(&radius) {
            anyhow::bail!("--oil-paint radius must be between 1 and 5 (got {radius})");
        }
        filters.push(Box::new(OilPaint { radius, levels: 16 }));
    }
    if let Some(ref spec) = opts.tilt_shift {
        filters.push(Box::new(parse_tilt_shift(spec)?));
    }
    if let Some(block_size) = opts.pixelate {
        if block_size < 2 {
            anyhow::bail!("--pixelate block size must be >= 2 (got {block_size})");
        }
        filters.push(Box::new(Pixelate { block_size }));
    }
    if let Some(strength) = opts.emboss {
        filters.push(Box::new(Emboss {
            strength: ensure_finite(strength, "--emboss")?,
        }));
    }
    if opts.edge_detect || opts.sketch {
        filters.push(Box::new(EdgeDetect {
            invert_edges: opts.sketch,
        }));
    }
    if let Some(strength) = opts.grain {
        filters.push(Box::new(FilmGrain {
            strength: ensure_finite(strength, "--grain")?,
        }));
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
            quality: opts.quality,
            background: [255, 255, 255, 255],
        },
    )?;
    Ok((result, step_count))
}

fn filter_json_value(
    input: &std::path::Path,
    result: &ConvertResult,
    step_count: usize,
) -> serde_json::Value {
    json!({
        "input": input,
        "output": result.output,
        "width": result.width,
        "height": result.height,
        "format": result.format.to_string(),
        "steps_applied": step_count,
    })
}

struct RemoveBgOptions<'a> {
    input: &'a std::path::Path,
    output: Option<PathBuf>,
    color: &'a str,
    tolerance: f32,
    feather: f32,
    edge_only: bool,
    quality: u8,
}

fn run_remove_bg(opts: &RemoveBgOptions<'_>) -> Result<ConvertResult> {
    let tolerance = ensure_finite(opts.tolerance, "--tolerance")?;
    let feather = ensure_finite(opts.feather, "--feather")?;
    if tolerance < 0.0 {
        anyhow::bail!("--tolerance must be >= 0.0, got {tolerance}");
    }
    if feather < 0.0 {
        anyhow::bail!("--feather must be >= 0.0, got {feather}");
    }
    let rgba = parse_rgba_hex(opts.color)?;
    let output = resolve_output(opts.output.clone(), opts.input, "png");
    let image = load_dynamic_image(opts.input)?;
    let cutout = remove_background_color(
        &image,
        [rgba[0], rgba[1], rgba[2]],
        tolerance,
        feather,
        opts.edge_only,
    );
    let format = output_format(&output)?;
    Ok(save_dynamic_image(
        &cutout,
        &output,
        ConvertOptions {
            format,
            quality: opts.quality,
            background: [255, 255, 255, 255],
        },
    )?)
}

fn remove_bg_json_value(
    input: &std::path::Path,
    result: &ConvertResult,
    color: &str,
    tolerance: f32,
    feather: f32,
    edge_only: bool,
) -> serde_json::Value {
    json!({
        "command": "remove-bg",
        "input": input,
        "output": result.output,
        "width": result.width,
        "height": result.height,
        "format": result.format.to_string(),
        "alpha_preserved": result.format.supports_alpha(),
        "color": color,
        "tolerance": tolerance,
        "feather": feather,
        "edge_only": edge_only,
    })
}

fn run_trim(
    input: &std::path::Path,
    output: Option<PathBuf>,
    corner_tolerance: Option<u8>,
    quality: u8,
) -> Result<(ConvertResult, TrimResult, TrimMode)> {
    let output = resolve_output(output, input, &input_ext(input));
    let image = load_dynamic_image(input)?;
    let mode = match corner_tolerance {
        Some(tol) => TrimMode::CornerColor(tol),
        None => TrimMode::Transparent,
    };
    let (trimmed, trim_rect) = auto_trim(&image, mode);
    let format = output_format(&output)?;
    let result = save_dynamic_image(
        &trimmed,
        &output,
        ConvertOptions {
            format,
            quality,
            background: [255, 255, 255, 255],
        },
    )?;
    Ok((result, trim_rect, mode))
}

fn trim_json_value(
    input: &std::path::Path,
    result: &ConvertResult,
    trim_rect: TrimResult,
    mode: TrimMode,
) -> serde_json::Value {
    json!({
        "command": "trim",
        "input": input,
        "output": result.output,
        "width": result.width,
        "height": result.height,
        "format": result.format.to_string(),
        "mode": match mode {
            TrimMode::Transparent => "transparent",
            TrimMode::CornerColor(_) => "corner-color",
        },
        "crop": {
            "x": trim_rect.x,
            "y": trim_rect.y,
            "width": trim_rect.width,
            "height": trim_rect.height,
        },
    })
}

struct FrameCliOptions<'a> {
    input: &'a std::path::Path,
    output: Option<PathBuf>,
    padding: u32,
    round_corners: f32,
    shadow_sigma: f32,
    shadow_x: i32,
    shadow_y: i32,
    background: &'a str,
    border_width: u32,
    border_color: &'a str,
    quality: u8,
}

fn run_frame(opts: &FrameCliOptions<'_>) -> Result<(ConvertResult, FrameCardOptions)> {
    let round_corners = ensure_finite(opts.round_corners, "--round-corners")?;
    let shadow_sigma = ensure_finite(opts.shadow_sigma, "--shadow-sigma")?;
    if round_corners < 0.0 {
        anyhow::bail!("--round-corners must be >= 0.0, got {round_corners}");
    }
    if shadow_sigma < 0.0 {
        anyhow::bail!("--shadow-sigma must be >= 0.0, got {shadow_sigma}");
    }
    let bg = parse_rgba_hex(opts.background)?.0;
    let border = parse_rgba_hex(opts.border_color)?.0;
    let default_ext = if bg[3] < 255
        && SupportedFormat::from_path(opts.input).is_some_and(|f| !f.supports_alpha())
    {
        "png".to_string()
    } else {
        input_ext(opts.input)
    };
    let output = resolve_output(opts.output.clone(), opts.input, &default_ext);
    let image = load_dynamic_image(opts.input)?;
    let card_opts = FrameCardOptions {
        padding: opts.padding,
        corner_radius: round_corners,
        shadow_sigma,
        shadow_offset_x: opts.shadow_x,
        shadow_offset_y: opts.shadow_y,
        background_color: bg,
        border_width: opts.border_width,
        border_color: border,
    };
    let framed = add_border_frame(&image, card_opts);
    let format = output_format(&output)?;
    let result = save_dynamic_image(
        &framed,
        &output,
        ConvertOptions {
            format,
            quality: opts.quality,
            background: [255, 255, 255, 255],
        },
    )?;
    Ok((result, card_opts))
}

fn frame_json_value(
    input: &std::path::Path,
    result: &ConvertResult,
    opts: &FrameCardOptions,
) -> serde_json::Value {
    json!({
        "command": "frame",
        "input": input,
        "output": result.output,
        "width": result.width,
        "height": result.height,
        "format": result.format.to_string(),
        "padding": opts.padding,
        "round_corners": opts.corner_radius,
        "shadow_sigma": opts.shadow_sigma,
        "shadow_x": opts.shadow_offset_x,
        "shadow_y": opts.shadow_offset_y,
        "border_width": opts.border_width,
    })
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

    #[test]
    fn cli_parses_pro_commands_and_new_filter_flags() {
        let hist_cli =
            Cli::try_parse_from(["studio-cli", "--json", "histogram", "in.png", "--bins"]).unwrap();
        assert!(hist_cli.json);
        assert!(matches!(
            hist_cli.command,
            Command::Histogram { bins: true, .. }
        ));

        let filter_cli = Cli::try_parse_from([
            "studio-cli",
            "filter",
            "in.png",
            "out.png",
            "--levels",
            "10,1.1,245",
            "--shadows",
            "-25",
            "--highlights",
            "-40",
            "--vibrance",
            "-15",
            "--clarity",
            "-10",
            "--color-balance",
            "-12,5,-8",
            "--threshold",
            "120",
            "--look",
            "cinematic",
            "--grain",
            "0.25",
            "--pixelate",
            "4",
            "--emboss",
            "0.8",
            "--edge-detect",
            "--sketch",
            "--oil-paint",
            "3",
            "--tilt-shift",
            "0.5,0.25,6.0",
        ])
        .unwrap();
        assert!(matches!(
            filter_cli.command,
            Command::Filter {
                shadows: Some(s),
                highlights: Some(h),
                vibrance: Some(v),
                clarity: Some(c),
                threshold: Some(120),
                pixelate: Some(4),
                edge_detect: true,
                sketch: true,
                oil_paint: Some(3),
                ..
            } if (s + 25.0).abs() < 1e-6
                && (h + 40.0).abs() < 1e-6
                && (v + 15.0).abs() < 1e-6
                && (c + 10.0).abs() < 1e-6
        ));

        let remove_bg_cli = Cli::try_parse_from([
            "studio-cli",
            "--json",
            "remove-bg",
            "in.jpg",
            "--color",
            "#00FF00",
            "--tolerance",
            "24",
            "--feather",
            "4",
            "--edge-only",
        ])
        .unwrap();
        assert!(matches!(
            remove_bg_cli.command,
            Command::RemoveBg {
                output: None,
                edge_only: true,
                ..
            }
        ));

        let trim_cli = Cli::try_parse_from([
            "studio-cli",
            "trim",
            "in.png",
            "out.png",
            "--corner-tolerance",
            "12",
        ])
        .unwrap();
        assert!(matches!(
            trim_cli.command,
            Command::Trim {
                corner_tolerance: Some(12),
                ..
            }
        ));

        let frame_cli = Cli::try_parse_from([
            "studio-cli",
            "frame",
            "in.png",
            "out.png",
            "--padding",
            "32",
            "--round-corners",
            "12",
            "--shadow-sigma",
            "8",
            "--shadow-x",
            "-4",
            "--shadow-y",
            "-2",
            "--background",
            "#112233FF",
            "--border-width",
            "2",
            "--border-color",
            "#AABBCCFF",
        ])
        .unwrap();
        assert!(matches!(
            frame_cli.command,
            Command::Frame {
                padding: 32,
                shadow_x: -4,
                shadow_y: -2,
                border_width: 2,
                ..
            }
        ));
    }

    #[test]
    fn parse_helpers_validate_levels_color_balance_tilt_shift_and_looks() {
        let lvl3 = parse_levels("10, 1.1, 245").unwrap();
        assert_eq!(lvl3.in_black, 10);
        assert!((lvl3.gamma - 1.1).abs() < 1e-6);
        assert_eq!(lvl3.in_white, 245);
        assert_eq!(lvl3.out_black, 0);
        assert_eq!(lvl3.out_white, 255);

        let lvl5 = parse_levels("12,0.9,240,5,250").unwrap();
        assert_eq!(lvl5.out_black, 5);
        assert_eq!(lvl5.out_white, 250);

        assert!(parse_levels("10,1.0").is_err());
        assert!(parse_levels("200,1.0,100").is_err());
        assert!(parse_levels("10,0.0,240").is_err());
        assert!(parse_levels("10,NaN,240").is_err());

        let cb = parse_color_balance("15,-5,-10").unwrap();
        assert!((cb.r_shift - 15.0).abs() < 1e-6);
        assert!((cb.g_shift + 5.0).abs() < 1e-6);
        assert!((cb.b_shift + 10.0).abs() < 1e-6);
        assert!(parse_color_balance("15,-5").is_err());
        assert!(parse_color_balance("15,inf,-10").is_err());

        let ts = parse_tilt_shift("0.5,0.25,6.0").unwrap();
        assert!((ts.focus_y - 0.5).abs() < 1e-6);
        assert!((ts.band_width - 0.25).abs() < 1e-6);
        assert!((ts.blur_sigma - 6.0).abs() < 1e-6);
        assert!(parse_tilt_shift("1.5,0.25,6.0").is_err());
        assert!(parse_tilt_shift("0.5,0.0,6.0").is_err());
        assert!(parse_tilt_shift("0.5,0.25,-1.0").is_err());

        for (name, expected) in [
            ("vivid", ColorLook::Vivid),
            ("cinematic", ColorLook::CinematicTealOrange),
            ("cinematic-teal-orange", ColorLook::CinematicTealOrange),
            ("matte", ColorLook::MatteFilm),
            ("matte-film", ColorLook::MatteFilm),
            ("moody-bw", ColorLook::MoodyBw),
            ("warm", ColorLook::WarmGolden),
            ("warm-golden", ColorLook::WarmGolden),
            ("cool", ColorLook::CrispCool),
            ("crisp-cool", ColorLook::CrispCool),
            ("high-key", ColorLook::HighKey),
            ("cyberpunk", ColorLook::Cyberpunk),
        ] {
            assert_eq!(parse_color_look(name).unwrap(), expected);
        }
        assert!(parse_color_look("nonexistent-look").is_err());
    }

    #[test]
    fn histogram_command_computes_stats_and_optional_bins_json() {
        let dir = TestDir::new("histogram");
        let src = dir.path().join("bw.png");
        let mut img = image::RgbaImage::new(4, 2);
        for y in 0..2 {
            for x in 0..4 {
                let v = if x < 2 { 0 } else { 255 };
                img.put_pixel(x, y, Rgba([v, v, v, 255]));
            }
        }
        DynamicImage::ImageRgba8(img).save(&src).unwrap();

        let hist = run_histogram(&src).unwrap();
        assert_eq!(hist.pixel_count, 8);
        assert!((hist.mean_luma - 127.5).abs() < 0.5);
        assert!((hist.shadow_clip_pct - 50.0).abs() < 0.01);
        assert!((hist.highlight_clip_pct - 50.0).abs() < 0.01);

        let summary_json = histogram_json_value(&src, &hist, false);
        assert_eq!(summary_json["pixel_count"], 8);
        assert!(summary_json["mean_luma"].is_number());
        assert!(summary_json["std_dev_luma"].is_number());
        assert!(summary_json["shadow_clip_pct"].is_number());
        assert!(summary_json["highlight_clip_pct"].is_number());
        assert!(summary_json.get("r").is_none());
        assert!(summary_json.get("luma").is_none());

        let bins_json = histogram_json_value(&src, &hist, true);
        let r_bins = bins_json["r"].as_array().unwrap();
        let luma_bins = bins_json["luma"].as_array().unwrap();
        assert_eq!(r_bins.len(), 256);
        assert_eq!(luma_bins.len(), 256);
        assert_eq!(luma_bins[0], 4);
        assert_eq!(luma_bins[255], 4);
        assert_eq!(bins_json["g"].as_array().unwrap().len(), 256);
        assert_eq!(bins_json["b"].as_array().unwrap().len(), 256);
    }

    #[test]
    fn filter_command_applies_pro_tonal_and_effect_flags() {
        let dir = TestDir::new("pro_filter");
        let src = dir.path().join("gradient.png");
        let mut rgba = image::RgbaImage::new(20, 20);
        for y in 0..20 {
            for x in 0..20 {
                rgba.put_pixel(
                    x,
                    y,
                    Rgba([(x * 12) as u8, (y * 12) as u8, ((x + y) * 6) as u8, 220]),
                );
            }
        }
        DynamicImage::ImageRgba8(rgba).save(&src).unwrap();

        let tonal_out = dir.path().join("tonal.png");
        let (tonal_res, tonal_steps) = run_filter(FilterOptions {
            input: src.clone(),
            output: Some(tonal_out.clone()),
            levels: Some("10,1.1,245".to_string()),
            shadows: Some(20.0),
            highlights: Some(-15.0),
            vibrance: Some(25.0),
            clarity: Some(30.0),
            color_balance: Some("10,-5,-10".to_string()),
            look: Some("cinematic".to_string()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(tonal_steps, 6);
        assert_eq!(tonal_res.width, 20);
        assert_eq!(tonal_res.height, 20);
        let tonal_img = load_dynamic_image(&tonal_out).unwrap().to_rgba8();
        assert!(tonal_img.pixels().all(|p| p[3] == 220));
        let tonal_json = filter_json_value(&src, &tonal_res, tonal_steps);
        assert_eq!(tonal_json["steps_applied"], 6);

        let threshold_out = dir.path().join("thresh.png");
        let (_, thresh_steps) = run_filter(FilterOptions {
            input: src.clone(),
            output: Some(threshold_out.clone()),
            threshold: Some(128),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(thresh_steps, 1);
        let thresh_img = load_dynamic_image(&threshold_out).unwrap().to_rgba8();
        assert!(
            thresh_img
                .pixels()
                .all(|p| (p[0] == 0 || p[0] == 255) && p[0] == p[1] && p[1] == p[2] && p[3] == 220)
        );

        let step_src = dir.path().join("step.png");
        let mut step_img = image::RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 255]));
        for y in 0..10 {
            for x in 5..10 {
                step_img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        DynamicImage::ImageRgba8(step_img).save(&step_src).unwrap();

        let edge_out = dir.path().join("edge.png");
        run_filter(FilterOptions {
            input: step_src.clone(),
            output: Some(edge_out.clone()),
            edge_detect: true,
            ..Default::default()
        })
        .unwrap();
        let edge_rgba = load_dynamic_image(&edge_out).unwrap().to_rgba8();
        assert_eq!(edge_rgba.get_pixel(1, 5)[0], 0);
        assert_eq!(edge_rgba.get_pixel(4, 5)[0], 255);

        let sketch_out = dir.path().join("sketch.png");
        run_filter(FilterOptions {
            input: step_src,
            output: Some(sketch_out.clone()),
            sketch: true,
            ..Default::default()
        })
        .unwrap();
        let sketch_rgba = load_dynamic_image(&sketch_out).unwrap().to_rgba8();
        assert_eq!(sketch_rgba.get_pixel(1, 5)[0], 255);
        assert_eq!(sketch_rgba.get_pixel(4, 5)[0], 0);

        let fx_out = dir.path().join("effects.png");
        let (fx_res, fx_steps) = run_filter(FilterOptions {
            input: src.clone(),
            output: Some(fx_out.clone()),
            grain: Some(0.2),
            pixelate: Some(4),
            emboss: Some(0.5),
            sketch: true,
            oil_paint: Some(2),
            tilt_shift: Some("0.5,0.3,2.0".to_string()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(fx_steps, 6);
        assert_eq!(fx_res.width, 20);
        assert_eq!(fx_res.height, 20);
        assert!(fx_out.exists());

        assert!(
            run_filter(FilterOptions {
                input: src.clone(),
                output: Some(dir.path().join("bad_pix.png")),
                pixelate: Some(1),
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            run_filter(FilterOptions {
                input: src,
                output: Some(dir.path().join("bad_oil.png")),
                oil_paint: Some(6),
                ..Default::default()
            })
            .is_err()
        );
    }

    #[test]
    fn remove_bg_command_preserves_png_and_webp_alpha_and_defaults_to_png() {
        let dir = TestDir::new("remove_bg");
        let mut rgba = image::RgbaImage::from_pixel(12, 12, Rgba([255, 255, 255, 255]));
        // Draw a solid red square ring with an enclosed white hole at the center.
        for y in 2..10 {
            for x in 2..10 {
                rgba.put_pixel(x, y, Rgba([220, 20, 20, 255]));
            }
        }
        for y in 4..8 {
            for x in 4..8 {
                rgba.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }

        // Omitting output on a JPEG input must default to .png to preserve transparency.
        let jpg_src = dir.path().join("subject.jpg");
        DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(rgba.clone()).to_rgb8())
            .save_with_format(&jpg_src, image::ImageFormat::Jpeg)
            .unwrap();
        let default_res = run_remove_bg(&RemoveBgOptions {
            input: &jpg_src,
            output: None,
            color: "#FFFFFF",
            tolerance: 32.0,
            feather: 8.0,
            edge_only: false,
            quality: 92,
        })
        .unwrap();
        assert_eq!(default_res.format, SupportedFormat::Png);
        assert_eq!(
            default_res.output.extension().and_then(|e| e.to_str()),
            Some("png")
        );
        let default_img = load_dynamic_image(&default_res.output).unwrap().to_rgba8();
        assert_eq!(default_img.get_pixel(0, 0)[3], 0);
        assert_eq!(default_img.get_pixel(2, 2)[3], 255);

        // Explicit WebP output with edge_only=true keeps enclosed white interior opaque.
        let png_src = dir.path().join("ring.png");
        DynamicImage::ImageRgba8(rgba).save(&png_src).unwrap();
        let webp_out = dir.path().join("cutout.webp");
        let webp_res = run_remove_bg(&RemoveBgOptions {
            input: &png_src,
            output: Some(webp_out.clone()),
            color: "#FFFFFF",
            tolerance: 16.0,
            feather: 0.0,
            edge_only: true,
            quality: 100,
        })
        .unwrap();
        assert_eq!(webp_res.format, SupportedFormat::Webp);
        let webp_img = load_dynamic_image(&webp_out).unwrap().to_rgba8();
        assert_eq!(webp_img.get_pixel(0, 0)[3], 0);
        assert_eq!(webp_img.get_pixel(5, 5)[3], 255);

        let json_val = remove_bg_json_value(&png_src, &webp_res, "#FFFFFF", 16.0, 0.0, true);
        assert_eq!(json_val["command"], "remove-bg");
        assert_eq!(json_val["width"], 12);
        assert_eq!(json_val["height"], 12);
        assert_eq!(json_val["alpha_preserved"], true);
        assert_eq!(json_val["edge_only"], true);
    }

    #[test]
    fn trim_command_trims_transparent_and_corner_borders() {
        let dir = TestDir::new("trim");
        let transparent_src = dir.path().join("padded.png");
        let mut rgba = image::RgbaImage::from_pixel(12, 10, Rgba([0, 0, 0, 0]));
        for y in 2..5 {
            for x in 3..7 {
                rgba.put_pixel(x, y, Rgba([40, 140, 220, 255]));
            }
        }
        DynamicImage::ImageRgba8(rgba)
            .save(&transparent_src)
            .unwrap();

        let t_out = dir.path().join("trimmed.png");
        let (t_res, t_crop, t_mode) = run_trim(&transparent_src, Some(t_out), None, 92).unwrap();
        assert_eq!(t_res.width, 4);
        assert_eq!(t_res.height, 3);
        assert_eq!(
            t_crop,
            TrimResult {
                x: 3,
                y: 2,
                width: 4,
                height: 3,
            }
        );
        let t_json = trim_json_value(&transparent_src, &t_res, t_crop, t_mode);
        assert_eq!(t_json["command"], "trim");
        assert_eq!(t_json["mode"], "transparent");
        assert_eq!(t_json["crop"]["x"], 3);
        assert_eq!(t_json["crop"]["y"], 2);
        assert_eq!(t_json["crop"]["width"], 4);
        assert_eq!(t_json["crop"]["height"], 3);

        let corner_src = dir.path().join("border.png");
        let mut corner_img = image::RgbaImage::from_pixel(10, 10, Rgba([250, 250, 250, 255]));
        for y in 3..7 {
            for x in 2..8 {
                corner_img.put_pixel(x, y, Rgba([20, 30, 40, 255]));
            }
        }
        DynamicImage::ImageRgba8(corner_img)
            .save(&corner_src)
            .unwrap();

        let (c_res, c_crop, c_mode) = run_trim(&corner_src, None, Some(5), 92).unwrap();
        assert_eq!(c_res.width, 6);
        assert_eq!(c_res.height, 4);
        assert_eq!(c_crop.x, 2);
        assert_eq!(c_crop.y, 3);
        let c_json = trim_json_value(&corner_src, &c_res, c_crop, c_mode);
        assert_eq!(c_json["mode"], "corner-color");
    }

    #[test]
    fn frame_command_adds_padding_rounded_card_border_and_shadow() {
        let dir = TestDir::new("frame");
        let src = dir.path().join("card.png");
        DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            16,
            12,
            Rgba([40, 90, 200, 255]),
        ))
        .save(&src)
        .unwrap();

        let out = dir.path().join("framed.png");
        let (res, card_opts) = run_frame(&FrameCliOptions {
            input: &src,
            output: Some(out.clone()),
            padding: 10,
            round_corners: 4.0,
            shadow_sigma: 2.0,
            shadow_x: 0,
            shadow_y: 4,
            background: "#F4F6F9FF",
            border_width: 2,
            border_color: "#D0D7DEFF",
            quality: 92,
        })
        .unwrap();
        assert!(out.exists());
        assert!(res.width > 16 + 20);
        assert!(res.height > 12 + 20);
        let framed_img = load_dynamic_image(&out).unwrap().to_rgba8();
        assert_eq!(*framed_img.get_pixel(0, 0), Rgba([0xF4, 0xF6, 0xF9, 0xFF]));

        let json_val = frame_json_value(&src, &res, &card_opts);
        assert_eq!(json_val["command"], "frame");
        assert_eq!(json_val["padding"], 10);
        assert_eq!(json_val["border_width"], 2);
        assert_eq!(json_val["shadow_y"], 4);

        // Transparent card background on JPEG input defaults to .png and retains transparency.
        let jpg_src = dir.path().join("photo.jpg");
        DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            8,
            8,
            image::Rgb([80, 120, 160]),
        ))
        .save_with_format(&jpg_src, image::ImageFormat::Jpeg)
        .unwrap();
        let (alpha_res, _) = run_frame(&FrameCliOptions {
            input: &jpg_src,
            output: None,
            padding: 6,
            round_corners: 0.0,
            shadow_sigma: 0.0,
            shadow_x: 0,
            shadow_y: 0,
            background: "#00000000",
            border_width: 0,
            border_color: "#D0D7DEFF",
            quality: 92,
        })
        .unwrap();
        assert_eq!(alpha_res.format, SupportedFormat::Png);
        let alpha_img = load_dynamic_image(&alpha_res.output).unwrap().to_rgba8();
        assert_eq!(alpha_img.get_pixel(0, 0)[3], 0);
    }
}
