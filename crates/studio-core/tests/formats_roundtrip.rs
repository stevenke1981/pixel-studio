use image::{DynamicImage, Rgba, RgbaImage};
use studio_core::{ConvertOptions, SupportedFormat, load_dynamic_image, save_dynamic_image};

fn sample() -> DynamicImage {
    let mut img = RgbaImage::from_pixel(300, 200, Rgba([10, 120, 200, 255]));
    for x in 0..300 {
        img.put_pixel(x, 100, Rgba([255, 0, 0, 128]));
    }
    DynamicImage::ImageRgba8(img)
}

#[test]
fn every_format_round_trips() {
    let dir = std::env::temp_dir().join(format!("ps-fmt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for format in SupportedFormat::ALL {
        let path = dir.join(format!("t.{}", format.extension()));
        save_dynamic_image(&sample(), &path, ConvertOptions { format, quality: 100, ..Default::default() }).unwrap();
        let back = load_dynamic_image(&path).unwrap();
        if format == SupportedFormat::Ico {
            assert_eq!((back.width(), back.height()), (256, 171), "ico downscaled");
        } else {
            assert_eq!((back.width(), back.height()), (300, 200), "{format}");
        }
    }
}

#[test]
fn webp_lossless_vs_lossy() {
    let dir = std::env::temp_dir().join(format!("ps-webp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut noisy = RgbaImage::new(256, 256);
    for (x, y, p) in noisy.enumerate_pixels_mut() {
        *p = Rgba([(x * 7 ^ y * 13) as u8, (x * y) as u8, (x + y) as u8, 255]);
    }
    let img = DynamicImage::ImageRgba8(noisy);
    let lossless = dir.join("a.webp");
    let lossy = dir.join("b.webp");
    save_dynamic_image(&img, &lossless, ConvertOptions { format: SupportedFormat::Webp, quality: 100, ..Default::default() }).unwrap();
    save_dynamic_image(&img, &lossy, ConvertOptions { format: SupportedFormat::Webp, quality: 50, ..Default::default() }).unwrap();
    assert_eq!(load_dynamic_image(&lossless).unwrap().to_rgba8(), img.to_rgba8(), "lossless exact");
    let a = std::fs::metadata(&lossless).unwrap().len();
    let b = std::fs::metadata(&lossy).unwrap().len();
    assert!(b < a, "lossy {b} < lossless {a}");
}

#[test]
fn export_strips_exif_unless_kept() {
    let dir = std::env::temp_dir().join(format!("ps-exif-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // Build a JPEG carrying an EXIF block.
    let src = dir.join("src.jpg");
    save_dynamic_image(&sample(), &src, ConvertOptions { format: SupportedFormat::Jpeg, ..Default::default() }).unwrap();
    {
        use img_parts::{Bytes, ImageEXIF, jpeg::Jpeg};
        let mut j = Jpeg::from_bytes(Bytes::from(std::fs::read(&src).unwrap())).unwrap();
        j.set_exif(Some(Bytes::from_static(b"Exif\0\0MM\0*\0\0\0\x08\0\0")));
        let mut out = Vec::new();
        j.encoder().write_to(&mut out).unwrap();
        std::fs::write(&src, out).unwrap();
    }
    assert!(studio_core::metadata::has_exif(&src));
    let img = load_dynamic_image(&src).unwrap();
    let stripped = dir.join("out.jpg");
    save_dynamic_image(&img, &stripped, ConvertOptions { format: SupportedFormat::Jpeg, ..Default::default() }).unwrap();
    assert!(!studio_core::metadata::has_exif(&stripped));
    studio_core::metadata::copy_metadata(&src, &stripped).unwrap();
    assert!(studio_core::metadata::has_exif(&stripped));
}
