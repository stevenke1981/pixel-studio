use std::fs::File;
use std::path::{Path, PathBuf};

use image::codecs::gif::{GifEncoder, Repeat};
use image::imageops::{FilterType, overlay};
use image::{Delay, DynamicImage, Frame, Rgba, RgbaImage};

use crate::{PhotoError, Result, load_dynamic_image};

#[derive(Debug, Clone, Copy)]
pub struct GifOptions {
    pub delay_ms: u32,
    pub repeat: bool,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub background: [u8; 4],
}

#[derive(Debug, Clone)]
pub struct GifResult {
    pub output: PathBuf,
    pub frame_count: usize,
    pub width: u32,
    pub height: u32,
    pub delay_ms: u32,
}

pub fn write_animated_gif(
    inputs: &[PathBuf],
    output: impl AsRef<Path>,
    options: GifOptions,
) -> Result<GifResult> {
    if inputs.is_empty() {
        return Err(PhotoError::InvalidInput(
            "animated GIF requires at least one input image".to_owned(),
        ));
    }

    let frames = load_frames(inputs, options)?;
    let (width, height) = (frames[0].width(), frames[0].height());
    let output = output.as_ref();
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(output)?;
    let mut encoder = GifEncoder::new(file);
    if options.repeat {
        encoder.set_repeat(Repeat::Infinite)?;
    }

    let delay = Delay::from_numer_denom_ms(options.delay_ms.max(10), 1);
    for frame in frames {
        encoder.encode_frame(Frame::from_parts(frame, 0, 0, delay))?;
    }

    Ok(GifResult {
        output: output.to_path_buf(),
        frame_count: inputs.len(),
        width,
        height,
        delay_ms: options.delay_ms.max(10),
    })
}

fn load_frames(inputs: &[PathBuf], options: GifOptions) -> Result<Vec<RgbaImage>> {
    let mut images = Vec::with_capacity(inputs.len());
    for input in inputs {
        images.push(apply_max_size(load_dynamic_image(input)?, options));
    }

    let width = images.iter().map(DynamicImage::width).max().unwrap_or(1);
    let height = images.iter().map(DynamicImage::height).max().unwrap_or(1);
    Ok(images
        .into_iter()
        .map(|image| center_on_canvas(&image, width, height, options.background))
        .collect())
}

fn apply_max_size(image: DynamicImage, options: GifOptions) -> DynamicImage {
    match (options.max_width, options.max_height) {
        (Some(max_width), Some(max_height)) => {
            image.resize(max_width.max(1), max_height.max(1), FilterType::Lanczos3)
        }
        _ => image,
    }
}

fn center_on_canvas(
    image: &DynamicImage,
    width: u32,
    height: u32,
    background: [u8; 4],
) -> RgbaImage {
    let mut canvas = RgbaImage::from_pixel(width, height, Rgba(background));
    let frame = image.to_rgba8();
    let x = width.saturating_sub(frame.width()) / 2;
    let y = height.saturating_sub(frame.height()) / 2;
    overlay(&mut canvas, &frame, i64::from(x), i64::from(y));
    canvas
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, Rgba, RgbaImage};

    use super::{GifOptions, write_animated_gif};

    #[test]
    fn writes_decodable_animated_gif() {
        let dir = temp_dir("writes_decodable_animated_gif");
        fs::create_dir_all(&dir).unwrap();
        let input_a = dir.join("frame_a.png");
        let input_b = dir.join("frame_b.png");
        let output = dir.join("animation.gif");

        DynamicImage::ImageRgba8(RgbaImage::from_pixel(16, 12, Rgba([255, 0, 0, 255])))
            .save(&input_a)
            .unwrap();
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 8, Rgba([0, 0, 255, 255])))
            .save(&input_b)
            .unwrap();

        let result = write_animated_gif(
            &[input_a, input_b],
            &output,
            GifOptions {
                delay_ms: 120,
                repeat: true,
                max_width: None,
                max_height: None,
                background: [255, 255, 255, 255],
            },
        )
        .unwrap();

        assert_eq!(result.frame_count, 2);
        assert_eq!((result.width, result.height), (16, 12));
        assert!(output.exists());
        let decoded = image::open(&output).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (16, 12));

        let _ = fs::remove_dir_all(dir);
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("photo_tool_{name}_{nanos}"))
    }
}
