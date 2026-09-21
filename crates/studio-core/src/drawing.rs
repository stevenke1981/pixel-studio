//! Raster drawing engine for Pixel Studio.
//!
//! Provides headless, pixel-level raster drawing primitives:
//! - Bresenham line (`draw_pencil_line`)
//! - Continuous circular brush stroke (`draw_brush_stroke`)
//! - Translucent highlighter stroke (`draw_highlighter_stroke`)
//! - Soft particle airbrush (`draw_airbrush`)
//! - Chisel-angled calligraphy stroke (`draw_calligraphy_stroke`)
//! - Background eraser (`draw_eraser`)
//! - Tolerance-aware BFS flood fill (`flood_fill`)
//! - Color sampling (`sample_color`)
//! - Regional pixelation (`apply_mosaic_region`)
//! - Raster shape primitives (`draw_rectangle`, `draw_circle`, `draw_ellipse`)

use image::{Rgba, RgbaImage};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

static AIRBRUSH_COUNTER: AtomicU64 = AtomicU64::new(0x853c_49e6_748f_ea9b);

/// Fast pseudo-random number generator using XorShift64.
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9e37_79b9_7f4a_7c15
            } else {
                seed
            },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / 16777216.0
    }
}

/// Blend source color over destination color using Porter-Duff source-over.
#[inline]
pub fn blend_pixel(dst: Rgba<u8>, src: Rgba<u8>) -> Rgba<u8> {
    let sa = src[3] as u32;
    if sa == 0 {
        return dst;
    }
    if sa == 255 {
        return src;
    }
    let da = dst[3] as u32;
    let inv_sa = 255 - sa;
    let out_a = sa + (da * inv_sa + 127) / 255;
    if out_a == 0 {
        return Rgba([0, 0, 0, 0]);
    }
    let sr = src[0] as u32;
    let sg = src[1] as u32;
    let sb = src[2] as u32;
    let dr = dst[0] as u32;
    let dg = dst[1] as u32;
    let db = dst[2] as u32;

    let out_r = ((sr * sa * 255 + dr * da * inv_sa) / (out_a * 255)).min(255) as u8;
    let out_g = ((sg * sa * 255 + dg * da * inv_sa) / (out_a * 255)).min(255) as u8;
    let out_b = ((sb * sa * 255 + db * da * inv_sa) / (out_a * 255)).min(255) as u8;

    Rgba([out_r, out_g, out_b, out_a.min(255) as u8])
}

/// Safely write a pixel with alpha blending.
#[inline]
pub fn put_pixel_blended(img: &mut RgbaImage, x: i32, y: i32, color: Rgba<u8>) {
    if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
        let ux = x as u32;
        let uy = y as u32;
        let dst = *img.get_pixel(ux, uy);
        img.put_pixel(ux, uy, blend_pixel(dst, color));
    }
}

/// Safely write a pixel directly (overwrite).
#[inline]
pub fn put_pixel_direct(img: &mut RgbaImage, x: i32, y: i32, color: Rgba<u8>) {
    if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
        img.put_pixel(x as u32, y as u32, color);
    }
}

/// Draw a continuous 1px hard-edge line between two points using Bresenham's algorithm.
pub fn draw_pencil_line(
    img: &mut RgbaImage,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    color: Rgba<u8>,
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        put_pixel_blended(img, x0, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// Stamp a filled circle centered at `(cx, cy)` with given radius.
fn stamp_circle(img: &mut RgbaImage, cx: i32, cy: i32, radius: f32, color: Rgba<u8>) {
    let r_ceil = radius.ceil() as i32;
    let r_sq = radius * radius;
    for dy in -r_ceil..=r_ceil {
        for dx in -r_ceil..=r_ceil {
            if (dx * dx + dy * dy) as f32 <= r_sq {
                put_pixel_blended(img, cx + dx, cy + dy, color);
            }
        }
    }
}

/// Draw a smooth circular brush stroke between two points with radius and alpha blending.
pub fn draw_brush_stroke(
    img: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    radius: f32,
    color: Rgba<u8>,
) {
    if radius <= 0.5 {
        draw_pencil_line(img, x0, y0, x1, y1, color);
        return;
    }

    let dx = (x1 - x0) as f32;
    let dy = (y1 - y0) as f32;
    let dist = (dx * dx + dy * dy).sqrt();
    let step = (radius * 0.5).clamp(0.5, 1.0);
    let steps = (dist / step).ceil() as usize;

    if steps == 0 {
        stamp_circle(img, x0, y0, radius, color);
        return;
    }

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let cx = (x0 as f32 + t * dx).round() as i32;
        let cy = (y0 as f32 + t * dy).round() as i32;
        stamp_circle(img, cx, cy, radius, color);
    }
}

/// Draw a translucent highlighter stroke between two points with uniform opacity.
#[allow(clippy::too_many_arguments)]
pub fn draw_highlighter_stroke(
    img: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    radius: f32,
    color: Rgba<u8>,
    alpha: u8,
) {
    let stroke_color = Rgba([color[0], color[1], color[2], alpha]);
    let r = radius.max(0.5);
    let r_sq = r * r;

    // Segment bounds
    let min_x = (x0.min(x1) as f32 - r).floor() as i32;
    let max_x = (x0.max(x1) as f32 + r).ceil() as i32;
    let min_y = (y0.min(y1) as f32 - r).floor() as i32;
    let max_y = (y0.max(y1) as f32 + r).ceil() as i32;

    let ax = x0 as f32;
    let ay = y0 as f32;
    let bx = x1 as f32;
    let by = y1 as f32;
    let vx = bx - ax;
    let vy = by - ay;
    let v_len_sq = vx * vx + vy * vy;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let px_f = px as f32;
            let py_f = py as f32;

            let dist_sq = if v_len_sq <= 0.0001 {
                let dx = px_f - ax;
                let dy = py_f - ay;
                dx * dx + dy * dy
            } else {
                let wx = px_f - ax;
                let wy = py_f - ay;
                let c1 = wx * vx + wy * vy;
                let t = (c1 / v_len_sq).clamp(0.0, 1.0);
                let proj_x = ax + t * vx;
                let proj_y = ay + t * vy;
                let dx = px_f - proj_x;
                let dy = py_f - proj_y;
                dx * dx + dy * dy
            };

            if dist_sq <= r_sq {
                put_pixel_blended(img, px, py, stroke_color);
            }
        }
    }
}

/// Spray airbrush particles within radius around (x, y).
pub fn draw_airbrush(
    img: &mut RgbaImage,
    x: i32,
    y: i32,
    radius: f32,
    density: u32,
    color: Rgba<u8>,
) {
    if radius <= 0.0 || density == 0 {
        return;
    }

    let seed = ((x as u64) << 32)
        ^ ((y as u64) << 16)
        ^ (density as u64)
        ^ AIRBRUSH_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut rng = XorShift64::new(seed);

    let particle_alpha = if color[3] > 0 {
        ((color[3] as u32 * 60) / 255).clamp(1, 255) as u8
    } else {
        0
    };
    let particle_color = Rgba([color[0], color[1], color[2], particle_alpha]);

    for _ in 0..density {
        let u1 = rng.next_f32();
        let u2 = rng.next_f32();
        let u3 = rng.next_f32();
        let angle = u1 * 2.0 * std::f32::consts::PI;
        let r = radius * (u2 * 0.7 + u3 * 0.3);
        let px = (x as f32 + r * angle.cos()).round() as i32;
        let py = (y as f32 + r * angle.sin()).round() as i32;
        put_pixel_blended(img, px, py, particle_color);
    }
}

/// Draw a chisel-angled calligraphy stroke.
#[allow(clippy::too_many_arguments)]
pub fn draw_calligraphy_stroke(
    img: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    width: f32,
    angle_deg: f32,
    color: Rgba<u8>,
) {
    let rad = angle_deg.to_radians();
    let hw = (width.max(1.0)) * 0.5;
    let dx_nib = hw * rad.cos();
    let dy_nib = hw * rad.sin();

    let dx = (x1 - x0) as f32;
    let dy = (y1 - y0) as f32;
    let dist = (dx * dx + dy * dy).sqrt();
    let steps = dist.ceil() as usize;

    let stamp_nib = |img: &mut RgbaImage, cx: f32, cy: f32| {
        let p0x = (cx - dx_nib).round() as i32;
        let p0y = (cy - dy_nib).round() as i32;
        let p1x = (cx + dx_nib).round() as i32;
        let p1y = (cy + dy_nib).round() as i32;
        draw_pencil_line(img, p0x, p0y, p1x, p1y, color);
    };

    if steps == 0 {
        stamp_nib(img, x0 as f32, y0 as f32);
        return;
    }

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let cx = x0 as f32 + t * dx;
        let cy = y0 as f32 + t * dy;
        stamp_nib(img, cx, cy);
    }
}

/// Erase pixels along a line segment to the specified background color.
pub fn draw_eraser(
    img: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    radius: f32,
    bg_color: Rgba<u8>,
) {
    if radius <= 0.5 {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut cur_x = x0;
        let mut cur_y = y0;

        loop {
            put_pixel_direct(img, cur_x, cur_y, bg_color);
            if cur_x == x1 && cur_y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                cur_x += sx;
            }
            if e2 <= dx {
                err += dx;
                cur_y += sy;
            }
        }
        return;
    }

    let dx = (x1 - x0) as f32;
    let dy = (y1 - y0) as f32;
    let dist = (dx * dx + dy * dy).sqrt();
    let step = (radius * 0.5).clamp(0.5, 1.0);
    let steps = (dist / step).ceil() as usize;

    let r_ceil = radius.ceil() as i32;
    let r_sq = radius * radius;

    let erase_circle = |img: &mut RgbaImage, cx: i32, cy: i32| {
        for dy in -r_ceil..=r_ceil {
            for dx in -r_ceil..=r_ceil {
                if (dx * dx + dy * dy) as f32 <= r_sq {
                    put_pixel_direct(img, cx + dx, cy + dy, bg_color);
                }
            }
        }
    };

    if steps == 0 {
        erase_circle(img, x0, y0);
        return;
    }

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let cx = (x0 as f32 + t * dx).round() as i32;
        let cy = (y0 as f32 + t * dy).round() as i32;
        erase_circle(img, cx, cy);
    }
}

/// Compute squared Euclidean distance between two colors.
#[inline]
fn color_distance_sq(c1: Rgba<u8>, c2: Rgba<u8>) -> f32 {
    let dr = c1[0] as f32 - c2[0] as f32;
    let dg = c1[1] as f32 - c2[1] as f32;
    let db = c1[2] as f32 - c2[2] as f32;
    let da = c1[3] as f32 - c2[3] as f32;
    dr * dr + dg * dg + db * db + da * da
}

/// Flood fill contiguous region starting at (start_x, start_y) within color tolerance.
pub fn flood_fill(
    img: &mut RgbaImage,
    start_x: u32,
    start_y: u32,
    fill_color: Rgba<u8>,
    tolerance: u8,
) {
    let width = img.width();
    let height = img.height();
    if start_x >= width || start_y >= height {
        return;
    }

    let target_color = *img.get_pixel(start_x, start_y);
    let tol_f = (tolerance as f32 / 255.0) * 510.0;
    let tol_sq = tol_f * tol_f;

    // Fast-path: if target is already identical to fill_color and tolerance is 0, no-op
    if target_color == fill_color && tolerance == 0 {
        return;
    }

    let total_pixels = (width as usize) * (height as usize);
    let mut visited = vec![false; total_pixels];
    let mut queue = VecDeque::with_capacity(1024);

    let start_idx = (start_y as usize) * (width as usize) + (start_x as usize);
    visited[start_idx] = true;
    img.put_pixel(start_x, start_y, fill_color);
    queue.push_back((start_x, start_y));

    while let Some((x, y)) = queue.pop_front() {
        let neighbors = [
            (x.wrapping_sub(1), y, x > 0),
            (x + 1, y, x + 1 < width),
            (x, y.wrapping_sub(1), y > 0),
            (x, y + 1, y + 1 < height),
        ];

        for (nx, ny, valid) in neighbors {
            if !valid {
                continue;
            }
            let idx = (ny as usize) * (width as usize) + (nx as usize);
            if !visited[idx] {
                visited[idx] = true;
                let cur = *img.get_pixel(nx, ny);
                if tolerance == 255 || color_distance_sq(cur, target_color) <= tol_sq {
                    img.put_pixel(nx, ny, fill_color);
                    queue.push_back((nx, ny));
                }
            }
        }
    }
}

/// Safely sample pixel color at (x, y), returning None if out of bounds.
pub fn sample_color(img: &RgbaImage, x: u32, y: u32) -> Option<Rgba<u8>> {
    if x < img.width() && y < img.height() {
        Some(*img.get_pixel(x, y))
    } else {
        None
    }
}

/// Apply mosaic / pixelation to a rectangular region of the image.
pub fn apply_mosaic_region(
    img: &mut RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    block_size: u32,
) {
    if width == 0 || height == 0 || block_size <= 1 {
        return;
    }

    let img_w = img.width();
    let img_h = img.height();

    let start_x = x.min(img_w);
    let start_y = y.min(img_h);
    let end_x = x.saturating_add(width).min(img_w);
    let end_y = y.saturating_add(height).min(img_h);

    if start_x >= end_x || start_y >= end_y {
        return;
    }

    let bs = block_size;
    let mut by = start_y;
    while by < end_y {
        let block_end_y = (by + bs).min(end_y);
        let mut bx = start_x;
        while bx < end_x {
            let block_end_x = (bx + bs).min(end_x);

            let mut sum_r = 0u64;
            let mut sum_g = 0u64;
            let mut sum_b = 0u64;
            let mut sum_a = 0u64;
            let mut count = 0u64;

            for py in by..block_end_y {
                for px in bx..block_end_x {
                    let p = img.get_pixel(px, py);
                    sum_r += p[0] as u64;
                    sum_g += p[1] as u64;
                    sum_b += p[2] as u64;
                    sum_a += p[3] as u64;
                    count += 1;
                }
            }

            if count > 0 {
                let avg = Rgba([
                    ((sum_r + count / 2) / count) as u8,
                    ((sum_g + count / 2) / count) as u8,
                    ((sum_b + count / 2) / count) as u8,
                    ((sum_a + count / 2) / count) as u8,
                ]);

                for py in by..block_end_y {
                    for px in bx..block_end_x {
                        img.put_pixel(px, py, avg);
                    }
                }
            }

            bx += bs;
        }
        by += bs;
    }
}

/// Draw a rectangle (outline or filled) between two corners.
#[allow(clippy::too_many_arguments)]
pub fn draw_rectangle(
    img: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: Rgba<u8>,
    stroke_width: u32,
    filled: bool,
) {
    let min_x = x0.min(x1);
    let max_x = x0.max(x1);
    let min_y = y0.min(y1);
    let max_y = y0.max(y1);

    if filled {
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                put_pixel_blended(img, x, y, color);
            }
        }
    } else {
        let sw = (stroke_width.max(1)) as i32;
        for i in 0..sw {
            let x0 = min_x + i;
            let x1 = max_x - i;
            let y0 = min_y + i;
            let y1 = max_y - i;
            if x0 > x1 || y0 > y1 {
                break;
            }
            draw_pencil_line(img, x0, y0, x1, y0, color);
            draw_pencil_line(img, x0, y1, x1, y1, color);
            draw_pencil_line(img, x0, y0, x0, y1, color);
            draw_pencil_line(img, x1, y0, x1, y1, color);
        }
    }
}

/// Draw a circle (outline or filled) centered at (cx, cy).
pub fn draw_circle(
    img: &mut RgbaImage,
    cx: i32,
    cy: i32,
    radius: i32,
    color: Rgba<u8>,
    stroke_width: u32,
    filled: bool,
) {
    if radius <= 0 {
        put_pixel_blended(img, cx, cy, color);
        return;
    }

    if filled {
        let r_sq = radius * radius;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy <= r_sq {
                    put_pixel_blended(img, cx + dx, cy + dy, color);
                }
            }
        }
    } else {
        let sw = stroke_width.max(1) as i32;
        let outer_r = radius;
        let inner_r = (radius - sw).max(0);
        let outer_sq = outer_r * outer_r;
        let inner_sq = inner_r * inner_r;

        for dy in -outer_r..=outer_r {
            for dx in -outer_r..=outer_r {
                let d_sq = dx * dx + dy * dy;
                if d_sq <= outer_sq && d_sq >= inner_sq {
                    put_pixel_blended(img, cx + dx, cy + dy, color);
                }
            }
        }
    }
}

/// Draw an ellipse (outline or filled) centered at (cx, cy) with radii (rx, ry).
#[allow(clippy::too_many_arguments)]
pub fn draw_ellipse(
    img: &mut RgbaImage,
    cx: i32,
    cy: i32,
    rx: i32,
    ry: i32,
    color: Rgba<u8>,
    stroke_width: u32,
    filled: bool,
) {
    if rx <= 0 || ry <= 0 {
        put_pixel_blended(img, cx, cy, color);
        return;
    }

    let rx_f = rx as f32;
    let ry_f = ry as f32;

    if filled {
        for dy in -ry..=ry {
            for dx in -rx..=rx {
                let norm = (dx as f32 / rx_f).powi(2) + (dy as f32 / ry_f).powi(2);
                if norm <= 1.0 {
                    put_pixel_blended(img, cx + dx, cy + dy, color);
                }
            }
        }
    } else {
        let sw = stroke_width.max(1) as f32;
        let inner_rx = (rx_f - sw).max(0.0);
        let inner_ry = (ry_f - sw).max(0.0);

        for dy in -ry..=ry {
            for dx in -rx..=rx {
                let norm_outer = (dx as f32 / rx_f).powi(2) + (dy as f32 / ry_f).powi(2);
                let is_inside_stroke = if inner_rx <= 0.0 || inner_ry <= 0.0 {
                    norm_outer <= 1.0
                } else {
                    let norm_inner =
                        (dx as f32 / inner_rx).powi(2) + (dy as f32 / inner_ry).powi(2);
                    norm_outer <= 1.0 && norm_inner >= 1.0
                };

                if is_inside_stroke {
                    put_pixel_blended(img, cx + dx, cy + dy, color);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_pencil_line_bresenham() {
        let mut img = RgbaImage::from_pixel(20, 20, Rgba([255, 255, 255, 255]));
        let black = Rgba([0, 0, 0, 255]);
        draw_pencil_line(&mut img, 2, 2, 8, 2, black);

        for x in 2..=8 {
            assert_eq!(*img.get_pixel(x, 2), black);
        }
        assert_eq!(*img.get_pixel(1, 2), Rgba([255, 255, 255, 255]));
        assert_eq!(*img.get_pixel(9, 2), Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn test_draw_pencil_line_diagonal() {
        let mut img = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        let red = Rgba([255, 0, 0, 255]);
        draw_pencil_line(&mut img, 1, 1, 5, 5, red);

        for i in 1..=5 {
            assert_eq!(*img.get_pixel(i, i), red);
        }
    }

    #[test]
    fn test_draw_brush_stroke() {
        let mut img = RgbaImage::from_pixel(30, 30, Rgba([255, 255, 255, 255]));
        let blue = Rgba([0, 0, 255, 255]);
        draw_brush_stroke(&mut img, 15, 15, 15, 15, 3.0, blue);

        assert_eq!(*img.get_pixel(15, 15), blue);
        assert_eq!(*img.get_pixel(15, 17), blue);
        assert_eq!(*img.get_pixel(15, 12), blue);
        assert_eq!(*img.get_pixel(15, 19), Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn test_draw_highlighter_stroke() {
        let mut img = RgbaImage::from_pixel(20, 20, Rgba([255, 255, 255, 255]));
        let yellow = Rgba([255, 255, 0, 255]);
        draw_highlighter_stroke(&mut img, 5, 10, 15, 10, 2.0, yellow, 128);

        let p = img.get_pixel(10, 10);
        // Yellow over white with ~128 alpha: R should be 255, G 255, B < 255
        assert_eq!(p[0], 255);
        assert_eq!(p[1], 255);
        assert!(p[2] < 200, "Blue channel should be reduced by yellow blend");
        assert_eq!(p[3], 255);
    }

    #[test]
    fn test_draw_airbrush() {
        let mut img = RgbaImage::from_pixel(40, 40, Rgba([255, 255, 255, 255]));
        let red = Rgba([255, 0, 0, 255]);
        draw_airbrush(&mut img, 20, 20, 10.0, 50, red);

        // At least one pixel inside the spray circle should have been altered
        let mut modified = 0;
        for y in 10..=30 {
            for x in 10..=30 {
                if *img.get_pixel(x, y) != Rgba([255, 255, 255, 255]) {
                    modified += 1;
                }
            }
        }
        assert!(modified > 0, "Airbrush must spray particles");

        // Out-of-radius pixels should remain unchanged
        assert_eq!(*img.get_pixel(0, 0), Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn test_draw_calligraphy_stroke() {
        let mut img = RgbaImage::from_pixel(30, 30, Rgba([255, 255, 255, 255]));
        let green = Rgba([0, 255, 0, 255]);
        // 45 degrees chisel, width 6
        draw_calligraphy_stroke(&mut img, 15, 15, 15, 15, 6.0, 45.0, green);

        assert_eq!(*img.get_pixel(15, 15), green);
        // Check chisel angle orientation
        let p_diag = *img.get_pixel(17, 17);
        assert_eq!(p_diag, green);
    }

    #[test]
    fn test_draw_eraser() {
        let mut img = RgbaImage::from_pixel(20, 20, Rgba([255, 0, 0, 255]));
        let bg_color = Rgba([255, 255, 255, 255]);
        draw_eraser(&mut img, 5, 5, 15, 5, 2.0, bg_color);

        for x in 5..=15 {
            assert_eq!(*img.get_pixel(x, 5), bg_color);
        }
        assert_eq!(*img.get_pixel(5, 15), Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn test_flood_fill_basic() {
        let mut img = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 255]));
        // Draw a boundary box
        let red = Rgba([255, 0, 0, 255]);
        for x in 2..=8 {
            img.put_pixel(x, 2, red);
            img.put_pixel(x, 8, red);
        }
        for y in 2..=8 {
            img.put_pixel(2, y, red);
            img.put_pixel(8, y, red);
        }

        let blue = Rgba([0, 0, 255, 255]);
        flood_fill(&mut img, 5, 5, blue, 0);

        // Center should be blue
        assert_eq!(*img.get_pixel(5, 5), blue);
        assert_eq!(*img.get_pixel(3, 3), blue);
        assert_eq!(*img.get_pixel(7, 7), blue);

        // Boundary should still be red
        assert_eq!(*img.get_pixel(2, 5), red);
        assert_eq!(*img.get_pixel(8, 5), red);

        // Outside should still be black
        assert_eq!(*img.get_pixel(0, 0), Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn test_flood_fill_tolerance() {
        let mut img = RgbaImage::from_pixel(10, 10, Rgba([100, 100, 100, 255]));
        img.put_pixel(4, 5, Rgba([105, 102, 98, 255]));
        img.put_pixel(6, 5, Rgba([180, 180, 180, 255])); // Different, out of tolerance

        let green = Rgba([0, 255, 0, 255]);
        flood_fill(&mut img, 5, 5, green, 15);

        assert_eq!(*img.get_pixel(4, 5), green);
        assert_eq!(*img.get_pixel(6, 5), Rgba([180, 180, 180, 255]));
    }

    #[test]
    fn test_sample_color() {
        let mut img = RgbaImage::from_pixel(10, 10, Rgba([12, 34, 56, 78]));
        img.put_pixel(3, 4, Rgba([99, 88, 77, 66]));

        assert_eq!(sample_color(&img, 3, 4), Some(Rgba([99, 88, 77, 66])));
        assert_eq!(sample_color(&img, 0, 0), Some(Rgba([12, 34, 56, 78])));
        assert_eq!(sample_color(&img, 10, 10), None);
        assert_eq!(sample_color(&img, 999, 0), None);
    }

    #[test]
    fn test_apply_mosaic_region() {
        let mut img = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 255]));
        // Put half white, half black
        for y in 0..4 {
            for x in 0..4 {
                img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        // Apply 4x4 mosaic on 0..4, 0..4
        apply_mosaic_region(&mut img, 0, 0, 4, 4, 4);

        let p = *img.get_pixel(0, 0);
        assert_eq!(p, Rgba([255, 255, 255, 255]));

        // Now mosaic whole image with 8x8
        apply_mosaic_region(&mut img, 0, 0, 8, 8, 8);
        // 16 pixels are 255, 48 pixels are 0 => average is 255 * 16 / 64 = 63.75 -> 64
        let p_avg = *img.get_pixel(0, 0);
        assert!((p_avg[0] as i32 - 64).abs() <= 1);
        assert_eq!(p_avg, *img.get_pixel(7, 7));
    }

    #[test]
    fn test_draw_shapes() {
        let mut img = RgbaImage::from_pixel(30, 30, Rgba([0, 0, 0, 255]));
        let white = Rgba([255, 255, 255, 255]);

        draw_rectangle(&mut img, 2, 2, 8, 8, white, 1, false);
        assert_eq!(*img.get_pixel(2, 2), white);
        assert_eq!(*img.get_pixel(5, 5), Rgba([0, 0, 0, 255]));

        draw_rectangle(&mut img, 10, 10, 15, 15, white, 1, true);
        assert_eq!(*img.get_pixel(12, 12), white);

        draw_circle(&mut img, 20, 20, 5, white, 1, true);
        assert_eq!(*img.get_pixel(20, 20), white);
    }
}
