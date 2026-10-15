use image::{DynamicImage, RgbaImage};
use studio_core::{ComposeDocument, ComposeLayer, ShapeKind, ShapeLayer, render_composition};

fn render(kind: ShapeKind, filled: bool) -> RgbaImage {
    let mut s = ShapeLayer::new(kind, 10, 10, 90, 70);
    s.filled = filled;
    s.fill_color = [0, 0, 255, 255];
    let doc = ComposeDocument {
        background: DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            100,
            80,
            image::Rgba([255, 255, 255, 255]),
        )),
        layers: vec![ComposeLayer::Shape(s)],
    };
    render_composition(&doc, None).to_rgba8()
}

#[test]
fn ellipse_and_triangle_render() {
    let e = render(ShapeKind::Ellipse, true);
    assert_eq!(e.get_pixel(50, 40).0, [0, 0, 255, 255], "filled centre");
    assert_eq!(
        e.get_pixel(11, 11).0,
        [255, 255, 255, 255],
        "corner outside ellipse"
    );
    assert!(e.get_pixel(10, 40)[1] < 50, "red outline at left edge");
    let t = render(ShapeKind::Triangle, false);
    assert_eq!(t.get_pixel(50, 50).0, [255, 255, 255, 255], "hollow");
    assert!(t.get_pixel(50, 69)[1] < 50, "base line");
}
