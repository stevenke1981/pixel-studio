use image::imageops::FilterType;
use image::{DynamicImage, Rgba, RgbaImage};

pub fn resize_exact(image: &DynamicImage, width: u32, height: u32) -> DynamicImage {
    image.resize_exact(width.max(1), height.max(1), FilterType::Lanczos3)
}

pub fn resize_to_fit(image: &DynamicImage, max_width: u32, max_height: u32) -> DynamicImage {
    image.resize(max_width.max(1), max_height.max(1), FilterType::Lanczos3)
}

/// Resizes canvas dimensions without scaling the contents.
/// Existing pixels are preserved starting at the top-left (0, 0).
/// Any newly exposed region is filled with `bg_color`.
pub fn canvas_resize(
    img: &RgbaImage,
    new_width: u32,
    new_height: u32,
    bg_color: Rgba<u8>,
) -> RgbaImage {
    let mut new_img = RgbaImage::from_pixel(new_width, new_height, bg_color);
    let copy_width = img.width().min(new_width);
    let copy_height = img.height().min(new_height);

    for y in 0..copy_height {
        for x in 0..copy_width {
            new_img.put_pixel(x, y, *img.get_pixel(x, y));
        }
    }

    new_img
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_resize_expand() {
        let mut original = RgbaImage::from_pixel(10, 10, Rgba([255, 0, 0, 255]));
        original.put_pixel(0, 0, Rgba([0, 255, 0, 255]));
        let bg = Rgba([255, 255, 255, 255]);

        let resized = canvas_resize(&original, 20, 30, bg);
        assert_eq!(resized.width(), 20);
        assert_eq!(resized.height(), 30);

        // Preserved top-left
        assert_eq!(*resized.get_pixel(0, 0), Rgba([0, 255, 0, 255]));
        assert_eq!(*resized.get_pixel(9, 9), Rgba([255, 0, 0, 255]));

        // Expanded area filled with bg
        assert_eq!(*resized.get_pixel(15, 5), bg);
        assert_eq!(*resized.get_pixel(5, 20), bg);
        assert_eq!(*resized.get_pixel(19, 29), bg);
    }

    #[test]
    fn test_canvas_resize_shrink() {
        let original = RgbaImage::from_pixel(50, 50, Rgba([100, 150, 200, 255]));
        let bg = Rgba([0, 0, 0, 255]);

        let resized = canvas_resize(&original, 25, 20, bg);
        assert_eq!(resized.width(), 25);
        assert_eq!(resized.height(), 20);
        assert_eq!(*resized.get_pixel(0, 0), Rgba([100, 150, 200, 255]));
        assert_eq!(*resized.get_pixel(24, 19), Rgba([100, 150, 200, 255]));
    }

    #[test]
    fn test_canvas_resize_empty() {
        let original = RgbaImage::from_pixel(10, 10, Rgba([255, 0, 0, 255]));
        let bg = Rgba([0, 0, 0, 255]);

        let resized = canvas_resize(&original, 0, 0, bg);
        assert_eq!(resized.width(), 0);
        assert_eq!(resized.height(), 0);
    }
}
