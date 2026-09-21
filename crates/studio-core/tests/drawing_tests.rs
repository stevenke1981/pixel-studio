use image::{Rgba, RgbaImage};
use studio_core::drawing::{
    apply_mosaic_region, draw_airbrush, draw_brush_stroke, draw_calligraphy_stroke, draw_circle,
    draw_ellipse, draw_eraser, draw_highlighter_stroke, draw_pencil_line, draw_rectangle,
    flood_fill, sample_color,
};
use studio_core::resize::canvas_resize;

#[test]
fn test_pencil_line_bounds_and_clipping() {
    let mut img = RgbaImage::from_pixel(15, 15, Rgba([255, 255, 255, 255]));
    let black = Rgba([0, 0, 0, 255]);

    // Line extending outside image bounds: (-5, 5) -> (20, 5)
    draw_pencil_line(&mut img, -5, 5, 20, 5, black);

    for x in 0..15 {
        assert_eq!(*img.get_pixel(x, 5), black);
    }
    assert_eq!(*img.get_pixel(0, 4), Rgba([255, 255, 255, 255]));
    assert_eq!(*img.get_pixel(0, 6), Rgba([255, 255, 255, 255]));
}

#[test]
fn test_pencil_single_point() {
    let mut img = RgbaImage::from_pixel(10, 10, Rgba([255, 255, 255, 255]));
    let red = Rgba([255, 0, 0, 255]);
    draw_pencil_line(&mut img, 4, 4, 4, 4, red);

    assert_eq!(*img.get_pixel(4, 4), red);
    assert_eq!(*img.get_pixel(3, 4), Rgba([255, 255, 255, 255]));
    assert_eq!(*img.get_pixel(5, 4), Rgba([255, 255, 255, 255]));
}

#[test]
fn test_brush_stroke_continuity() {
    let mut img = RgbaImage::from_pixel(50, 50, Rgba([255, 255, 255, 255]));
    let blue = Rgba([0, 0, 255, 255]);

    // Diagonal brush stroke with radius 3
    draw_brush_stroke(&mut img, 10, 10, 40, 40, 3.0, blue);

    // Every pixel on the central line must be blue
    for i in 10..=40 {
        assert_eq!(*img.get_pixel(i, i), blue);
    }

    // Neighbors within radius 2 must also be blue
    for i in 12..=38 {
        assert_eq!(*img.get_pixel(i - 1, i + 1), blue);
        assert_eq!(*img.get_pixel(i + 1, i - 1), blue);
    }

    // Distant pixels remain white
    assert_eq!(*img.get_pixel(0, 0), Rgba([255, 255, 255, 255]));
    assert_eq!(*img.get_pixel(49, 0), Rgba([255, 255, 255, 255]));
}

#[test]
fn test_highlighter_uniform_alpha() {
    let mut img = RgbaImage::from_pixel(30, 30, Rgba([255, 255, 255, 255]));
    let yellow = Rgba([255, 255, 0, 255]);

    draw_highlighter_stroke(&mut img, 5, 15, 25, 15, 2.5, yellow, 100);

    // Check that along the stroke, pixels have consistent color and alpha
    let p1 = img.get_pixel(10, 15);
    let p2 = img.get_pixel(20, 15);
    assert_eq!(p1, p2);
    assert_eq!(p1[0], 255);
    assert_eq!(p1[1], 255);
    assert!(p1[2] < 255); // Yellow tint over white reduces blue
}

#[test]
fn test_airbrush_density_distribution() {
    let mut img = RgbaImage::from_pixel(60, 60, Rgba([255, 255, 255, 255]));
    let magenta = Rgba([255, 0, 255, 255]);

    draw_airbrush(&mut img, 30, 30, 15.0, 100, magenta);

    // Pixels far away (> 20 px) must not be affected
    assert_eq!(*img.get_pixel(5, 5), Rgba([255, 255, 255, 255]));
    assert_eq!(*img.get_pixel(55, 55), Rgba([255, 255, 255, 255]));

    // Inside radius, at least some pixels should be colored
    let mut colored_count = 0;
    for y in 20..=40 {
        for x in 20..=40 {
            if *img.get_pixel(x, y) != Rgba([255, 255, 255, 255]) {
                colored_count += 1;
            }
        }
    }
    assert!(
        colored_count >= 10,
        "Expected at least 10 sprayed particles, found {}",
        colored_count
    );
}

#[test]
fn test_calligraphy_anisotropic_stroke() {
    let mut img = RgbaImage::from_pixel(60, 60, Rgba([255, 255, 255, 255]));
    let black = Rgba([0, 0, 0, 255]);

    // Stroke along the angle (45 degrees)
    draw_calligraphy_stroke(&mut img, 10, 10, 25, 25, 8.0, 45.0, black);

    // Stroke perpendicular to the angle (135 degrees: from (40, 10) to (25, 25))
    let mut img2 = RgbaImage::from_pixel(60, 60, Rgba([255, 255, 255, 255]));
    draw_calligraphy_stroke(&mut img2, 40, 10, 25, 25, 8.0, 45.0, black);

    let count_pixels = |im: &RgbaImage| -> usize {
        im.pixels()
            .filter(|p| **p != Rgba([255, 255, 255, 255]))
            .count()
    };

    let parallel_count = count_pixels(&img);
    let perp_count = count_pixels(&img2);

    // Perpendicular stroke must cover significantly more area than parallel stroke
    assert!(
        perp_count > parallel_count,
        "Perpendicular stroke ({}) should be broader than parallel ({})",
        perp_count,
        parallel_count
    );
}

#[test]
fn test_eraser_restores_background() {
    let mut img = RgbaImage::from_pixel(30, 30, Rgba([10, 20, 30, 255]));
    let bg = Rgba([255, 255, 255, 255]);

    draw_eraser(&mut img, 10, 15, 20, 15, 3.0, bg);

    for x in 10..=20 {
        assert_eq!(*img.get_pixel(x, 15), bg);
    }
    assert_eq!(*img.get_pixel(5, 5), Rgba([10, 20, 30, 255]));
}

#[test]
fn test_flood_fill_enclosed_region() {
    let mut img = RgbaImage::from_pixel(20, 20, Rgba([255, 255, 255, 255]));
    let border = Rgba([0, 0, 0, 255]);
    let fill = Rgba([255, 0, 0, 255]);

    // Hollow rectangle 5..=15
    for x in 5..=15 {
        img.put_pixel(x, 5, border);
        img.put_pixel(x, 15, border);
    }
    for y in 5..=15 {
        img.put_pixel(5, y, border);
        img.put_pixel(15, y, border);
    }

    flood_fill(&mut img, 10, 10, fill, 0);

    // Inside must be filled
    assert_eq!(*img.get_pixel(10, 10), fill);
    assert_eq!(*img.get_pixel(6, 6), fill);
    assert_eq!(*img.get_pixel(14, 14), fill);

    // Border remains intact
    assert_eq!(*img.get_pixel(5, 10), border);
    assert_eq!(*img.get_pixel(15, 10), border);

    // Outside is unchanged
    assert_eq!(*img.get_pixel(2, 2), Rgba([255, 255, 255, 255]));
    assert_eq!(*img.get_pixel(18, 18), Rgba([255, 255, 255, 255]));
}

#[test]
fn test_sample_color_matrix() {
    let mut img = RgbaImage::from_pixel(5, 5, Rgba([10, 20, 30, 40]));
    img.put_pixel(2, 3, Rgba([200, 100, 50, 255]));

    assert_eq!(sample_color(&img, 2, 3), Some(Rgba([200, 100, 50, 255])));
    assert_eq!(sample_color(&img, 0, 0), Some(Rgba([10, 20, 30, 40])));
    assert_eq!(sample_color(&img, 4, 4), Some(Rgba([10, 20, 30, 40])));
    assert_eq!(sample_color(&img, 5, 0), None);
    assert_eq!(sample_color(&img, 0, 5), None);
    assert_eq!(sample_color(&img, 100, 100), None);
}

#[test]
fn test_mosaic_filter_consistency() {
    let mut img = RgbaImage::from_pixel(16, 16, Rgba([100, 100, 100, 255]));
    // Draw 4 distinct quadrants in a subregion (0..8, 0..8)
    for y in 0..4 {
        for x in 0..4 {
            img.put_pixel(x, y, Rgba([200, 0, 0, 255]));
        }
    }
    for y in 0..4 {
        for x in 4..8 {
            img.put_pixel(x, y, Rgba([0, 200, 0, 255]));
        }
    }

    // Apply 4x4 mosaic
    apply_mosaic_region(&mut img, 0, 0, 8, 8, 4);

    // Quadrant (0..4, 0..4) should be uniformly red
    for y in 0..4 {
        for x in 0..4 {
            assert_eq!(*img.get_pixel(x, y), Rgba([200, 0, 0, 255]));
        }
    }
    // Quadrant (4..8, 0..4) should be uniformly green
    for y in 0..4 {
        for x in 4..8 {
            assert_eq!(*img.get_pixel(x, y), Rgba([0, 200, 0, 255]));
        }
    }
    // Outside region is untouched
    assert_eq!(*img.get_pixel(12, 12), Rgba([100, 100, 100, 255]));
}

#[test]
fn test_canvas_resize_integration() {
    let mut original = RgbaImage::from_pixel(20, 20, Rgba([10, 20, 30, 255]));
    original.put_pixel(5, 5, Rgba([255, 255, 0, 255]));
    let bg = Rgba([240, 240, 240, 255]);

    // 1. Expand
    let expanded = canvas_resize(&original, 40, 30, bg);
    assert_eq!(expanded.width(), 40);
    assert_eq!(expanded.height(), 30);
    assert_eq!(*expanded.get_pixel(5, 5), Rgba([255, 255, 0, 255]));
    assert_eq!(*expanded.get_pixel(19, 19), Rgba([10, 20, 30, 255]));
    assert_eq!(*expanded.get_pixel(25, 25), bg);
    assert_eq!(*expanded.get_pixel(39, 29), bg);

    // 2. Shrink
    let shrunk = canvas_resize(&expanded, 10, 10, bg);
    assert_eq!(shrunk.width(), 10);
    assert_eq!(shrunk.height(), 10);
    assert_eq!(*shrunk.get_pixel(5, 5), Rgba([255, 255, 0, 255]));
    assert_eq!(*shrunk.get_pixel(9, 9), Rgba([10, 20, 30, 255]));
}

#[test]
fn test_raster_shapes_draw() {
    let mut img = RgbaImage::from_pixel(40, 40, Rgba([0, 0, 0, 255]));
    let color = Rgba([255, 255, 255, 255]);

    // Hollow rectangle
    draw_rectangle(&mut img, 5, 5, 15, 15, color, 1, false);
    assert_eq!(*img.get_pixel(5, 5), color);
    assert_eq!(*img.get_pixel(10, 10), Rgba([0, 0, 0, 255]));

    // Filled circle
    draw_circle(&mut img, 25, 25, 8, color, 1, true);
    assert_eq!(*img.get_pixel(25, 25), color);
    assert_eq!(*img.get_pixel(25, 30), color);

    // Filled ellipse
    draw_ellipse(&mut img, 20, 10, 6, 3, color, 1, true);
    assert_eq!(*img.get_pixel(20, 10), color);
    assert_eq!(*img.get_pixel(24, 10), color);
}
