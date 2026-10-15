//! Offline text detection/recognition for auto-redaction (ocrs + rten, ONNX models).
//!
//! Models (~12 MB) are downloaded once on first use into the user data dir, or loaded
//! from `PIXEL_STUDIO_MODEL_DIR` / a `models` folder next to the executable.
//! Detection is script-agnostic (works for Chinese and English text); recognition
//! currently reads Latin letters, digits and symbols only, which covers emails,
//! phone numbers, IDs, card numbers and URLs.

use std::path::{Path, PathBuf};

use image::DynamicImage;
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;

use crate::redact::{RedactBox, preselect_sensitive};

pub const DETECTION_MODEL: &str = "text-detection.onnx";
pub const RECOGNITION_MODEL: &str = "text-recognition.onnx";
const MODEL_BASE_URL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/";

/// Directories searched for the models, in priority order.
pub fn model_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(d) = std::env::var_os("PIXEL_STUDIO_MODEL_DIR") {
        dirs.push(PathBuf::from(d));
    }
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        dirs.push(exe_dir.join("models"));
    }
    if let Some(d) = dirs::data_dir() {
        dirs.push(d.join("pixel-studio").join("models"));
    }
    dirs
}

pub fn find_models() -> Option<PathBuf> {
    model_dirs()
        .into_iter()
        .find(|d| d.join(DETECTION_MODEL).is_file() && d.join(RECOGNITION_MODEL).is_file())
}

/// Ensure the models exist locally, downloading them into the user data dir if needed.
pub fn ensure_models() -> anyhow::Result<PathBuf> {
    if let Some(d) = find_models() {
        return Ok(d);
    }
    let dir = model_dirs()
        .pop()
        .ok_or_else(|| anyhow::anyhow!("no data directory available"))?;
    std::fs::create_dir_all(&dir)?;
    for name in [DETECTION_MODEL, RECOGNITION_MODEL] {
        let target = dir.join(name);
        if target.is_file() {
            continue;
        }
        let mut resp = ureq::get(&format!("{MODEL_BASE_URL}{name}")).call()?;
        let bytes = resp
            .body_mut()
            .with_config()
            .limit(64 * 1024 * 1024)
            .read_to_vec()?;
        let tmp = target.with_extension("part");
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, &target)?;
    }
    Ok(dir)
}

pub struct TextDetector {
    engine: OcrEngine,
}

impl TextDetector {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let detection_model = Model::load_file(dir.join(DETECTION_MODEL))?;
        let recognition_model = Model::load_file(dir.join(RECOGNITION_MODEL))?;
        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            ..Default::default()
        })?;
        Ok(Self { engine })
    }

    /// Detect text lines. Each line becomes one box with its recognized text;
    /// sensitive lines are pre-selected.
    pub fn detect(&self, image: &DynamicImage) -> anyhow::Result<Vec<RedactBox>> {
        let rgb = image.to_rgb8();
        let src = ImageSource::from_bytes(rgb.as_raw(), rgb.dimensions())?;
        let input = self.engine.prepare_input(src)?;
        let words = self.engine.detect_words(&input)?;
        let lines = self.engine.find_text_lines(&input, &words);
        let texts = self.engine.recognize_text(&input, &lines)?;
        let mut boxes = Vec::new();
        for (line, text) in lines.iter().zip(texts.iter()) {
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for word in line {
                for c in word.corners() {
                    x0 = x0.min(c.x);
                    y0 = y0.min(c.y);
                    x1 = x1.max(c.x);
                    y1 = y1.max(c.y);
                }
            }
            // Drop specks: too small to be readable text.
            if x1 - x0 < 6.0 || y1 - y0 < 8.0 || (x1 - x0) * (y1 - y0) < 150.0 {
                continue;
            }
            boxes.push(RedactBox {
                x: x0.floor() as i32,
                y: y0.floor() as i32,
                width: (x1 - x0).ceil() as u32,
                height: (y1 - y0).ceil() as u32,
                text: text.as_ref().map(|t| t.to_string()).unwrap_or_default(),
                selected: false,
                kind: None,
            });
        }
        preselect_sensitive(&mut boxes);
        Ok(boxes)
    }
}
