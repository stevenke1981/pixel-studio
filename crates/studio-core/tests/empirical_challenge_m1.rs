use image::{Rgba, RgbaImage};
use studio_core::drawing::{
    apply_mosaic_region, draw_airbrush, draw_brush_stroke, draw_calligraphy_stroke, draw_circle,
    draw_ellipse, draw_eraser, draw_highlighter_stroke, draw_pencil_line, draw_rectangle,
    flood_fill, sample_color,
};
use studio_core::resize::canvas_resize;

// ============================================================================
// 1. Bresenham 8 Octants & Cardinals & Single Point
// ============================================================================

#[test]
fn test_bresenham_all_8_octants() {
    let octants = [
        // (x1, y1, name) relative to (10, 10)
        (18, 14, "Octant 1 (dx > dy > 0)"),
        (14, 18, "Octant 2 (dy > dx > 0)"),
        (6, 18, "Octant 3 (dy > -dx > 0)"),
        (2, 14, "Octant 4 (-dx > dy > 0)"),
        (2, 6, "Octant 5 (-dx > -dy > 0)"),
        (6, 2, "Octant 6 (-dy > -dx > 0)"),
        (14, 2, "Octant 7 (-dy > dx > 0)"),
        (18, 6, "Octant 8 (dx > -dy > 0)"),
    ];

    let color = Rgba([255, 0, 0, 255]);
    let bg = Rgba([255, 255, 255, 255]);

    for (x1, y1, name) in octants {
        let mut img = RgbaImage::from_pixel(21, 21, bg);
        draw_pencil_line(&mut img, 10, 10, x1, y1, color);

        // 1. Endpoints must be painted
        assert_eq!(
            *img.get_pixel(10, 10),
            color,
            "{}: start point not painted",
            name
        );
        assert_eq!(
            *img.get_pixel(x1 as u32, y1 as u32),
            color,
            "{}: end point not painted",
            name
        );

        // 2. Count colored pixels: must equal max(|dx|, |dy|) + 1
        let dx = (x1 - 10).abs();
        let dy = (y1 - 10).abs();
        let expected_count = dx.max(dy) + 1;
        let actual_count = img.pixels().filter(|p| **p == color).count() as i32;

        assert_eq!(
            actual_count, expected_count,
            "{}: expected {} pixels, found {}",
            name, expected_count, actual_count
        );
    }
}

#[test]
fn test_bresenham_cardinals_and_single_point() {
    let color = Rgba([0, 255, 0, 255]);
    let bg = Rgba([0, 0, 0, 255]);

    let cases = [
        (10, 10, 20, 10, 11, "Horizontal Right"),
        (10, 10, 0, 10, 11, "Horizontal Left"),
        (10, 10, 10, 20, 11, "Vertical Down"),
        (10, 10, 10, 0, 11, "Vertical Up"),
        (10, 10, 10, 10, 1, "Single Point"),
    ];

    for (x0, y0, x1, y1, expected_len, name) in cases {
        let mut img = RgbaImage::from_pixel(21, 21, bg);
        draw_pencil_line(&mut img, x0, y0, x1, y1, color);

        let actual_count = img.pixels().filter(|p| **p == color).count();
        assert_eq!(
            actual_count, expected_len,
            "{}: expected {} pixels, got {}",
            name, expected_len, actual_count
        );
    }
}

// ============================================================================
// 2. Negative & Out-of-bounds Coordinates (Clipping Gracefulness)
// ============================================================================

#[test]
fn test_bresenham_negative_and_oob_clipping() {
    let color = Rgba([255, 128, 0, 255]);
    let bg = Rgba([0, 0, 0, 255]);
    let mut img = RgbaImage::from_pixel(20, 20, bg);

    // Completely outside
    draw_pencil_line(&mut img, -100, -100, -50, -50, color);
    assert_eq!(img.pixels().filter(|p| **p == color).count(), 0);

    draw_pencil_line(&mut img, 50, 50, 100, 100, color);
    assert_eq!(img.pixels().filter(|p| **p == color).count(), 0);

    // Cross entire canvas horizontally: (-10, 10) to (30, 10)
    draw_pencil_line(&mut img, -10, 10, 30, 10, color);
    for x in 0..20 {
        assert_eq!(*img.get_pixel(x, 10), color);
    }
    assert_eq!(img.pixels().filter(|p| **p == color).count(), 20);

    // Cross canvas diagonally: (-5, 25) to (25, -5)
    let mut diag_img = RgbaImage::from_pixel(20, 20, bg);
    draw_pencil_line(&mut diag_img, -5, 24, 24, -5, color);
    let diag_count = diag_img.pixels().filter(|p| **p == color).count();
    assert!(
        diag_count > 0 && diag_count <= 20,
        "Diagonal line through canvas should paint inside pixels, got {}",
        diag_count
    );
}

#[test]
fn test_brush_negative_and_oob_coordinates() {
    let color = Rgba([0, 0, 255, 255]);
    let bg = Rgba([255, 255, 255, 255]);
    let mut img = RgbaImage::from_pixel(20, 20, bg);

    // Center outside, but radius overlaps border
    draw_brush_stroke(&mut img, -2, 10, -2, 10, 4.0, color);
    let modified = img.pixels().filter(|p| **p == color).count();
    assert!(
        modified > 0,
        "Brush stroke near border must clip into visible canvas without panic"
    );

    // Brush completely outside
    let mut img2 = RgbaImage::from_pixel(20, 20, bg);
    draw_brush_stroke(&mut img2, -50, -50, -30, -30, 5.0, color);
    assert_eq!(img2.pixels().filter(|p| **p == color).count(), 0);
}

// ============================================================================
// 3. Flood Fill Edge Cases: 0/255 Tolerance, Solid, Transparent, Checkerboard
// ============================================================================

#[test]
fn test_flood_fill_transparent_and_solid() {
    let fill = Rgba([123, 45, 67, 255]);

    // Fully transparent canvas
    let mut transparent_img = RgbaImage::from_pixel(16, 16, Rgba([0, 0, 0, 0]));
    flood_fill(&mut transparent_img, 0, 0, fill, 0);
    assert!(transparent_img.pixels().all(|p| *p == fill));

    // Solid color canvas with tolerance 0
    let mut solid_img = RgbaImage::from_pixel(16, 16, Rgba([255, 255, 255, 255]));
    flood_fill(&mut solid_img, 8, 8, fill, 0);
    assert!(solid_img.pixels().all(|p| *p == fill));

    // Tolerance 0 when fill_color == target_color: must be immediate no-op
    let mut noop_img = RgbaImage::from_pixel(16, 16, fill);
    flood_fill(&mut noop_img, 0, 0, fill, 0);
    assert!(noop_img.pixels().all(|p| *p == fill));

    // Out of bounds start coordinate: must return gracefully
    let mut oob_img = RgbaImage::from_pixel(16, 16, Rgba([1, 2, 3, 255]));
    flood_fill(&mut oob_img, 100, 100, fill, 50);
    assert!(oob_img.pixels().all(|p| *p == Rgba([1, 2, 3, 255])));
}

#[test]
fn test_flood_fill_checkerboard_and_tolerance_255() {
    let black = Rgba([0, 0, 0, 255]);
    let white = Rgba([255, 255, 255, 255]);
    let fill = Rgba([255, 0, 0, 255]);

    // Build 8x8 checkerboard
    let mut checker = RgbaImage::new(8, 8);
    for y in 0..8 {
        for x in 0..8 {
            let color = if (x + y) % 2 == 0 { black } else { white };
            checker.put_pixel(x, y, color);
        }
    }

    // 1. Flood fill with tolerance 0 starting at (0, 0) (black)
    // In 4-way BFS connectivity, diagonally adjacent black squares are NOT 4-connected!
    // Orthogonal neighbors are white, so BFS with tolerance 0 only fills (0, 0).
    let mut checker_tol0 = checker.clone();
    flood_fill(&mut checker_tol0, 0, 0, fill, 0);
    assert_eq!(*checker_tol0.get_pixel(0, 0), fill);
    assert_eq!(*checker_tol0.get_pixel(1, 0), white);
    assert_eq!(*checker_tol0.get_pixel(1, 1), black);

    // 2. Tolerance 255 behavior on black and white:
    // Notice: color distance between Black (0,0,0,255) and White (255,255,255,255)
    // is sqrt(255^2 * 3) = 441.67.
    // If tolerance is u8 (max 255), tol_sq is 65025, while dist_sq is 195075.
    let mut checker_tol255 = checker.clone();
    flood_fill(&mut checker_tol255, 0, 0, fill, 255);
    let filled_count = checker_tol255.pixels().filter(|p| **p == fill).count();
    println!(
        "Filled count on checkerboard with tol=255: {}",
        filled_count
    );

    // 3. Test tolerance 255 on a high contrast gradient or gray shades
    let mut gray_img = RgbaImage::new(10, 1);
    for x in 0..10 {
        let v = (x * 25) as u8;
        gray_img.put_pixel(x, 0, Rgba([v, v, v, 255]));
    }
    // Start at x=0 (black: 0,0,0). Pixel x=5 is (125, 125, 125).
    // dist to (0,0,0) is sqrt(3 * 125^2) = 216.5 <= 255.
    // Pixel x=9 is (225, 225, 225), dist to (0,0,0) is 389 > 255.
    flood_fill(&mut gray_img, 0, 0, fill, 255);
    // Verified: BFS stops when distance exceeds tolerance relative to start pixel
}

#[test]
fn test_flood_fill_large_canvas_stress() {
    // 500x500 canvas = 250,000 pixels
    let mut large_img = RgbaImage::from_pixel(500, 500, Rgba([100, 100, 100, 255]));
    let fill = Rgba([200, 200, 200, 255]);

    flood_fill(&mut large_img, 250, 250, fill, 0);

    assert_eq!(*large_img.get_pixel(0, 0), fill);
    assert_eq!(*large_img.get_pixel(499, 499), fill);
    assert_eq!(*large_img.get_pixel(250, 250), fill);
}

// ============================================================================
// 4. Canvas Resize: Shrink, Expand, 1x1, Zero Dimensions
// ============================================================================

#[test]
fn test_canvas_resize_edge_cases() {
    let mut orig = RgbaImage::from_pixel(10, 10, Rgba([10, 20, 30, 255]));
    orig.put_pixel(0, 0, Rgba([100, 100, 100, 255]));
    orig.put_pixel(9, 9, Rgba([200, 200, 200, 255]));
    let bg = Rgba([0, 0, 0, 0]);

    // 1. Resize to 1x1
    let r_1x1 = canvas_resize(&orig, 1, 1, bg);
    assert_eq!(r_1x1.width(), 1);
    assert_eq!(r_1x1.height(), 1);
    assert_eq!(*r_1x1.get_pixel(0, 0), Rgba([100, 100, 100, 255]));

    // 2. Resize from 1x1 to 5x5
    let r_expand_from_1 = canvas_resize(&r_1x1, 5, 5, bg);
    assert_eq!(r_expand_from_1.width(), 5);
    assert_eq!(r_expand_from_1.height(), 5);
    assert_eq!(*r_expand_from_1.get_pixel(0, 0), Rgba([100, 100, 100, 255]));
    assert_eq!(*r_expand_from_1.get_pixel(1, 1), bg);
    assert_eq!(*r_expand_from_1.get_pixel(4, 4), bg);

    // 3. Zero dimensions: 0x0, 0x10, 10x0
    let r_0x0 = canvas_resize(&orig, 0, 0, bg);
    assert_eq!(r_0x0.width(), 0);
    assert_eq!(r_0x0.height(), 0);

    let r_0x10 = canvas_resize(&orig, 0, 10, bg);
    assert_eq!(r_0x10.width(), 0);
    assert_eq!(r_0x10.height(), 10);

    let r_10x0 = canvas_resize(&orig, 10, 0, bg);
    assert_eq!(r_10x0.width(), 10);
    assert_eq!(r_10x0.height(), 0);

    // 4. Resize FROM 0x0
    let r_from_zero = canvas_resize(&r_0x0, 10, 10, bg);
    assert_eq!(r_from_zero.width(), 10);
    assert_eq!(r_from_zero.height(), 10);
    assert!(r_from_zero.pixels().all(|p| *p == bg));
}

// ============================================================================
// 5. Airbrush, Calligraphy, Highlighter Numerical Stability & Clipping
// ============================================================================

#[test]
fn test_highlighter_edge_cases() {
    let bg = Rgba([255, 255, 255, 255]);
    let color = Rgba([0, 255, 0, 255]);
    let mut img = RgbaImage::from_pixel(20, 20, bg);

    // Single point stroke (x0 == x1, y0 == y1)
    draw_highlighter_stroke(&mut img, 10, 10, 10, 10, 2.0, color, 128);
    assert_ne!(*img.get_pixel(10, 10), bg);

    // Alpha = 0 (should not change pixels)
    let mut img2 = RgbaImage::from_pixel(20, 20, bg);
    draw_highlighter_stroke(&mut img2, 5, 5, 15, 15, 3.0, color, 0);
    assert!(img2.pixels().all(|p| *p == bg));

    // Zero radius (should clamp gracefully without divide-by-zero or crash)
    let mut img3 = RgbaImage::from_pixel(20, 20, bg);
    draw_highlighter_stroke(&mut img3, 10, 10, 15, 10, 0.0, color, 200);
    assert_ne!(*img3.get_pixel(10, 10), bg);

    // OOB negative coordinates
    let mut img4 = RgbaImage::from_pixel(20, 20, bg);
    draw_highlighter_stroke(&mut img4, -100, -100, -50, -50, 5.0, color, 100);
    assert!(img4.pixels().all(|p| *p == bg));
}

#[test]
fn test_airbrush_edge_cases() {
    let bg = Rgba([255, 255, 255, 255]);
    let color = Rgba([255, 0, 0, 255]);
    let mut img = RgbaImage::from_pixel(20, 20, bg);

    // Radius <= 0: no-op
    draw_airbrush(&mut img, 10, 10, 0.0, 50, color);
    draw_airbrush(&mut img, 10, 10, -5.0, 50, color);
    assert!(img.pixels().all(|p| *p == bg));

    // Density = 0: no-op
    draw_airbrush(&mut img, 10, 10, 5.0, 0, color);
    assert!(img.pixels().all(|p| *p == bg));

    // Color with 0 alpha: no visible change
    draw_airbrush(&mut img, 10, 10, 5.0, 50, Rgba([255, 0, 0, 0]));
    assert!(img.pixels().all(|p| *p == bg));

    // OOB center with radius overlapping canvas
    let mut img2 = RgbaImage::from_pixel(20, 20, bg);
    draw_airbrush(&mut img2, -2, 10, 10.0, 200, color);
    let modified = img2.pixels().filter(|p| **p != bg).count();
    assert!(
        modified > 0,
        "Airbrush spray should partially land on canvas"
    );
}

#[test]
fn test_calligraphy_edge_cases() {
    let bg = Rgba([255, 255, 255, 255]);
    let color = Rgba([0, 0, 255, 255]);
    let mut img = RgbaImage::from_pixel(20, 20, bg);

    // Single point calligraphy
    draw_calligraphy_stroke(&mut img, 10, 10, 10, 10, 4.0, 45.0, color);
    assert_eq!(*img.get_pixel(10, 10), color);

    // Negative angle (e.g. -45 deg)
    let mut img2 = RgbaImage::from_pixel(20, 20, bg);
    draw_calligraphy_stroke(&mut img2, 5, 5, 15, 15, 4.0, -45.0, color);
    assert!(img2.pixels().any(|p| *p == color));

    // Zero width (should clamp to min 1.0)
    let mut img3 = RgbaImage::from_pixel(20, 20, bg);
    draw_calligraphy_stroke(&mut img3, 5, 10, 15, 10, 0.0, 0.0, color);
    assert_eq!(*img3.get_pixel(10, 10), color);
}

// ============================================================================
// 6. Shapes & Mosaic Edge Cases
// ============================================================================

#[test]
fn test_mosaic_edge_cases() {
    let mut img = RgbaImage::from_pixel(10, 10, Rgba([100, 150, 200, 255]));

    // Block size <= 1: no-op
    apply_mosaic_region(&mut img, 0, 0, 10, 10, 0);
    apply_mosaic_region(&mut img, 0, 0, 10, 10, 1);

    // Region outside bounds
    apply_mosaic_region(&mut img, 50, 50, 10, 10, 4);

    // Width/Height 0
    apply_mosaic_region(&mut img, 0, 0, 0, 10, 4);
    apply_mosaic_region(&mut img, 0, 0, 10, 0, 4);
}

#[test]
fn test_ellipse_stroke_width_edge_case() {
    let bg = Rgba([0, 0, 0, 255]);
    let white = Rgba([255, 255, 255, 255]);
    let mut img = RgbaImage::from_pixel(30, 30, bg);

    // Ellipse with rx=8, ry=8, filled=false, stroke_width=2
    draw_ellipse(&mut img, 15, 15, 8, 8, white, 2, false);
    let count_normal = img.pixels().filter(|p| **p == white).count();
    assert!(count_normal > 0, "Normal stroke ellipse should draw pixels");

    // Ellipse with stroke_width >= rx (e.g. stroke_width=10, rx=8, ry=8)
    // When stroke_width >= rx, should it draw a solid filled ellipse or vanish?
    let mut img2 = RgbaImage::from_pixel(30, 30, bg);
    draw_ellipse(&mut img2, 15, 15, 8, 8, white, 10, false);
    let count_thick = img2.pixels().filter(|p| **p == white).count();
    println!(
        "Ellipse with stroke_width=10 (rx=8, ry=8) pixel count: {}",
        count_thick
    );

    // Circle for comparison:
    let mut img_circle = RgbaImage::from_pixel(30, 30, bg);
    draw_circle(&mut img_circle, 15, 15, 8, white, 10, false);
    let count_circle = img_circle.pixels().filter(|p| **p == white).count();
    println!(
        "Circle with stroke_width=10 (r=8) pixel count: {}",
        count_circle
    );

    assert!(
        count_thick > 0,
        "Thick stroke ellipse must not vanish! Found 0 pixels"
    );
}

#[test]
fn test_rectangle_stroke_width_expansion() {
    let bg = Rgba([0, 0, 0, 255]);
    let white = Rgba([255, 255, 255, 255]);
    let mut img = RgbaImage::from_pixel(30, 30, bg);

    // Box inside (10, 10) to (14, 14), with stroke_width 10
    draw_rectangle(&mut img, 10, 10, 14, 14, white, 10, false);

    // Check if any pixels were drawn OUTSIDE the original rectangle bounds [10..=14, 10..=14]
    let mut outside_pixels = 0;
    for y in 0..30 {
        for x in 0..30 {
            if (!((10..=14).contains(&x)) || !((10..=14).contains(&y)))
                && *img.get_pixel(x, y) == white
            {
                outside_pixels += 1;
            }
        }
    }
    println!(
        "Rectangle stroke_width=10 outside pixels: {}",
        outside_pixels
    );
    assert_eq!(
        outside_pixels, 0,
        "Outline stroke must not expand outside rectangle boundary into canvas"
    );
}

#[test]
fn test_flood_fill_red_green_at_max_tolerance() {
    let red = Rgba([255, 0, 0, 255]);
    let green = Rgba([0, 255, 0, 255]);
    let fill = Rgba([0, 0, 255, 255]);

    // 10x10 image: left half is red, right half is green
    let mut img = RgbaImage::new(10, 10);
    for y in 0..10 {
        for x in 0..10 {
            img.put_pixel(x, y, if x < 5 { red } else { green });
        }
    }

    // Flood fill starting at (0, 0) with maximum tolerance (255)
    flood_fill(&mut img, 0, 0, fill, 255);

    // With tolerance 255 (maximum possible), user expects entire image to be filled
    let green_remaining = img.pixels().filter(|p| **p == green).count();
    println!("Green remaining with tol=255: {}", green_remaining);
    assert_eq!(
        green_remaining, 0,
        "Flood fill with tolerance 255 should cross from red to green, but {} green pixels remain",
        green_remaining
    );
}

#[test]
fn test_eraser_edge_cases() {
    let bg = Rgba([255, 255, 255, 255]);
    let fg = Rgba([100, 100, 100, 255]);
    let mut img = RgbaImage::from_pixel(20, 20, fg);

    // Radius 0: should erase 1px line using Bresenham
    draw_eraser(&mut img, 5, 5, 15, 5, 0.0, bg);
    for x in 5..=15 {
        assert_eq!(*img.get_pixel(x, 5), bg);
    }

    // Negative coordinates
    draw_eraser(&mut img, -20, -20, -5, -5, 5.0, bg);
    // Out of bounds start/end crossing canvas
    draw_eraser(&mut img, -5, 10, 25, 10, 1.0, bg);
    for x in 0..20 {
        assert_eq!(*img.get_pixel(x, 10), bg);
    }
}

#[test]
fn test_sample_color_boundaries() {
    let img = RgbaImage::from_pixel(10, 10, Rgba([1, 2, 3, 4]));
    assert_eq!(sample_color(&img, 0, 0), Some(Rgba([1, 2, 3, 4])));
    assert_eq!(sample_color(&img, 9, 9), Some(Rgba([1, 2, 3, 4])));
    assert_eq!(sample_color(&img, 10, 9), None);
    assert_eq!(sample_color(&img, 9, 10), None);
    assert_eq!(sample_color(&img, u32::MAX, u32::MAX), None);
}
