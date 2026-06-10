use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rayon::prelude::*;
use walkdir::WalkDir;

use crate::{
    C2paManifestDraft, ComposeDocument, ComposeLayer, ConvertOptions, ConvertResult,
    SupportedFormat, TextLayer, load_dynamic_image, remove_c2pa_manifest, save_dynamic_image,
    write_c2pa_manifest,
};

#[derive(Debug, Clone)]
pub struct BatchOptions {
    pub output_dir: PathBuf,
    pub convert: ConvertOptions,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub watermark_text: Option<String>,
    pub watermark_opacity: f32,
    pub watermark_font_size: f32,
}

#[derive(Debug, Clone)]
pub struct BatchResult {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BatchSummary {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub results: Vec<BatchResult>,
}

#[derive(Debug, Clone)]
pub struct C2paRemovalOptions {
    pub output_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct C2paAddOptions {
    pub output_dir: PathBuf,
    pub draft: C2paManifestDraft,
}

pub fn collect_supported_files(root: impl AsRef<Path>) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| SupportedFormat::from_path(path).is_some())
        .collect();
    files.sort();
    files
}

pub fn process_batch(inputs: &[PathBuf], options: &BatchOptions) -> BatchSummary {
    process_batch_with_progress(inputs, options, |_completed, _total| {})
}

pub fn process_batch_with_progress(
    inputs: &[PathBuf],
    options: &BatchOptions,
    progress: impl Fn(usize, usize) + Sync,
) -> BatchSummary {
    let total = inputs.len();
    let completed = Arc::new(AtomicUsize::new(0));
    let results: Vec<BatchResult> = inputs
        .par_iter()
        .map(|input| {
            let result = process_one(input, options);
            let completed = completed.fetch_add(1, Ordering::Relaxed) + 1;
            progress(completed, total);
            result
        })
        .collect();

    summarize_results(results)
}

pub fn process_c2pa_add_batch(inputs: &[PathBuf], options: &C2paAddOptions) -> BatchSummary {
    process_c2pa_add_batch_with_progress(inputs, options, |_completed, _total| {})
}

pub fn process_c2pa_add_batch_with_progress(
    inputs: &[PathBuf],
    options: &C2paAddOptions,
    progress: impl Fn(usize, usize) + Sync,
) -> BatchSummary {
    let total = inputs.len();
    let completed = Arc::new(AtomicUsize::new(0));
    let results: Vec<BatchResult> = inputs
        .par_iter()
        .map(|input| {
            let result = process_c2pa_add_one(input, &options.output_dir, &options.draft);
            let completed = completed.fetch_add(1, Ordering::Relaxed) + 1;
            progress(completed, total);
            result
        })
        .collect();

    summarize_results(results)
}

pub fn process_c2pa_removal_batch(
    inputs: &[PathBuf],
    options: &C2paRemovalOptions,
) -> BatchSummary {
    process_c2pa_removal_batch_with_progress(inputs, options, |_completed, _total| {})
}

pub fn process_c2pa_removal_batch_with_progress(
    inputs: &[PathBuf],
    options: &C2paRemovalOptions,
    progress: impl Fn(usize, usize) + Sync,
) -> BatchSummary {
    let total = inputs.len();
    let completed = Arc::new(AtomicUsize::new(0));
    let results: Vec<BatchResult> = inputs
        .par_iter()
        .map(|input| {
            let result = process_c2pa_removal_one(input, &options.output_dir);
            let completed = completed.fetch_add(1, Ordering::Relaxed) + 1;
            progress(completed, total);
            result
        })
        .collect();

    summarize_results(results)
}

fn summarize_results(results: Vec<BatchResult>) -> BatchSummary {
    let succeeded = results
        .iter()
        .filter(|result| result.error.is_none())
        .count();
    let failed = results.len().saturating_sub(succeeded);
    BatchSummary {
        total: results.len(),
        succeeded,
        failed,
        results,
    }
}

fn process_one(input: &Path, options: &BatchOptions) -> BatchResult {
    let stem = input
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let output = options
        .output_dir
        .join(format!("{stem}.{}", options.convert.format.extension()));

    let image = match load_dynamic_image(input) {
        Ok(img) => img,
        Err(e) => {
            return BatchResult {
                input: input.to_path_buf(),
                output: None,
                error: Some(format!("Failed to load image: {}", e)),
            };
        }
    };

    let processed_image = if let Some(watermark) = options
        .watermark_text
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        add_watermark_to_image(
            &image,
            watermark,
            options.watermark_opacity,
            options.watermark_font_size,
        )
    } else {
        image
    };

    let result = save_dynamic_image(&processed_image, &output, options.convert);

    match result {
        Ok(ConvertResult { output, .. }) => BatchResult {
            input: input.to_path_buf(),
            output: Some(output),
            error: None,
        },
        Err(error) => BatchResult {
            input: input.to_path_buf(),
            output: None,
            error: Some(error.to_string()),
        },
    }
}

fn process_c2pa_add_one(input: &Path, output_dir: &Path, draft: &C2paManifestDraft) -> BatchResult {
    let output = c2pa_output_path(input, output_dir, "_c2pa");
    if output.exists() {
        return BatchResult {
            input: input.to_path_buf(),
            output: None,
            error: Some(format!("Output already exists: {}", output.display())),
        };
    }

    let temp_output = temporary_c2pa_output_path(&output);
    let _ = std::fs::remove_file(&temp_output);
    let normalized_draft = normalized_c2pa_draft(input, draft);
    match write_c2pa_manifest(input, &temp_output, &normalized_draft) {
        Ok(()) => finalize_c2pa_output(input, temp_output, output),
        Err(error) => {
            let _ = std::fs::remove_file(&temp_output);
            BatchResult {
                input: input.to_path_buf(),
                output: None,
                error: Some(error.to_string()),
            }
        }
    }
}

fn process_c2pa_removal_one(input: &Path, output_dir: &Path) -> BatchResult {
    let output = c2pa_output_path(input, output_dir, "_no_c2pa");
    if output.exists() {
        return BatchResult {
            input: input.to_path_buf(),
            output: None,
            error: Some(format!("Output already exists: {}", output.display())),
        };
    }

    // Stage through a hidden same-extension file so C2PA format detection still works.
    let temp_output = temporary_c2pa_output_path(&output);
    let _ = std::fs::remove_file(&temp_output);
    match remove_c2pa_manifest(input, &temp_output) {
        Ok(()) => finalize_c2pa_output(input, temp_output, output),
        Err(e) => {
            let _ = std::fs::remove_file(&temp_output);
            BatchResult {
                input: input.to_path_buf(),
                output: None,
                error: Some(e.to_string()),
            }
        }
    }
}

fn c2pa_output_path(input: &Path, output_dir: &Path, suffix: &str) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let ext = input
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("bin");
    output_dir.join(format!("{stem}{suffix}.{ext}"))
}

fn temporary_c2pa_output_path(output: &Path) -> PathBuf {
    let extension = output
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("tmp");
    let stem = output
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let file_name = format!(
        ".studio-app-{stem}-{}-{nanos}.{extension}",
        std::process::id()
    );

    output
        .parent()
        .map(|parent| parent.join(&file_name))
        .unwrap_or_else(|| std::env::temp_dir().join(file_name))
}

fn normalized_c2pa_draft(input: &Path, draft: &C2paManifestDraft) -> C2paManifestDraft {
    C2paManifestDraft {
        title: non_empty_or_else(&draft.title, || {
            input
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("image")
                .to_owned()
        }),
        creator: non_empty_or_else(&draft.creator, || "Pixel Studio".to_owned()),
        action: non_empty_or_else(&draft.action, || "Batch C2PA manifest added".to_owned()),
    }
}

fn non_empty_or_else(value: &str, fallback: impl FnOnce() -> String) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback()
    } else {
        trimmed.to_owned()
    }
}

fn finalize_c2pa_output(input: &Path, temp_output: PathBuf, output: PathBuf) -> BatchResult {
    if let Err(e) = std::fs::rename(&temp_output, &output) {
        let _ = std::fs::remove_file(&temp_output);
        return BatchResult {
            input: input.to_path_buf(),
            output: None,
            error: Some(format!("Failed to finalize output: {}", e)),
        };
    }
    BatchResult {
        input: input.to_path_buf(),
        output: Some(output),
        error: None,
    }
}

fn add_watermark_to_image(
    image: &image::DynamicImage,
    text: &str,
    opacity: f32,
    font_size: f32,
) -> image::DynamicImage {
    let mut layers: Vec<ComposeLayer> = Vec::new();

    let image_width = image.width();
    let image_height = image.height();
    let margin = ((image_width.min(image_height) as f32) * 0.03).clamp(16.0, 64.0) as i32;

    // Estimate text dimensions for placement (the renderer uses the real font,
    // but we only need a sane bottom-right anchor here).
    let estimated_width = (text.chars().count() as f32 * font_size * 0.62).max(font_size);
    let estimated_height = font_size;

    let x = (image_width as i32 - estimated_width as i32 - margin).max(margin);
    let y = (image_height as i32 - estimated_height as i32 - margin).max(margin);

    layers.push(ComposeLayer::Text(TextLayer {
        name: "watermark".to_owned(),
        text: text.to_owned(),
        font_path: None,
        x,
        y,
        font_size: font_size.clamp(12.0, 256.0),
        color: [255, 255, 255, 255],
        opacity: opacity.clamp(0.05, 1.0),
        stroke: true,
        shadow: true,
        visible: true,
    }));

    crate::render_composition(
        &ComposeDocument {
            background: image.clone(),
            layers,
        },
        None,
    )
}
