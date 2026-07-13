use image::{DynamicImage, GenericImageView};

use crate::{PhotoError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMask {
    Rectangle {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    Circle {
        x: u32,
        y: u32,
        diameter: u32,
    },
}

pub fn rotate_left(image: &DynamicImage) -> DynamicImage {
    image.rotate270()
}

pub fn rotate_right(image: &DynamicImage) -> DynamicImage {
    image.rotate90()
}

pub fn flip_horizontal(image: &DynamicImage) -> DynamicImage {
    image.fliph()
}

pub fn flip_vertical(image: &DynamicImage) -> DynamicImage {
    image.flipv()
}

pub fn adjust_brightness(image: &DynamicImage, value: i32) -> DynamicImage {
    image.brighten(value)
}

pub fn adjust_contrast(image: &DynamicImage, value: f32) -> DynamicImage {
    image.adjust_contrast(value)
}

pub fn adjust_saturation(image: &DynamicImage, value: f32) -> DynamicImage {
    let factor = 1.0 + value / 100.0;
    let mut rgba = image.to_rgba8();

    for pixel in rgba.pixels_mut() {
        let red = f32::from(pixel[0]);
        let green = f32::from(pixel[1]);
        let blue = f32::from(pixel[2]);
        let luma = red * 0.2126 + green * 0.7152 + blue * 0.0722;

        pixel[0] = clamp_channel(luma + (red - luma) * factor);
        pixel[1] = clamp_channel(luma + (green - luma) * factor);
        pixel[2] = clamp_channel(luma + (blue - luma) * factor);
    }

    DynamicImage::ImageRgba8(rgba)
}

pub fn adjust_exposure(image: &DynamicImage, stops: f32) -> DynamicImage {
    let factor = 2.0_f32.powf(stops);
    let mut rgba = image.to_rgba8();

    for pixel in rgba.pixels_mut() {
        pixel[0] = clamp_channel(f32::from(pixel[0]) * factor);
        pixel[1] = clamp_channel(f32::from(pixel[1]) * factor);
        pixel[2] = clamp_channel(f32::from(pixel[2]) * factor);
    }

    DynamicImage::ImageRgba8(rgba)
}

pub fn blur(image: &DynamicImage, sigma: f32) -> DynamicImage {
    image.blur(sigma.max(0.0))
}

/// Blurs only the requested rectangle while preserving every pixel outside it.
///
/// The blurred pixels are sampled from the complete image so content at the
/// selection edge is obscured as well. Invalid or empty rectangles are
/// rejected instead of silently producing a misleading privacy edit.
pub fn blur_region(
    image: &DynamicImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    sigma: f32,
) -> Result<DynamicImage> {
    if !sigma.is_finite() || sigma <= 0.0 {
        return Err(PhotoError::InvalidInput(
            "blur sigma must be a positive finite number".to_string(),
        ));
    }
    let (x, y, width, height) = validate_region(image, x, y, width, height)?;
    let blurred = blur(image, sigma);
    Ok(apply_selection_mask(
        image,
        &blurred,
        SelectionMask::Rectangle {
            x,
            y,
            width,
            height,
        },
    ))
}

/// Returns a cropped copy of an image after validating the requested region.
pub fn crop_region(
    image: &DynamicImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<DynamicImage> {
    let (x, y, width, height) = validate_region(image, x, y, width, height)?;
    Ok(image.crop_imm(x, y, width, height))
}

pub fn sharpen(image: &DynamicImage, sigma: f32, threshold: i32) -> DynamicImage {
    image.unsharpen(sigma.max(0.1), threshold.max(0))
}

pub fn grayscale(image: &DynamicImage) -> DynamicImage {
    image.grayscale()
}

pub fn invert(image: &DynamicImage) -> DynamicImage {
    let mut image = image.clone();
    image.invert();
    image
}

pub fn apply_selection_mask(
    original: &DynamicImage,
    edited: &DynamicImage,
    mask: SelectionMask,
) -> DynamicImage {
    if original.dimensions() != edited.dimensions() {
        return edited.clone();
    }

    let mut output = original.to_rgba8();
    let edited = edited.to_rgba8();
    let (image_width, image_height) = output.dimensions();
    let (x, y, width, height) = mask_bounds(mask, image_width, image_height);

    for target_y in y..y.saturating_add(height) {
        for target_x in x..x.saturating_add(width) {
            if mask_contains(mask, target_x, target_y) {
                *output.get_pixel_mut(target_x, target_y) = *edited.get_pixel(target_x, target_y);
            }
        }
    }

    DynamicImage::ImageRgba8(output)
}

fn mask_bounds(mask: SelectionMask, image_width: u32, image_height: u32) -> (u32, u32, u32, u32) {
    let (x, y, width, height) = match mask {
        SelectionMask::Rectangle {
            x,
            y,
            width,
            height,
        } => (x, y, width, height),
        SelectionMask::Circle { x, y, diameter } => (x, y, diameter, diameter),
    };

    let x = x.min(image_width.saturating_sub(1));
    let y = y.min(image_height.saturating_sub(1));
    let width = width.min(image_width.saturating_sub(x)).max(1);
    let height = height.min(image_height.saturating_sub(y)).max(1);
    (x, y, width, height)
}

fn mask_contains(mask: SelectionMask, x: u32, y: u32) -> bool {
    match mask {
        SelectionMask::Rectangle { .. } => true,
        SelectionMask::Circle {
            x: mask_x,
            y: mask_y,
            diameter,
        } => {
            let radius = diameter as f32 / 2.0;
            let center_x = mask_x as f32 + radius;
            let center_y = mask_y as f32 + radius;
            let dx = x as f32 + 0.5 - center_x;
            let dy = y as f32 + 0.5 - center_y;
            dx * dx + dy * dy <= radius * radius
        }
    }
}

fn validate_region(
    image: &DynamicImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<(u32, u32, u32, u32)> {
    if width == 0 || height == 0 {
        return Err(PhotoError::InvalidInput(
            "region width and height must be greater than zero".to_string(),
        ));
    }

    let (image_width, image_height) = image.dimensions();
    if x >= image_width || y >= image_height {
        return Err(PhotoError::InvalidInput(format!(
            "region origin ({x}, {y}) is outside image bounds {image_width}x{image_height}"
        )));
    }

    let end_x = x
        .checked_add(width)
        .ok_or_else(|| PhotoError::InvalidInput("region horizontal bounds overflow".to_string()))?;
    let end_y = y
        .checked_add(height)
        .ok_or_else(|| PhotoError::InvalidInput("region vertical bounds overflow".to_string()))?;
    if end_x > image_width || end_y > image_height {
        return Err(PhotoError::InvalidInput(format!(
            "region {x},{y} {width}x{height} exceeds image bounds {image_width}x{image_height}"
        )));
    }

    Ok((x, y, width, height))
}

fn clamp_channel(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

    use super::{
        SelectionMask, adjust_exposure, adjust_saturation, apply_selection_mask, blur_region,
        crop_region, invert,
    };

    #[test]
    fn saturation_zero_preserves_alpha() {
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([200, 40, 40, 77])));
        let result = adjust_saturation(&image, -100.0).to_rgba8();
        assert_eq!(result.get_pixel(0, 0)[3], 77);
        assert_eq!(result.get_pixel(0, 0)[0], result.get_pixel(0, 0)[1]);
        assert_eq!(result.get_pixel(0, 0)[1], result.get_pixel(0, 0)[2]);
    }

    #[test]
    fn exposure_lifts_rgb_and_keeps_alpha() {
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([10, 20, 30, 99])));
        let result = adjust_exposure(&image, 1.0).to_rgba8();
        assert_eq!(result.get_pixel(0, 0).0, [20, 40, 60, 99]);
    }

    #[test]
    fn invert_flips_rgb() {
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([10, 20, 30, 255])));
        let result = invert(&image).to_rgba8();
        assert_eq!(result.get_pixel(0, 0).0, [245, 235, 225, 255]);
    }

    #[test]
    fn rectangle_selection_copies_only_selected_pixels() {
        let original = DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 255])));
        let edited = DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 255])));
        let result = apply_selection_mask(
            &original,
            &edited,
            SelectionMask::Rectangle {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            },
        )
        .to_rgba8();

        assert_eq!(result.get_pixel(1, 1).0, [255, 0, 0, 255]);
        assert_eq!(result.get_pixel(0, 0).0, [0, 0, 0, 255]);
    }

    #[test]
    fn circle_selection_masks_corners() {
        let original = DynamicImage::ImageRgba8(RgbaImage::from_pixel(5, 5, Rgba([0, 0, 0, 255])));
        let edited = DynamicImage::ImageRgba8(RgbaImage::from_pixel(5, 5, Rgba([255, 0, 0, 255])));
        let result = apply_selection_mask(
            &original,
            &edited,
            SelectionMask::Circle {
                x: 0,
                y: 0,
                diameter: 5,
            },
        )
        .to_rgba8();

        assert_eq!(result.get_pixel(2, 2).0, [255, 0, 0, 255]);
        assert_eq!(result.get_pixel(0, 0).0, [0, 0, 0, 255]);
    }

    #[test]
    fn region_blur_preserves_pixels_outside_selection() {
        let mut source = RgbaImage::new(7, 7);
        for (x, y, pixel) in source.enumerate_pixels_mut() {
            *pixel = if (x + y) % 2 == 0 {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 255])
            };
        }
        let image = DynamicImage::ImageRgba8(source.clone());
        let result = blur_region(&image, 2, 2, 3, 3, 8.0).unwrap().to_rgba8();

        assert_eq!(result.get_pixel(0, 0), source.get_pixel(0, 0));
        assert_ne!(result.get_pixel(3, 3), source.get_pixel(3, 3));
    }

    #[test]
    fn crop_region_returns_requested_dimensions() {
        let image = DynamicImage::ImageRgba8(RgbaImage::new(10, 8));
        let result = crop_region(&image, 2, 1, 5, 4).unwrap();
        assert_eq!(result.dimensions(), (5, 4));
    }

    #[test]
    fn crop_region_rejects_out_of_bounds_selection() {
        let image = DynamicImage::ImageRgba8(RgbaImage::new(10, 8));
        assert!(crop_region(&image, 8, 1, 5, 4).is_err());
        assert!(crop_region(&image, 0, 0, 0, 4).is_err());
    }

    #[test]
    fn region_blur_rejects_non_positive_sigma() {
        let image = DynamicImage::ImageRgba8(RgbaImage::new(10, 8));
        assert!(blur_region(&image, 0, 0, 4, 4, 0.0).is_err());
        assert!(blur_region(&image, 0, 0, 4, 4, f32::NAN).is_err());
    }
}
