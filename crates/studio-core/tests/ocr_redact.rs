//! Offline OCR auto-redaction on a fixture with known text positions.
//! Needs the ocrs models; downloads them on first run. Set PIXEL_STUDIO_SKIP_OCR=1 to skip.

use studio_core::redact::{RedactStyle, SensitiveKind, apply_redactions, select_by_keyword};

fn overlaps(b: &studio_core::redact::RedactBox, r: [i32; 4]) -> bool {
    let (x1, y1) = (b.x + b.width as i32, b.y + b.height as i32);
    b.x < r[2] && x1 > r[0] && b.y < r[3] && y1 > r[1]
}

#[test]
fn detects_and_redacts_sensitive_lines() {
    if std::env::var_os("PIXEL_STUDIO_SKIP_OCR").is_some() {
        return;
    }
    let img = image::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/redact_sample.png"
    ))
    .unwrap();
    let dir = studio_core::ocr::ensure_models().expect("models");
    let det = studio_core::ocr::TextDetector::load(&dir).unwrap();
    let mut boxes = det.detect(&img).unwrap();
    // Known glyph bounds from the fixture generator (x0, y0, x1, y1).
    let expected = [
        ([30, 120, 342, 146], SensitiveKind::Phone),
        ([30, 163, 484, 191], SensitiveKind::Email),
        ([30, 204, 298, 230], SensitiveKind::TaiwanId),
        ([30, 246, 433, 272], SensitiveKind::CreditCard),
        ([30, 288, 430, 317], SensitiveKind::Url),
    ];
    for (rect, kind) in expected {
        assert!(
            boxes
                .iter()
                .any(|b| b.selected && b.kind == Some(kind) && overlaps(b, rect)),
            "{kind:?} not pre-selected near {rect:?}: {boxes:#?}"
        );
    }
    // Chinese-only lines are still *detected* (script-agnostic detector).
    assert!(
        boxes.iter().any(|b| overlaps(b, [30, 330, 434, 356])),
        "address line detected"
    );
    // Non-sensitive line is not pre-selected.
    assert!(
        !boxes
            .iter()
            .any(|b| b.selected && overlaps(b, [30, 372, 509, 401]) && b.text.contains("Meeting"))
    );
    assert!(select_by_keyword(&mut boxes, "meeting") >= 1);

    let (out, n) = apply_redactions(&img, &boxes, RedactStyle::default(), 3);
    assert!(n >= 6);
    let out = out.to_rgba8();
    // Centre of the email line must be solid black now.
    assert_eq!(out.get_pixel(250, 177).0, [0, 0, 0, 255]);
    // Title bar untouched.
    assert_eq!(out.get_pixel(880, 5).0, [33, 150, 243, 255]);
}
