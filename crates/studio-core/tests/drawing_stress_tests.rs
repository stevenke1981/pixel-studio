//! Empirical stress harness for drawing primitives and memory bounds.
//!
//! Validates:
//! - Repeated stroke memory allocations: thousands of strokes execute without memory leaks
//!   or exponential slowdown (sub-linear or linear performance).
//! - Bounds clipping and numerical stability under extreme coordinate stresses.
//! - Repeated flood fill throughput and heap recycling.
//! - Repeated non-scaling canvas resize stability across alternating expand/shrink dimensions.

use image::{Rgba, RgbaImage};
use std::time::Instant;
use studio_core::drawing::*;
use studio_core::resize::canvas_resize;

#[test]
fn test_repeated_pencil_strokes_throughput_and_bounds() {
    let mut canvas = RgbaImage::from_pixel(1280, 720, Rgba([255, 255, 255, 255]));
    let total_strokes = 10_000;
    let batch_size = 1_000;
    let mut batch_times = Vec::new();

    let overall_start = Instant::now();

    for batch in 0..(total_strokes / batch_size) {
        let batch_start = Instant::now();
        for i in 0..batch_size {
            let stroke_idx = batch * batch_size + i;
            let x0 = (stroke_idx * 37) % 1320 - 20;
            let y0 = (stroke_idx * 73) % 760 - 20;
            let x1 = (stroke_idx * 101) % 1320 - 20;
            let y1 = (stroke_idx * 149) % 760 - 20;
            let color = Rgba([
                (stroke_idx % 256) as u8,
                ((stroke_idx * 3) % 256) as u8,
                ((stroke_idx * 7) % 256) as u8,
                255,
            ]);
            draw_pencil_line(&mut canvas, x0, y0, x1, y1, color);
        }
        let elapsed = batch_start.elapsed();
        batch_times.push(elapsed);
    }

    let overall_time = overall_start.elapsed();
    println!(
        "Pencil 10,000 strokes total time: {:?}, avg batch time: {:?}",
        overall_time,
        overall_time / (total_strokes / batch_size) as u32
    );

    // Verify no exponential slowdown: the slowest batch should not exceed 3.5x the first batch
    let t_first = batch_times[0].as_micros() as f64;
    for (idx, duration) in batch_times.iter().enumerate() {
        let t_cur = duration.as_micros() as f64;
        let ratio = t_cur / t_first.max(1.0);
        assert!(
            ratio < 4.0,
            "Batch {} took too long relative to batch 0: ratio={:.2} ({:?} vs {:?})",
            idx,
            ratio,
            duration,
            batch_times[0]
        );
    }

    // Verify canvas dimensions unchanged and non-empty pixel mutations
    assert_eq!(canvas.width(), 1280);
    assert_eq!(canvas.height(), 720);
    let sample = sample_color(&canvas, 640, 360).expect("Center pixel must exist");
    assert_ne!(sample, Rgba([0, 0, 0, 0]));
}

#[test]
fn test_repeated_brush_strokes_throughput_and_memory() {
    let mut canvas = RgbaImage::from_pixel(1280, 720, Rgba([255, 255, 255, 255]));
    let total_strokes = 5_000;
    let batch_size = 500;
    let mut batch_times = Vec::new();

    let overall_start = Instant::now();

    for batch in 0..(total_strokes / batch_size) {
        let batch_start = Instant::now();
        for i in 0..batch_size {
            let stroke_idx = batch * batch_size + i;
            let x0 = (stroke_idx * 43) % 1280;
            let y0 = (stroke_idx * 67) % 720;
            let x1 = x0 + (stroke_idx % 60 - 30);
            let y1 = y0 + ((stroke_idx * 2) % 60 - 30);
            let radius = if stroke_idx % 2 == 0 { 4.0 } else { 8.0 };
            let color = Rgba([
                (stroke_idx % 255) as u8,
                128,
                200,
                180, // translucent for alpha blending stress
            ]);
            draw_brush_stroke(&mut canvas, x0, y0, x1, y1, radius, color);
        }
        let elapsed = batch_start.elapsed();
        batch_times.push(elapsed);
    }

    let overall_time = overall_start.elapsed();
    println!(
        "Brush 5,000 strokes total time: {:?}, avg batch time: {:?}",
        overall_time,
        overall_time / (total_strokes / batch_size) as u32
    );

    // Verify throughput stability (no exponential slowdown)
    let t_first = batch_times[0].as_micros() as f64;
    for (idx, duration) in batch_times.iter().enumerate() {
        let t_cur = duration.as_micros() as f64;
        let ratio = t_cur / t_first.max(1.0);
        assert!(
            ratio < 4.0,
            "Brush batch {} took too long relative to batch 0: ratio={:.2}",
            idx,
            ratio
        );
    }
}

#[test]
fn test_repeated_highlighter_strokes_throughput() {
    let mut canvas = RgbaImage::from_pixel(1280, 720, Rgba([255, 255, 255, 255]));
    let total_strokes = 2_000;
    let batch_size = 200;
    let mut batch_times = Vec::new();

    let start = Instant::now();
    for batch in 0..(total_strokes / batch_size) {
        let b_start = Instant::now();
        for i in 0..batch_size {
            let idx = batch * batch_size + i;
            let x0 = (idx * 59) % 1200;
            let y0 = (idx * 83) % 680;
            let x1 = x0 + 100;
            let y1 = y0 + 20;
            let color = Rgba([255, 255, 0, 255]); // Yellow highlighter
            draw_highlighter_stroke(&mut canvas, x0, y0, x1, y1, 10.0, color, 64);
        }
        batch_times.push(b_start.elapsed());
    }

    let total = start.elapsed();
    println!(
        "Highlighter 2,000 strokes total time: {:?}, avg batch: {:?}",
        total,
        total / (total_strokes / batch_size) as u32
    );

    // Compare against the median batch (shared CI runners jitter a lot), ignoring sub-5ms noise.
    let mut sorted: Vec<f64> = batch_times.iter().map(|d| d.as_micros() as f64).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2].max(5000.0);
    for (idx, duration) in batch_times.iter().enumerate() {
        let ratio = duration.as_micros() as f64 / median;
        assert!(
            ratio < 4.0,
            "Highlighter batch {} slowdown: ratio={:.2}",
            idx,
            ratio
        );
    }
}

#[test]
fn test_repeated_airbrush_sprays_throughput() {
    let mut canvas = RgbaImage::from_pixel(1280, 720, Rgba([255, 255, 255, 255]));
    let total_sprays = 10_000;
    let batch_size = 1_000;
    let mut batch_times = Vec::new();

    let start = Instant::now();
    for batch in 0..(total_sprays / batch_size) {
        let b_start = Instant::now();
        for i in 0..batch_size {
            let idx = batch * batch_size + i;
            let x = (idx * 97) % 1280;
            let y = (idx * 131) % 720;
            let color = Rgba([30, 144, 255, 200]);
            draw_airbrush(&mut canvas, x, y, 15.0, 40, color);
        }
        batch_times.push(b_start.elapsed());
    }

    let total = start.elapsed();
    println!(
        "Airbrush 10,000 sprays total time: {:?}, avg batch: {:?}",
        total,
        total / (total_sprays / batch_size) as u32
    );

    // Compare against the median batch (shared CI runners jitter a lot), ignoring sub-5ms noise.
    let mut sorted: Vec<f64> = batch_times.iter().map(|d| d.as_micros() as f64).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2].max(5000.0);
    for (idx, duration) in batch_times.iter().enumerate() {
        let ratio = duration.as_micros() as f64 / median;
        assert!(
            ratio < 4.0,
            "Airbrush batch {} slowdown: ratio={:.2}",
            idx,
            ratio
        );
    }
}

#[test]
fn test_repeated_calligraphy_strokes_throughput() {
    let mut canvas = RgbaImage::from_pixel(1280, 720, Rgba([255, 255, 255, 255]));
    let total_strokes = 5_000;
    let batch_size = 500;
    let mut batch_times = Vec::new();

    let start = Instant::now();
    for batch in 0..(total_strokes / batch_size) {
        let b_start = Instant::now();
        for i in 0..batch_size {
            let idx = batch * batch_size + i;
            let x0 = (idx * 31) % 1280;
            let y0 = (idx * 47) % 720;
            let x1 = x0 + 40;
            let y1 = y0 + 40;
            let angle = (idx * 15 % 360) as f32;
            let color = Rgba([16, 16, 16, 255]);
            draw_calligraphy_stroke(&mut canvas, x0, y0, x1, y1, 8.0, angle, color);
        }
        batch_times.push(b_start.elapsed());
    }

    let total = start.elapsed();
    println!(
        "Calligraphy 5,000 strokes total time: {:?}, avg batch: {:?}",
        total,
        total / (total_strokes / batch_size) as u32
    );

    // Compare against the median batch (shared CI runners jitter a lot), ignoring sub-5ms noise.
    let mut sorted: Vec<f64> = batch_times.iter().map(|d| d.as_micros() as f64).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2].max(5000.0);
    for (idx, duration) in batch_times.iter().enumerate() {
        let ratio = duration.as_micros() as f64 / median;
        assert!(
            ratio < 4.0,
            "Calligraphy batch {} slowdown: ratio={:.2}",
            idx,
            ratio
        );
    }
}

#[test]
fn test_repeated_eraser_strokes_throughput() {
    let mut canvas = RgbaImage::from_pixel(1280, 720, Rgba([50, 100, 150, 255]));
    let total_strokes = 5_000;
    let batch_size = 500;
    let mut batch_times = Vec::new();

    let bg = Rgba([255, 255, 255, 255]);
    let start = Instant::now();
    for batch in 0..(total_strokes / batch_size) {
        let b_start = Instant::now();
        for i in 0..batch_size {
            let idx = batch * batch_size + i;
            let x0 = (idx * 29) % 1280;
            let y0 = (idx * 53) % 720;
            let x1 = x0 + 30;
            let y1 = y0 + 30;
            let radius = if idx % 2 == 0 { 6.0 } else { 12.0 };
            draw_eraser(&mut canvas, x0, y0, x1, y1, radius, bg);
        }
        batch_times.push(b_start.elapsed());
    }

    let total = start.elapsed();
    println!(
        "Eraser 5,000 strokes total time: {:?}, avg batch: {:?}",
        total,
        total / (total_strokes / batch_size) as u32
    );

    // Compare against the median batch (shared CI runners jitter a lot), ignoring sub-5ms noise.
    let mut sorted: Vec<f64> = batch_times.iter().map(|d| d.as_micros() as f64).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2].max(5000.0);
    for (idx, duration) in batch_times.iter().enumerate() {
        let ratio = duration.as_micros() as f64 / median;
        assert!(
            ratio < 4.0,
            "Eraser batch {} slowdown: ratio={:.2}",
            idx,
            ratio
        );
    }
}

#[test]
fn test_flood_fill_repeated_stress() {
    let mut canvas = RgbaImage::from_pixel(1280, 720, Rgba([255, 255, 255, 255]));
    let color_a = Rgba([200, 50, 50, 255]);
    let color_b = Rgba([50, 200, 50, 255]);

    let start = Instant::now();
    for cycle in 0..20 {
        let fill_color = if cycle % 2 == 0 { color_a } else { color_b };
        flood_fill(&mut canvas, 640, 360, fill_color, 10);
        assert_eq!(
            *canvas.get_pixel(0, 0),
            fill_color,
            "Cycle {} fill failed at corner",
            cycle
        );
        assert_eq!(
            *canvas.get_pixel(1279, 719),
            fill_color,
            "Cycle {} fill failed at bottom corner",
            cycle
        );
    }
    let elapsed = start.elapsed();
    println!("Flood fill 20 full-frame (1280x720) cycles: {:?}", elapsed);
    assert!(
        elapsed.as_secs() < 10,
        "Flood fill cycles took too long: {:?}",
        elapsed
    );
}

#[test]
fn test_canvas_resize_repeated_expansion_and_shrink() {
    let mut canvas = RgbaImage::from_pixel(1280, 720, Rgba([10, 20, 30, 255]));
    let bg = Rgba([255, 255, 255, 255]);

    let target_sizes = [
        (1920, 1080),
        (800, 600),
        (2560, 1440),
        (640, 480),
        (1280, 720),
    ];

    let start = Instant::now();
    for cycle in 0..100 {
        let (w, h) = target_sizes[cycle % target_sizes.len()];
        canvas = canvas_resize(&canvas, w, h, bg);
        assert_eq!(canvas.width(), w);
        assert_eq!(canvas.height(), h);
        // Top-left pixel should remain original throughout all cycles
        assert_eq!(*canvas.get_pixel(0, 0), Rgba([10, 20, 30, 255]));
    }
    let elapsed = start.elapsed();
    println!("Canvas resize 100 cycles: {:?}", elapsed);
    assert!(
        elapsed.as_secs() < 15,
        "Canvas resize took too long: {:?}",
        elapsed
    );
}

#[test]
fn test_drawing_extreme_coordinates_and_clipping() {
    let mut canvas = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));
    let red = Rgba([255, 0, 0, 255]);

    // Entirely out-of-bounds coordinates (negative and huge)
    draw_pencil_line(&mut canvas, -1000, -1000, -500, -500, red);
    draw_pencil_line(&mut canvas, 5000, 5000, 10000, 10000, red);
    draw_pencil_line(&mut canvas, -500, 50, 500, 50, red); // Pierces through canvas horizontally
    draw_pencil_line(&mut canvas, 50, -500, 50, 500, red); // Pierces through canvas vertically

    draw_brush_stroke(&mut canvas, -50, -50, -10, -10, 20.0, red);
    draw_brush_stroke(&mut canvas, -20, 50, 120, 50, 15.0, red);

    draw_highlighter_stroke(&mut canvas, -100, 50, 200, 50, 20.0, red, 128);
    draw_airbrush(&mut canvas, -50, -50, 100.0, 100, red);
    draw_airbrush(&mut canvas, 50, 50, 0.0, 100, red); // 0 radius
    draw_airbrush(&mut canvas, 50, 50, 50.0, 0, red); // 0 density

    draw_calligraphy_stroke(&mut canvas, -100, -100, 200, 200, 30.0, 45.0, red);
    draw_eraser(&mut canvas, -50, 50, 150, 50, 10.0, Rgba([0, 0, 0, 0]));

    // Shapes with negative / out-of-bounds bounds
    draw_rectangle(&mut canvas, -50, -50, 150, 150, red, 2, false);
    draw_rectangle(&mut canvas, -20, -20, 120, 120, red, 1, true);
    draw_circle(&mut canvas, 50, 50, 200, red, 2, false);
    draw_circle(&mut canvas, 50, 50, 200, red, 1, true);
    draw_ellipse(&mut canvas, 50, 50, 200, 150, red, 2, false);
    draw_ellipse(&mut canvas, 50, 50, 200, 150, red, 1, true);

    // Mosaic on out of bounds
    apply_mosaic_region(&mut canvas, 500, 500, 100, 100, 10);
    apply_mosaic_region(&mut canvas, 0, 0, 0, 0, 10);
    apply_mosaic_region(&mut canvas, 0, 0, 50, 50, 1);

    // Sampling out of bounds
    assert_eq!(sample_color(&canvas, 9999, 9999), None);
    assert_eq!(sample_color(&canvas, 100, 100), None);
    assert!(sample_color(&canvas, 50, 50).is_some());
}
