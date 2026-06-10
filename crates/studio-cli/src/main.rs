use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use studio_core::{
    BatchOptions, C2paAddOptions, C2paManifestDraft, C2paRemovalOptions, ConvertOptions,
    GifOptions, PanoramaMode, PanoramaOptions, SupportedFormat, collect_supported_files,
    convert_image, inspect_image, process_batch, process_c2pa_add_batch,
    process_c2pa_removal_batch, write_animated_gif, write_panorama_jpeg,
};

#[derive(Debug, Parser)]
#[command(name = "studio-cli")]
#[command(about = "Pixel Studio command-line utilities")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
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
