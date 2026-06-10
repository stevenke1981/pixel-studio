use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use image::{DynamicImage, RgbaImage};
use studio_core::gpano_xmp::contains_gpano_marker;
use studio_core::{
    C2paAddOptions, C2paManifestDraft, C2paRemovalOptions, ConvertOptions, PanoramaMode,
    PanoramaOptions, SupportedFormat, convert_image, inspect_c2pa, inspect_image,
    process_c2pa_add_batch, process_c2pa_removal_batch, remove_c2pa_manifest, write_c2pa_manifest,
    write_panorama_jpeg,
};

#[test]
fn converts_png_to_jpeg() {
    let dir = temp_dir("convert_png_to_jpeg");
    fs::create_dir_all(&dir).unwrap();
    let input = dir.join("input.png");
    let output = dir.join("output.jpg");
    DynamicImage::ImageRgba8(RgbaImage::new(16, 16))
        .save(&input)
        .unwrap();

    let result = convert_image(
        &input,
        &output,
        ConvertOptions {
            format: SupportedFormat::Jpeg,
            quality: 90,
            background: [255, 255, 255, 255],
        },
    )
    .unwrap();

    assert_eq!(result.format, SupportedFormat::Jpeg);
    assert_eq!((result.width, result.height), (16, 16));
    assert!(output.exists());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn writes_decodable_gpano_jpeg() {
    let dir = temp_dir("writes_decodable_gpano_jpeg");
    fs::create_dir_all(&dir).unwrap();
    let input = dir.join("input.png");
    let output = dir.join("output_360.jpg");
    DynamicImage::ImageRgba8(RgbaImage::new(80, 60))
        .save(&input)
        .unwrap();

    let result = write_panorama_jpeg(
        &input,
        &output,
        PanoramaOptions {
            mode: PanoramaMode::Pad,
            target_width: Some(100),
            quality: 90,
            background: [0, 0, 0, 255],
        },
    )
    .unwrap();

    assert_eq!((result.width, result.height), (100, 50));
    let bytes = fs::read(&output).unwrap();
    assert!(contains_gpano_marker(&bytes));

    let info = inspect_image(&output).unwrap();
    assert_eq!((info.width, info.height), (100, 50));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn reports_missing_c2pa_manifest_for_plain_image() {
    let dir = temp_dir("reports_missing_c2pa_manifest_for_plain_image");
    fs::create_dir_all(&dir).unwrap();
    let input = dir.join("plain.jpg");
    DynamicImage::ImageRgba8(RgbaImage::new(16, 16))
        .save(&input)
        .unwrap();

    let info = inspect_c2pa(&input).unwrap();

    assert!(!info.present);
    assert_eq!(info.manifest_count, 0);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn writes_and_reads_c2pa_manifest() {
    let dir = temp_dir("writes_and_reads_c2pa_manifest");
    fs::create_dir_all(&dir).unwrap();
    let input = dir.join("input.jpg");
    let output = dir.join("output.jpg");
    DynamicImage::ImageRgba8(RgbaImage::new(32, 24))
        .save(&input)
        .unwrap();

    write_c2pa_manifest(
        &input,
        &output,
        &C2paManifestDraft {
            title: "Edited portrait".to_string(),
            creator: "Steven".to_string(),
            action: "Adjusted color and crop".to_string(),
        },
    )
    .unwrap();

    let info = inspect_c2pa(&output).unwrap();

    assert!(info.present);
    assert_eq!(info.title.as_deref(), Some("Edited portrait"));
    assert_eq!(info.creator.as_deref(), Some("Steven"));
    assert_eq!(info.action.as_deref(), Some("Adjusted color and crop"));
    assert_eq!(info.manifest_count, 1);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn removes_c2pa_manifest_from_image_copy() {
    let dir = temp_dir("removes_c2pa_manifest_from_image_copy");
    fs::create_dir_all(&dir).unwrap();
    let input = dir.join("input.jpg");
    let signed = dir.join("signed.jpg");
    let stripped = dir.join("stripped.jpg");
    DynamicImage::ImageRgba8(RgbaImage::new(32, 24))
        .save(&input)
        .unwrap();

    write_c2pa_manifest(
        &input,
        &signed,
        &C2paManifestDraft {
            title: "Signed image".to_string(),
            creator: "Steven".to_string(),
            action: "Added C2PA".to_string(),
        },
    )
    .unwrap();
    assert!(inspect_c2pa(&signed).unwrap().present);

    remove_c2pa_manifest(&signed, &stripped).unwrap();

    let info = inspect_c2pa(&stripped).unwrap();
    assert!(!info.present);
    assert!(stripped.exists());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn batch_adds_c2pa_manifest_to_image_copies() {
    let dir = temp_dir("batch_adds_c2pa_manifest_to_image_copies");
    let input_dir = dir.join("input");
    let output_dir = dir.join("output");
    fs::create_dir_all(&input_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();
    let input = input_dir.join("plain.jpg");
    DynamicImage::ImageRgba8(RgbaImage::new(32, 24))
        .save(&input)
        .unwrap();

    let summary = process_c2pa_add_batch(
        std::slice::from_ref(&input),
        &C2paAddOptions {
            output_dir: output_dir.clone(),
            draft: C2paManifestDraft {
                title: "Batch image".to_string(),
                creator: "Pixel Studio Test".to_string(),
                action: "Added in batch".to_string(),
            },
        },
    );

    let output = output_dir.join("plain_c2pa.jpg");
    assert_eq!(summary.total, 1);
    assert_eq!(summary.succeeded, 1);
    assert!(output.exists());
    let info = inspect_c2pa(&output).unwrap();
    assert!(info.present);
    assert_eq!(info.title.as_deref(), Some("Batch image"));
    assert_eq!(info.creator.as_deref(), Some("Pixel Studio Test"));
    assert_eq!(info.action.as_deref(), Some("Added in batch"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn batch_removes_c2pa_manifest_from_image_copies() {
    let dir = temp_dir("batch_removes_c2pa_manifest_from_image_copies");
    let input_dir = dir.join("input");
    let output_dir = dir.join("output");
    fs::create_dir_all(&input_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();
    let input = input_dir.join("plain.jpg");
    let signed = input_dir.join("signed.jpg");
    DynamicImage::ImageRgba8(RgbaImage::new(32, 24))
        .save(&input)
        .unwrap();
    write_c2pa_manifest(
        &input,
        &signed,
        &C2paManifestDraft {
            title: "Signed image".to_string(),
            creator: "Pixel Studio Test".to_string(),
            action: "Signed before batch removal".to_string(),
        },
    )
    .unwrap();

    let summary = process_c2pa_removal_batch(
        std::slice::from_ref(&signed),
        &C2paRemovalOptions {
            output_dir: output_dir.clone(),
        },
    );

    let output = output_dir.join("signed_no_c2pa.jpg");
    assert_eq!(summary.total, 1);
    assert_eq!(summary.succeeded, 1);
    assert!(output.exists());
    assert!(!inspect_c2pa(&output).unwrap().present);

    let _ = fs::remove_dir_all(dir);
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("photo_tool_{name}_{nanos}"))
}
