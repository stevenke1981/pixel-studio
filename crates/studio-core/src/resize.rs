use image::DynamicImage;
use image::imageops::FilterType;

pub fn resize_exact(image: &DynamicImage, width: u32, height: u32) -> DynamicImage {
    image.resize_exact(width.max(1), height.max(1), FilterType::Lanczos3)
}

pub fn resize_to_fit(image: &DynamicImage, max_width: u32, max_height: u32) -> DynamicImage {
    image.resize(max_width.max(1), max_height.max(1), FilterType::Lanczos3)
}
