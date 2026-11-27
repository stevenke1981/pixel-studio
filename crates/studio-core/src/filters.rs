use image::DynamicImage;
use rayon::prelude::*;

use crate::util::{clamp_channel, hsl_to_rgb, lerp_channel, rgb_to_hsl};

/// A trait for pixel-level image filters.
///
/// Implementing this trait allows a filter to be used in a `FilterPipeline`
/// and applied generically across the system (OCP / DIP).
pub trait Filter {
    /// Human-readable filter name.
    fn name(&self) -> &'static str;
    /// Apply this filter to an image.
    fn apply(&self, image: &DynamicImage) -> DynamicImage;
}

/// A composable pipeline of filters applied in sequence.
///
/// Instead of calling filters as individual free functions,
/// compose them into a pipeline for batch or incremental editing.
pub struct FilterPipeline {
    filters: Vec<Box<dyn Filter>>,
}

impl FilterPipeline {
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
        }
    }

    pub fn push(&mut self, filter: Box<dyn Filter>) {
        self.filters.push(filter);
    }

    pub fn pop(&mut self) -> Option<Box<dyn Filter>> {
        self.filters.pop()
    }

    pub fn apply_all(&self, image: &DynamicImage) -> DynamicImage {
        let mut result = image.clone();
        for f in &self.filters {
            result = f.apply(&result);
        }
        result
    }

    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }

    pub fn len(&self) -> usize {
        self.filters.len()
    }
}

impl Default for FilterPipeline {
    fn default() -> Self {
        Self::new()
    }
}

// ── Individual filter structs implementing the Filter trait ──

/// Rotate image 180 degrees.
#[derive(Debug, Clone, Copy)]
pub struct Rotate180;

impl Filter for Rotate180 {
    fn name(&self) -> &'static str {
        "Rotate 180"
    }
    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        rotate_180(image)
    }
}

/// Shift hue by degrees (-180..180).
#[derive(Debug, Clone, Copy)]
pub struct HueAdjust {
    pub degrees: f32,
}

impl Filter for HueAdjust {
    fn name(&self) -> &'static str {
        "Hue"
    }
    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        adjust_hue(image, self.degrees)
    }
}

/// Apply sepia tone.
#[derive(Debug, Clone, Copy)]
pub struct Sepia {
    pub strength: f32,
}

impl Filter for Sepia {
    fn name(&self) -> &'static str {
        "Sepia"
    }
    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        sepia(image, self.strength)
    }
}

/// Vignette darkens edges.
#[derive(Debug, Clone, Copy)]
pub struct Vignette {
    pub strength: f32,
}

impl Filter for Vignette {
    fn name(&self) -> &'static str {
        "Vignette"
    }
    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        vignette(image, self.strength)
    }
}

/// Color temperature adjustment.
#[derive(Debug, Clone, Copy)]
pub struct TemperatureAdjust {
    pub amount: f32,
}

impl Filter for TemperatureAdjust {
    fn name(&self) -> &'static str {
        "Temperature"
    }
    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        adjust_temperature(image, self.amount)
    }
}

/// Green/magenta tint adjustment.
#[derive(Debug, Clone, Copy)]
pub struct TintAdjust {
    pub amount: f32,
}

impl Filter for TintAdjust {
    fn name(&self) -> &'static str {
        "Tint"
    }
    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        adjust_tint(image, self.amount)
    }
}

/// Auto contrast (histogram stretch).
#[derive(Debug, Clone, Copy)]
pub struct AutoContrast;

impl Filter for AutoContrast {
    fn name(&self) -> &'static str {
        "Auto Contrast"
    }
    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        auto_contrast(image)
    }
}

/// Posterize to `levels` per channel.
#[derive(Debug, Clone, Copy)]
pub struct Posterize {
    pub levels: u8,
}

impl Filter for Posterize {
    fn name(&self) -> &'static str {
        "Posterize"
    }
    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        posterize(image, self.levels)
    }
}

// ── Free functions (kept for backward compatibility) ──

/// Rotate image 180 degrees.
pub fn rotate_180(image: &DynamicImage) -> DynamicImage {
    image.rotate180()
}

/// Shift hue by degrees (-180..180). Zero leaves the image unchanged.
pub fn adjust_hue(image: &DynamicImage, degrees: f32) -> DynamicImage {
    if degrees.abs() < f32::EPSILON {
        return image.clone();
    }

    let shift = degrees.rem_euclid(360.0) / 360.0;
    let mut rgba = image.to_rgba8();

    rgba.par_chunks_mut(4).for_each(|pixel| {
        let (h, s, l) = rgb_to_hsl(pixel[0], pixel[1], pixel[2]);
        let new_h = (h + shift).rem_euclid(1.0);
        let (r, g, b) = hsl_to_rgb(new_h, s, l);
        pixel[0] = r;
        pixel[1] = g;
        pixel[2] = b;
    });

    DynamicImage::ImageRgba8(rgba)
}

/// Apply sepia tone. `strength` 0.0 = original, 1.0 = full sepia.
pub fn sepia(image: &DynamicImage, strength: f32) -> DynamicImage {
    let strength = strength.clamp(0.0, 1.0);
    if strength < f32::EPSILON {
        return image.clone();
    }

    let mut rgba = image.to_rgba8();
    rgba.par_chunks_mut(4).for_each(|pixel| {
        let r = f32::from(pixel[0]);
        let g = f32::from(pixel[1]);
        let b = f32::from(pixel[2]);
        let sepia_r = (r * 0.393 + g * 0.769 + b * 0.189).clamp(0.0, 255.0);
        let sepia_g = (r * 0.349 + g * 0.686 + b * 0.168).clamp(0.0, 255.0);
        let sepia_b = (r * 0.272 + g * 0.534 + b * 0.131).clamp(0.0, 255.0);
        pixel[0] = lerp_channel(r, sepia_r, strength);
        pixel[1] = lerp_channel(g, sepia_g, strength);
        pixel[2] = lerp_channel(b, sepia_b, strength);
    });

    DynamicImage::ImageRgba8(rgba)
}

/// Vignette darkens edges. `strength` 0.0 = none, 1.0 = strong.
pub fn vignette(image: &DynamicImage, strength: f32) -> DynamicImage {
    let strength = strength.clamp(0.0, 1.0);
    if strength < f32::EPSILON {
        return image.clone();
    }

    let mut rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let center_x = width as f32 / 2.0;
    let center_y = height as f32 / 2.0;
    let max_dist = (center_x.hypot(center_y)).max(1.0);

    if width == 0 || height == 0 {
        return DynamicImage::ImageRgba8(rgba);
    }

    rgba.par_chunks_mut(width as usize * 4)
        .enumerate()
        .for_each(|(y, row)| {
            let dy = y as f32 + 0.5 - center_y;
            for (x, pixel) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let dx = x as f32 + 0.5 - center_x;
                let dist = dx.hypot(dy) / max_dist;
                let factor = 1.0 - strength * dist.powf(1.8);
                pixel[0] = (f32::from(pixel[0]) * factor).round().clamp(0.0, 255.0) as u8;
                pixel[1] = (f32::from(pixel[1]) * factor).round().clamp(0.0, 255.0) as u8;
                pixel[2] = (f32::from(pixel[2]) * factor).round().clamp(0.0, 255.0) as u8;
            }
        });

    DynamicImage::ImageRgba8(rgba)
}

/// Warm/cool color temperature. Negative = cooler (blue), positive = warmer (orange).
pub fn adjust_temperature(image: &DynamicImage, amount: f32) -> DynamicImage {
    if amount.abs() < f32::EPSILON {
        return image.clone();
    }

    let mut rgba = image.to_rgba8();
    let warm = amount.clamp(-100.0, 100.0) / 100.0;

    rgba.par_chunks_mut(4).for_each(|pixel| {
        let r = f32::from(pixel[0]);
        let g = f32::from(pixel[1]);
        let b = f32::from(pixel[2]);
        pixel[0] = clamp_channel(r + warm * 40.0);
        pixel[1] = clamp_channel(g + warm * 10.0);
        pixel[2] = clamp_channel(b - warm * 40.0);
    });

    DynamicImage::ImageRgba8(rgba)
}

/// Green/magenta tint. Negative = green, positive = magenta.
pub fn adjust_tint(image: &DynamicImage, amount: f32) -> DynamicImage {
    if amount.abs() < f32::EPSILON {
        return image.clone();
    }

    let mut rgba = image.to_rgba8();
    let tint = amount.clamp(-100.0, 100.0) / 100.0;

    rgba.par_chunks_mut(4).for_each(|pixel| {
        let r = f32::from(pixel[0]);
        let g = f32::from(pixel[1]);
        let b = f32::from(pixel[2]);
        pixel[0] = clamp_channel(r + tint * 30.0);
        pixel[1] = clamp_channel(g - tint * 20.0);
        pixel[2] = clamp_channel(b + tint * 30.0);
    });

    DynamicImage::ImageRgba8(rgba)
}

/// Stretch histogram to full 0-255 range per channel (auto levels).
pub fn auto_contrast(image: &DynamicImage) -> DynamicImage {
    let mut rgba = image.to_rgba8();
    let mut min = [255u8; 3];
    let mut max = [0u8; 3];

    for pixel in rgba.pixels() {
        for channel in 0..3 {
            min[channel] = min[channel].min(pixel[channel]);
            max[channel] = max[channel].max(pixel[channel]);
        }
    }

    if min == [255, 255, 255] && max == [0, 0, 0] {
        return image.clone();
    }

    // Per-channel 256-entry lookup tables; identity where the range is flat.
    let luts: [[u8; 256]; 3] = std::array::from_fn(|channel| {
        let lo = min[channel];
        let hi = max[channel];
        std::array::from_fn(|v| {
            let v = v as u8;
            if hi > lo && v >= lo {
                let value = f32::from(v - lo) / f32::from(hi - lo);
                (value * 255.0).round().clamp(0.0, 255.0) as u8
            } else {
                v
            }
        })
    });
    rgba.par_chunks_mut(4).for_each(|pixel| {
        for channel in 0..3 {
            pixel[channel] = luts[channel][usize::from(pixel[channel])];
        }
    });

    DynamicImage::ImageRgba8(rgba)
}

/// Posterize to `levels` per channel (2..=32).
pub fn posterize(image: &DynamicImage, levels: u8) -> DynamicImage {
    let levels = levels.clamp(2, 32);
    let step = 255.0 / f32::from(levels - 1);
    let mut rgba = image.to_rgba8();

    let lut: [u8; 256] = std::array::from_fn(|v| {
        let quantized = (v as f32 / step).round() * step;
        quantized.round().clamp(0.0, 255.0) as u8
    });
    rgba.par_chunks_mut(4).for_each(|pixel| {
        for channel in &mut pixel[..3] {
            *channel = lut[usize::from(*channel)];
        }
    });

    DynamicImage::ImageRgba8(rgba)
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

    use super::*;

    #[test]
    fn rotate_180_flips_pixels() {
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 1, Rgba([10, 20, 30, 255])));
        let result = rotate_180(&image).to_rgba8();
        assert_eq!(result.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }

    #[test]
    fn hue_zero_is_identity() {
        let image =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([100, 50, 200, 255])));
        let result = adjust_hue(&image, 0.0).to_rgba8();
        assert_eq!(result.get_pixel(0, 0).0, [100, 50, 200, 255]);
    }

    #[test]
    fn sepia_preserves_alpha() {
        let image =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([200, 100, 50, 128])));
        let result = sepia(&image, 1.0).to_rgba8();
        assert_eq!(result.get_pixel(0, 0)[3], 128);
    }

    #[test]
    fn auto_contrast_stretches_range() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([50, 50, 50, 255]));
        img.put_pixel(1, 0, Rgba([150, 150, 150, 255]));
        let result = auto_contrast(&DynamicImage::ImageRgba8(img)).to_rgba8();
        assert_eq!(result.get_pixel(0, 0)[0], 0);
        assert_eq!(result.get_pixel(1, 0)[0], 255);
    }

    #[test]
    fn posterize_reduces_levels() {
        let image =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([127, 127, 127, 255])));
        let result = posterize(&image, 2).to_rgba8();
        assert!(result.get_pixel(0, 0)[0] == 0 || result.get_pixel(0, 0)[0] == 255);
    }

    // ── Serial reference implementations (pre-parallelization behaviour) ──

    fn gradient(w: u32, h: u32) -> DynamicImage {
        let mut img = RgbaImage::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = Rgba([
                ((x * 37 + y * 11) % 256) as u8,
                ((x * 5 + y * 53 + 17) % 256) as u8,
                ((x * 91 + y * 7 + 40) % 256).min(230) as u8,
                ((x + y * 3) % 256) as u8,
            ]);
        }
        DynamicImage::ImageRgba8(img)
    }

    fn serial_map(image: &DynamicImage, f: impl Fn(u32, u32, &mut Rgba<u8>)) -> RgbaImage {
        let mut rgba = image.to_rgba8();
        for (x, y, p) in rgba.enumerate_pixels_mut() {
            f(x, y, p);
        }
        rgba
    }

    #[test]
    fn parallel_filters_match_serial_reference() {
        let img = gradient(37, 23);

        let shift = 75.0f32.rem_euclid(360.0) / 360.0;
        let want = serial_map(&img, |_, _, p| {
            let (h, s, l) = rgb_to_hsl(p[0], p[1], p[2]);
            let (r, g, b) = hsl_to_rgb((h + shift).rem_euclid(1.0), s, l);
            p[0] = r;
            p[1] = g;
            p[2] = b;
        });
        assert_eq!(adjust_hue(&img, 75.0).to_rgba8(), want);

        let want = serial_map(&img, |_, _, p| {
            let (r, g, b) = (f32::from(p[0]), f32::from(p[1]), f32::from(p[2]));
            let sr = (r * 0.393 + g * 0.769 + b * 0.189).clamp(0.0, 255.0);
            let sg = (r * 0.349 + g * 0.686 + b * 0.168).clamp(0.0, 255.0);
            let sb = (r * 0.272 + g * 0.534 + b * 0.131).clamp(0.0, 255.0);
            p[0] = lerp_channel(r, sr, 0.6);
            p[1] = lerp_channel(g, sg, 0.6);
            p[2] = lerp_channel(b, sb, 0.6);
        });
        assert_eq!(sepia(&img, 0.6).to_rgba8(), want);

        let (cx, cy) = (37.0f32 / 2.0, 23.0f32 / 2.0);
        let max_dist = cx.hypot(cy).max(1.0);
        let want = serial_map(&img, |x, y, p| {
            let dist = (x as f32 + 0.5 - cx).hypot(y as f32 + 0.5 - cy) / max_dist;
            let factor = 1.0 - 0.7 * dist.powf(1.8);
            for c in 0..3 {
                p[c] = (f32::from(p[c]) * factor).round().clamp(0.0, 255.0) as u8;
            }
        });
        assert_eq!(vignette(&img, 0.7).to_rgba8(), want);

        let want = serial_map(&img, |_, _, p| {
            let warm = 0.35f32;
            p[0] = clamp_channel(f32::from(p[0]) + warm * 40.0);
            p[1] = clamp_channel(f32::from(p[1]) + warm * 10.0);
            p[2] = clamp_channel(f32::from(p[2]) - warm * 40.0);
        });
        assert_eq!(adjust_temperature(&img, 35.0).to_rgba8(), want);

        let want = serial_map(&img, |_, _, p| {
            let t = -0.4f32;
            p[0] = clamp_channel(f32::from(p[0]) + t * 30.0);
            p[1] = clamp_channel(f32::from(p[1]) - t * 20.0);
            p[2] = clamp_channel(f32::from(p[2]) + t * 30.0);
        });
        assert_eq!(adjust_tint(&img, -40.0).to_rgba8(), want);

        for levels in [2u8, 3, 5, 32] {
            let step = 255.0 / f32::from(levels - 1);
            let want = serial_map(&img, |_, _, p| {
                for c in 0..3 {
                    let q = (f32::from(p[c]) / step).round() * step;
                    p[c] = q.round().clamp(0.0, 255.0) as u8;
                }
            });
            assert_eq!(posterize(&img, levels).to_rgba8(), want, "levels {levels}");
        }

        let src = img.to_rgba8();
        let mut lo = [255u8; 3];
        let mut hi = [0u8; 3];
        for p in src.pixels() {
            for c in 0..3 {
                lo[c] = lo[c].min(p[c]);
                hi[c] = hi[c].max(p[c]);
            }
        }
        let want = serial_map(&img, |_, _, p| {
            for c in 0..3 {
                if hi[c] > lo[c] {
                    let v = f32::from(p[c] - lo[c]) / f32::from(hi[c] - lo[c]);
                    p[c] = (v * 255.0).round().clamp(0.0, 255.0) as u8;
                }
            }
        });
        assert_eq!(auto_contrast(&img).to_rgba8(), want);
    }

    // ── Filter trait tests ──

    #[test]
    fn filter_pipeline_empty_is_noop() {
        let image =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([100, 100, 100, 255])));
        let pipeline = FilterPipeline::new();
        let result = pipeline.apply_all(&image);
        assert_eq!(result.as_bytes(), image.as_bytes());
    }

    #[test]
    fn filter_pipeline_applies_in_order() {
        let image =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([200, 100, 50, 255])));
        let mut pipeline = FilterPipeline::new();
        pipeline.push(Box::new(Rotate180));
        pipeline.push(Box::new(Sepia { strength: 0.5 }));
        let result = pipeline.apply_all(&image);
        // Should not panic and produce a valid image
        assert_eq!(result.dimensions(), image.dimensions());
    }

    #[test]
    fn filter_name_is_not_empty() {
        let filters: Vec<Box<dyn Filter>> = vec![
            Box::new(Rotate180),
            Box::new(HueAdjust { degrees: 0.0 }),
            Box::new(Sepia { strength: 0.5 }),
            Box::new(Vignette { strength: 0.3 }),
            Box::new(TemperatureAdjust { amount: 10.0 }),
            Box::new(TintAdjust { amount: 5.0 }),
            Box::new(AutoContrast),
            Box::new(Posterize { levels: 4 }),
        ];
        for f in &filters {
            assert!(!f.name().is_empty(), "Filter name should not be empty");
        }
    }
}
