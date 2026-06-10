use image::DynamicImage;

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

    for pixel in rgba.pixels_mut() {
        let (h, s, l) = rgb_to_hsl(pixel[0], pixel[1], pixel[2]);
        let new_h = (h + shift).rem_euclid(1.0);
        let (r, g, b) = hsl_to_rgb(new_h, s, l);
        pixel[0] = r;
        pixel[1] = g;
        pixel[2] = b;
    }

    DynamicImage::ImageRgba8(rgba)
}

/// Apply sepia tone. `strength` 0.0 = original, 1.0 = full sepia.
pub fn sepia(image: &DynamicImage, strength: f32) -> DynamicImage {
    let strength = strength.clamp(0.0, 1.0);
    if strength < f32::EPSILON {
        return image.clone();
    }

    let mut rgba = image.to_rgba8();
    for pixel in rgba.pixels_mut() {
        let r = f32::from(pixel[0]);
        let g = f32::from(pixel[1]);
        let b = f32::from(pixel[2]);
        let sepia_r = (r * 0.393 + g * 0.769 + b * 0.189).clamp(0.0, 255.0);
        let sepia_g = (r * 0.349 + g * 0.686 + b * 0.168).clamp(0.0, 255.0);
        let sepia_b = (r * 0.272 + g * 0.534 + b * 0.131).clamp(0.0, 255.0);
        pixel[0] = lerp_channel(r, sepia_r, strength);
        pixel[1] = lerp_channel(g, sepia_g, strength);
        pixel[2] = lerp_channel(b, sepia_b, strength);
    }

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

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 + 0.5 - center_x;
            let dy = y as f32 + 0.5 - center_y;
            let dist = dx.hypot(dy) / max_dist;
            let factor = 1.0 - strength * dist.powf(1.8);
            let pixel = rgba.get_pixel_mut(x, y);
            pixel[0] = (f32::from(pixel[0]) * factor).round().clamp(0.0, 255.0) as u8;
            pixel[1] = (f32::from(pixel[1]) * factor).round().clamp(0.0, 255.0) as u8;
            pixel[2] = (f32::from(pixel[2]) * factor).round().clamp(0.0, 255.0) as u8;
        }
    }

    DynamicImage::ImageRgba8(rgba)
}

/// Warm/cool color temperature. Negative = cooler (blue), positive = warmer (orange).
pub fn adjust_temperature(image: &DynamicImage, amount: f32) -> DynamicImage {
    if amount.abs() < f32::EPSILON {
        return image.clone();
    }

    let mut rgba = image.to_rgba8();
    let warm = amount.clamp(-100.0, 100.0) / 100.0;

    for pixel in rgba.pixels_mut() {
        let r = f32::from(pixel[0]);
        let g = f32::from(pixel[1]);
        let b = f32::from(pixel[2]);
        pixel[0] = clamp_channel(r + warm * 40.0);
        pixel[1] = clamp_channel(g + warm * 10.0);
        pixel[2] = clamp_channel(b - warm * 40.0);
    }

    DynamicImage::ImageRgba8(rgba)
}

/// Green/magenta tint. Negative = green, positive = magenta.
pub fn adjust_tint(image: &DynamicImage, amount: f32) -> DynamicImage {
    if amount.abs() < f32::EPSILON {
        return image.clone();
    }

    let mut rgba = image.to_rgba8();
    let tint = amount.clamp(-100.0, 100.0) / 100.0;

    for pixel in rgba.pixels_mut() {
        let r = f32::from(pixel[0]);
        let g = f32::from(pixel[1]);
        let b = f32::from(pixel[2]);
        pixel[0] = clamp_channel(r + tint * 30.0);
        pixel[1] = clamp_channel(g - tint * 20.0);
        pixel[2] = clamp_channel(b + tint * 30.0);
    }

    DynamicImage::ImageRgba8(rgba)
}

/// Stretch histogram to full 0-255 range per channel (auto levels).
pub fn auto_contrast(image: &DynamicImage) -> DynamicImage {
    let rgba = image.to_rgba8();
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

    let mut output = rgba.clone();
    for pixel in output.pixels_mut() {
        for channel in 0..3 {
            let lo = min[channel];
            let hi = max[channel];
            if hi > lo {
                let value = f32::from(pixel[channel] - lo) / f32::from(hi - lo);
                pixel[channel] = (value * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    DynamicImage::ImageRgba8(output)
}

/// Posterize to `levels` per channel (2..=32).
pub fn posterize(image: &DynamicImage, levels: u8) -> DynamicImage {
    let levels = levels.clamp(2, 32);
    let step = 255.0 / f32::from(levels - 1);
    let mut rgba = image.to_rgba8();

    for pixel in rgba.pixels_mut() {
        for channel in 0..3 {
            let quantized = (f32::from(pixel[channel]) / step).round() * step;
            pixel[channel] = quantized.round().clamp(0.0, 255.0) as u8;
        }
    }

    DynamicImage::ImageRgba8(rgba)
}

fn lerp_channel(from: f32, to: f32, t: f32) -> u8 {
    clamp_channel(from + (to - from) * t)
}

fn clamp_channel(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = f32::from(r) / 255.0;
    let g = f32::from(g) / 255.0;
    let b = f32::from(b) / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < f32::EPSILON {
        let mut hue = (g - b) / d;
        if g < b {
            hue += 6.0;
        }
        hue
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;

    (h.rem_euclid(1.0), s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    if s.abs() < f32::EPSILON {
        let v = clamp_channel(l * 255.0);
        return (v, v, v);
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;

    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);
    (
        clamp_channel(r * 255.0),
        clamp_channel(g * 255.0),
        clamp_channel(b * 255.0),
    )
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 1.0 / 2.0 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, Rgba, RgbaImage};

    use super::{adjust_hue, auto_contrast, posterize, rotate_180, sepia};

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
}
