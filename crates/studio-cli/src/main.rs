use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use studio_core::{
    BatchOptions, C2paAddOptions, C2paManifestDraft, C2paRemovalOptions, ConvertOptions,
    GifOptions, PanoramaMode, PanoramaOptions, SupportedFormat, blur_region,
    collect_supported_files, convert_image, crop_region, inspect_image, load_dynamic_image,
    process_batch, process_c2pa_add_batch, process_c2pa_removal_batch, remove_c2pa_manifest,
    save_dynamic_image, write_animated_gif, write_panorama_jpeg,
};

#[derive(Debug, Parser)]
#[command(name = "studio-cli")]
#[command(about = "Pixel Studio command-line utilities", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Detect text offline and redact sensitive data (solid box by default).
    Redact {
        input: PathBuf,
        output: PathBuf,
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
    Info {
        input: PathBuf,
    },
    Convert {
        input: PathBuf,
        output: PathBuf,
        #[arg(long)]
        format: SupportedFormat,
        #[arg(long, default_value_t = 92)]
        quality: u8,
    },
    Crop {
        input: PathBuf,
        output: PathBuf,
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
    BlurRegion {
        input: PathBuf,
        output: PathBuf,
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
    Pano360 {
        input: PathBuf,
        output: PathBuf,
        #[arg(long, default_value = "pad")]
        mode: PanoramaMode,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long, default_value_t = 92)]
        quality: u8,
    },
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
    C2paRemove {
        input_dir: PathBuf,
        output_dir: PathBuf,
    },
    C2paRemoveFile {
        input: PathBuf,
        output: PathBuf,
    },
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Redact { input, output, style, padding, keyword, all, keep_metadata, quality } => {
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
            for b in &boxes {
                println!(
                    "{} {:>4},{:<4} {:>4}x{:<3} {:<8} {}",
                    if b.selected { "[x]" } else { "[ ]" },
                    b.x, b.y, b.width, b.height,
                    b.kind.map(|k| k.label()).unwrap_or("-"),
                    b.text
                );
            }
            let style = match style.as_str() {
                "mosaic" => RedactStyle::Mosaic(12),
                "blur" => RedactStyle::Blur(10.0),
                _ => RedactStyle::default(),
            };
            let (out, n) = apply_redactions(&image, &boxes, style, padding);
            let format = studio_core::convert::require_output_format(&output)?;
            save_dynamic_image(&out, &output, ConvertOptions { format, quality, ..Default::default() })?;
            if keep_metadata {
                studio_core::metadata::copy_metadata(&input, &output)?;
            }
            println!("detected {} text lines, redacted {n}", boxes.len());
        }
        Command::Info { input } => {
            let info = inspect_image(input)?;
            println!(
                "{}: {}x{} {} {} bytes",
                info.path.display(),
                info.width,
                info.height,
                info.format,
                info.file_size
            );
        }
        Command::Convert {
            input,
            output,
            format,
            quality,
        } => {
            let result = convert_image(
                input,
                output,
                ConvertOptions {
                    format,
                    quality,
                    background: [255, 255, 255, 255],
                },
            )?;
            println!(
                "converted {} -> {} ({}x{}, {})",
                result.input.display(),
                result.output.display(),
                result.width,
                result.height,
                result.format
            );
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
            println!(
                "cropped {} -> {} ({}x{}, {})",
                input.display(),
                result.output.display(),
                result.width,
                result.height,
                result.format
            );
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
            println!(
                "blurred region in {} -> {} ({}x{}, {})",
                input.display(),
                result.output.display(),
                result.width,
                result.height,
                result.format
            );
        }
        Command::Pano360 {
            input,
            output,
            mode,
            width,
            quality,
        } => {
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
            println!(
                "wrote 360-compatible JPEG {} ({}x{}, mode={})",
                result.output.display(),
                result.width,
                result.height,
                result.mode
            );
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
        Command::C2paRemoveFile { input, output } => {
            remove_c2pa_manifest(&input, &output)?;
            println!(
                "removed C2PA manifest {} -> {}",
                input.display(),
                output.display()
            );
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

    Ok(())
}

fn output_format(output: &std::path::Path) -> Result<SupportedFormat> {
    SupportedFormat::from_path(output)
        .ok_or_else(|| anyhow::anyhow!("unsupported output format: {}", output.display()))
}
