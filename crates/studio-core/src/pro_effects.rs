//! Pro spatial effects, cutout/selection tools, framing polish, and raster brush primitives.
//!
//! Pure-Rust image processing algorithms with zero UI dependencies:
//! - Alpha cutout & smart selection (`magic_wand_mask`, `remove_background_color`, `auto_trim`)
//! - Pro framing & UI screenshot polish (`round_corners`, `add_drop_shadow`, `add_border_frame`)
//! - Pro spatial & stylize filters (`tilt_shift`, `pixelate`, `film_grain`, `emboss`, `edge_detect`, `oil_paint`)
//! - Gradient & clone stamp raster primitives (`draw_gradient`, `clone_stamp_dab`)

use std::collections::VecDeque;

use image::{DynamicImage, GenericImageView, GrayImage, Luma, Rgba, RgbaImage};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::drawing::{apply_mosaic_region, blend_pixel};
use crate::filters::Filter;
use crate::util::{clamp_channel, lerp_channel};

// ── Shared math & color helpers ──

#[inline]
fn rgba_distance_sq(a: Rgba<u8>, b: Rgba<u8>) -> f32 {
    let dr = f32::from(a[0]) - f32::from(b[0]);
    let dg = f32::from(a[1]) - f32::from(b[1]);
    let db = f32::from(a[2]) - f32::from(b[2]);
    let da = f32::from(a[3]) - f32::from(b[3]);
    dr * dr + dg * dg + db * db + da * da
}

#[inline]
fn rgb_distance(pixel: Rgba<u8>, target: [u8; 3]) -> f32 {
    let dr = f32::from(pixel[0]) - f32::from(target[0]);
    let dg = f32::from(pixel[1]) - f32::from(target[1]);
    let db = f32::from(pixel[2]) - f32::from(target[2]);
    (dr * dr + dg * dg + db * db).sqrt()
}

#[inline]
fn max_channel_diff(a: Rgba<u8>, b: Rgba<u8>) -> u8 {
    a[0].abs_diff(b[0])
        .max(a[1].abs_diff(b[1]))
        .max(a[2].abs_diff(b[2]))
        .max(a[3].abs_diff(b[3]))
}

#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge1 <= edge0 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn luma_bt709(p: Rgba<u8>) -> f32 {
    0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2])
}

/// Signed distance to a rectangle `[0..width, 0..height]` with rounded corners of `radius`.
#[inline]
fn rounded_rect_sdf(px: f32, py: f32, width: f32, height: f32, radius: f32) -> f32 {
    let max_r = 0.5 * width.min(height);
    let r = radius.clamp(0.0, max_r);
    let cx = width * 0.5;
    let cy = height * 0.5;
    let hx = (cx - r).max(0.0);
    let hy = (cy - r).max(0.0);
    let qx = (px - cx).abs() - hx;
    let qy = (py - cy).abs() - hy;
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

#[inline]
fn lerp_premultiplied(c0: Rgba<u8>, c1: Rgba<u8>, t: f32) -> Rgba<u8> {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.0 {
        return c0;
    }
    if t >= 1.0 {
        return c1;
    }
    let a0 = f32::from(c0[3]) / 255.0;
    let a1 = f32::from(c1[3]) / 255.0;
    let out_a = a0 + (a1 - a0) * t;
    if out_a <= 1e-6 {
        return Rgba([0, 0, 0, 0]);
    }
    let inv_a = 1.0 / out_a;
    let r = ((f32::from(c0[0]) * a0 * (1.0 - t) + f32::from(c1[0]) * a1 * t) * inv_a)
        .round()
        .clamp(0.0, 255.0) as u8;
    let g = ((f32::from(c0[1]) * a0 * (1.0 - t) + f32::from(c1[1]) * a1 * t) * inv_a)
        .round()
        .clamp(0.0, 255.0) as u8;
    let b = ((f32::from(c0[2]) * a0 * (1.0 - t) + f32::from(c1[2]) * a1 * t) * inv_a)
        .round()
        .clamp(0.0, 255.0) as u8;
    let a = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgba([r, g, b, a])
}

/// Deterministic XorShift64 PRNG for repeatable grain synthesis.
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

    #[inline]
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    #[inline]
    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / 16_777_216.0
    }
}

// ── 1. Alpha Cutout & Smart Selection ──

/// Trim criterion for [`auto_trim`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrimMode {
    /// Trim outer rows and columns whose alpha channel is zero.
    Transparent,
    /// Trim outer rows and columns matching the top-left corner color within tolerance.
    CornerColor(u8),
}

/// Bounding rectangle retained by [`auto_trim`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrimResult {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Compute a 0/255 selection mask matching the pixel at `(start_x, start_y)`.
///
/// Out-of-bounds seed coordinates safely return an all-zero `width x height` mask.
/// When `contiguous` is true, performs a 4-connected BFS flood fill; otherwise
/// selects all matching pixels across the entire image.
pub fn magic_wand_mask(
    image: &DynamicImage,
    start_x: u32,
    start_y: u32,
    tolerance: u8,
    contiguous: bool,
) -> GrayImage {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut mask = GrayImage::new(width, height);

    if width == 0 || height == 0 || start_x >= width || start_y >= height {
        return mask;
    }

    let target = *rgba.get_pixel(start_x, start_y);
    let tol_f = (f32::from(tolerance) / 255.0) * 510.0;
    let tol_sq = tol_f * tol_f;
    let matches =
        |px: Rgba<u8>| -> bool { tolerance == 255 || rgba_distance_sq(px, target) <= tol_sq };

    if !contiguous {
        for y in 0..height {
            for x in 0..width {
                if matches(*rgba.get_pixel(x, y)) {
                    mask.put_pixel(x, y, Luma([255]));
                }
            }
        }
        return mask;
    }

    let total = (width as usize) * (height as usize);
    let mut visited = vec![false; total];
    let mut queue = VecDeque::with_capacity(1024);

    let start_idx = (start_y as usize) * (width as usize) + (start_x as usize);
    visited[start_idx] = true;
    mask.put_pixel(start_x, start_y, Luma([255]));
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
                if matches(*rgba.get_pixel(nx, ny)) {
                    mask.put_pixel(nx, ny, Luma([255]));
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    mask
}

/// Remove pixels matching `target` RGB color by smoothly tapering alpha to 0.
///
/// Pixels within `tolerance` become fully transparent; pixels between `tolerance`
/// and `tolerance + feather` transition smoothly; when `edge_connected` is true,
/// only background regions reachable from the image perimeter are affected.
pub fn remove_background_color(
    image: &DynamicImage,
    target: [u8; 3],
    tolerance: f32,
    feather: f32,
    edge_connected: bool,
) -> DynamicImage {
    let mut rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return DynamicImage::ImageRgba8(rgba);
    }

    let tol = tolerance.max(0.0);
    let feather = feather.max(0.0);
    let max_dist = tol + feather;

    let alpha_scale_for_dist = |dist: f32| -> f32 {
        if dist <= tol {
            0.0
        } else if feather > 0.0 && dist < max_dist {
            smoothstep(tol, max_dist, dist)
        } else {
            1.0
        }
    };

    if !edge_connected {
        for pixel in rgba.pixels_mut() {
            let dist = rgb_distance(*pixel, target);
            let scale = alpha_scale_for_dist(dist);
            if scale < 1.0 {
                pixel[3] = (f32::from(pixel[3]) * scale).round().clamp(0.0, 255.0) as u8;
            }
        }
        return DynamicImage::ImageRgba8(rgba);
    }

    let total = (width as usize) * (height as usize);
    let mut visited = vec![false; total];
    let mut queue = VecDeque::with_capacity((width + height) as usize * 2);

    let in_reach = |px: Rgba<u8>| -> bool {
        let dist = rgb_distance(px, target);
        if feather > 0.0 {
            dist < max_dist
        } else {
            dist <= tol
        }
    };

    let mut try_seed = |x: u32, y: u32| {
        let idx = (y as usize) * (width as usize) + (x as usize);
        if !visited[idx] {
            visited[idx] = true;
            if in_reach(*rgba.get_pixel(x, y)) {
                queue.push_back((x, y));
            }
        }
    };

    for x in 0..width {
        try_seed(x, 0);
        if height > 1 {
            try_seed(x, height - 1);
        }
    }
    for y in 1..height.saturating_sub(1) {
        try_seed(0, y);
        if width > 1 {
            try_seed(width - 1, y);
        }
    }

    while let Some((x, y)) = queue.pop_front() {
        let pixel = rgba.get_pixel_mut(x, y);
        let dist = rgb_distance(*pixel, target);
        let scale = alpha_scale_for_dist(dist);
        pixel[3] = (f32::from(pixel[3]) * scale).round().clamp(0.0, 255.0) as u8;

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
                if in_reach(*rgba.get_pixel(nx, ny)) {
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    DynamicImage::ImageRgba8(rgba)
}

/// Crop excess transparent or uniform-corner borders from an image.
///
/// If the entire image is transparent or uniform under `mode`, safely keeps the
/// original bounds (at least 1x1 when non-empty) without panicking.
pub fn auto_trim(image: &DynamicImage, mode: TrimMode) -> (DynamicImage, TrimResult) {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return (
            image.clone(),
            TrimResult {
                x: 0,
                y: 0,
                width,
                height,
            },
        );
    }

    let corner = *rgba.get_pixel(0, 0);
    let is_content = |px: Rgba<u8>| -> bool {
        match mode {
            TrimMode::Transparent => px[3] > 0,
            TrimMode::CornerColor(tol) => max_channel_diff(px, corner) > tol,
        }
    };

    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;

    for y in 0..height {
        for x in 0..width {
            if is_content(*rgba.get_pixel(x, y)) {
                found = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    if !found {
        return (
            image.clone(),
            TrimResult {
                x: 0,
                y: 0,
                width,
                height,
            },
        );
    }

    let trim_w = max_x - min_x + 1;
    let trim_h = max_y - min_y + 1;
    let cropped = image.crop_imm(min_x, min_y, trim_w, trim_h);
    (
        cropped,
        TrimResult {
            x: min_x,
            y: min_y,
            width: trim_w,
            height: trim_h,
        },
    )
}

// ── 2. Pro Framing & UI Screenshot Polish ──

/// Round image corners smoothly using an anti-aliased signed distance field.
pub fn round_corners(image: &DynamicImage, radius: f32) -> DynamicImage {
    let mut rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 || radius <= 0.0 {
        return DynamicImage::ImageRgba8(rgba);
    }

    let w_f = width as f32;
    let h_f = height as f32;
    let r = radius.min(0.5 * w_f.min(h_f));

    for y in 0..height {
        let py = y as f32 + 0.5;
        for x in 0..width {
            let px = x as f32 + 0.5;
            let sdf = rounded_rect_sdf(px, py, w_f, h_f, r);
            if sdf > -0.5 {
                let coverage = (0.5 - sdf).clamp(0.0, 1.0);
                let pixel = rgba.get_pixel_mut(x, y);
                pixel[3] = (f32::from(pixel[3]) * coverage).round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    DynamicImage::ImageRgba8(rgba)
}

/// Expand the canvas, render a blurred alpha-derived drop shadow, and composite the image on top.
pub fn add_drop_shadow(
    image: &DynamicImage,
    offset_x: i32,
    offset_y: i32,
    blur_sigma: f32,
    shadow_color: [u8; 4],
    padding: u32,
) -> DynamicImage {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return DynamicImage::ImageRgba8(rgba);
    }

    let sigma = blur_sigma.max(0.0);
    let blur_margin = if sigma > 0.0 {
        (sigma * 3.0).ceil() as u32
    } else {
        0
    };

    let pad_left = padding
        .saturating_add(blur_margin)
        .saturating_add((-offset_x).max(0) as u32);
    let pad_right = padding
        .saturating_add(blur_margin)
        .saturating_add(offset_x.max(0) as u32);
    let pad_top = padding
        .saturating_add(blur_margin)
        .saturating_add((-offset_y).max(0) as u32);
    let pad_bottom = padding
        .saturating_add(blur_margin)
        .saturating_add(offset_y.max(0) as u32);

    let out_w = width.saturating_add(pad_left).saturating_add(pad_right);
    let out_h = height.saturating_add(pad_top).saturating_add(pad_bottom);

    let mut out = RgbaImage::new(out_w, out_h);

    if shadow_color[3] > 0 {
        let mut alpha_mask = GrayImage::new(out_w, out_h);
        for y in 0..height {
            let sy = y as i32 + pad_top as i32 + offset_y;
            if sy < 0 || sy >= out_h as i32 {
                continue;
            }
            for x in 0..width {
                let sx = x as i32 + pad_left as i32 + offset_x;
                if sx < 0 || sx >= out_w as i32 {
                    continue;
                }
                let a = rgba.get_pixel(x, y)[3];
                if a > 0 {
                    alpha_mask.put_pixel(sx as u32, sy as u32, Luma([a]));
                }
            }
        }

        let blurred_alpha = if sigma > 0.0 {
            image::imageops::blur(&alpha_mask, sigma)
        } else {
            alpha_mask
        };

        let sa = f32::from(shadow_color[3]) / 255.0;
        for (dst, mask_px) in out.pixels_mut().zip(blurred_alpha.pixels()) {
            let a = (f32::from(mask_px[0]) * sa).round().clamp(0.0, 255.0) as u8;
            if a > 0 {
                *dst = Rgba([shadow_color[0], shadow_color[1], shadow_color[2], a]);
            }
        }
    }

    for y in 0..height {
        let dy = pad_top + y;
        for x in 0..width {
            let dx = pad_left + x;
            let src = *rgba.get_pixel(x, y);
            let dst = *out.get_pixel(dx, dy);
            out.put_pixel(dx, dy, blend_pixel(dst, src));
        }
    }

    DynamicImage::ImageRgba8(out)
}

/// Configuration for [`add_border_frame`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FrameCardOptions {
    pub padding: u32,
    pub corner_radius: f32,
    pub shadow_sigma: f32,
    pub shadow_offset_x: i32,
    pub shadow_offset_y: i32,
    pub background_color: [u8; 4],
    pub border_width: u32,
    pub border_color: [u8; 4],
}

impl Default for FrameCardOptions {
    fn default() -> Self {
        Self {
            padding: 32,
            corner_radius: 12.0,
            shadow_sigma: 12.0,
            shadow_offset_x: 0,
            shadow_offset_y: 8,
            background_color: [240, 244, 250, 255],
            border_width: 1,
            border_color: [220, 226, 234, 255],
        }
    }
}

/// Create a polished framed screenshot/card with rounded corners, optional border, drop shadow, and background.
pub fn add_border_frame(image: &DynamicImage, opts: FrameCardOptions) -> DynamicImage {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return image.clone();
    }

    let corner_r = opts.corner_radius.max(0.0);
    let rounded_content = if corner_r > 0.0 {
        round_corners(image, corner_r).to_rgba8()
    } else {
        image.to_rgba8()
    };

    let card = if opts.border_width > 0 && opts.border_color[3] > 0 {
        let bw = opts.border_width;
        let card_w = width.saturating_add(bw.saturating_mul(2));
        let card_h = height.saturating_add(bw.saturating_mul(2));
        let outer_r = if corner_r > 0.0 {
            corner_r + bw as f32
        } else {
            0.0
        };
        let mut framed = RgbaImage::new(card_w, card_h);

        for y in 0..card_h {
            let py = y as f32 + 0.5;
            for x in 0..card_w {
                let px = x as f32 + 0.5;
                let outer_sdf = rounded_rect_sdf(px, py, card_w as f32, card_h as f32, outer_r);
                let outer_cov = (0.5 - outer_sdf).clamp(0.0, 1.0);
                if outer_cov <= 0.0 {
                    continue;
                }

                let in_inner_box = x >= bw && x < bw + width && y >= bw && y < bw + height;
                let inner_sdf = if in_inner_box {
                    rounded_rect_sdf(
                        px - bw as f32,
                        py - bw as f32,
                        width as f32,
                        height as f32,
                        corner_r,
                    )
                } else {
                    1.0
                };

                // Slight overlap under the anti-aliased inner corner avoids transparent seams.
                let border_cov = if inner_sdf < -0.5 { 0.0 } else { outer_cov };
                if border_cov > 0.0 {
                    let ba = (f32::from(opts.border_color[3]) * border_cov)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    framed.put_pixel(
                        x,
                        y,
                        Rgba([
                            opts.border_color[0],
                            opts.border_color[1],
                            opts.border_color[2],
                            ba,
                        ]),
                    );
                }

                if in_inner_box {
                    let src = *rounded_content.get_pixel(x - bw, y - bw);
                    let dst = *framed.get_pixel(x, y);
                    framed.put_pixel(x, y, blend_pixel(dst, src));
                }
            }
        }

        framed
    } else {
        rounded_content
    };

    let has_shadow =
        opts.shadow_sigma > 0.0 || opts.shadow_offset_x != 0 || opts.shadow_offset_y != 0;
    let shadow_color = if has_shadow {
        [0, 0, 0, 110]
    } else {
        [0, 0, 0, 0]
    };

    let mut result = add_drop_shadow(
        &DynamicImage::ImageRgba8(card),
        opts.shadow_offset_x,
        opts.shadow_offset_y,
        opts.shadow_sigma.max(0.0),
        shadow_color,
        opts.padding,
    )
    .to_rgba8();

    if opts.background_color[3] > 0 {
        let bg = Rgba(opts.background_color);
        for pixel in result.pixels_mut() {
            *pixel = blend_pixel(bg, *pixel);
        }
    }

    DynamicImage::ImageRgba8(result)
}

// ── 3. Pro Spatial & Stylize Filters ──

/// Miniature / tilt-shift selective focus filter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TiltShift {
    pub focus_y: f32,
    pub band_width: f32,
    pub blur_sigma: f32,
}

impl Filter for TiltShift {
    fn name(&self) -> &'static str {
        "Tilt Shift"
    }

    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        tilt_shift(image, self.focus_y, self.band_width, self.blur_sigma)
    }
}

/// Apply a miniature tilt-shift blur with a sharp horizontal focus band around `focus_y`.
pub fn tilt_shift(
    image: &DynamicImage,
    focus_y: f32,
    band_width: f32,
    blur_sigma: f32,
) -> DynamicImage {
    let focus_y = focus_y.clamp(0.0, 1.0);
    let band_width = band_width.clamp(0.05, 0.9);
    let blur_sigma = blur_sigma.max(0.0);

    let (width, height) = image.dimensions();
    if width == 0 || height == 0 || blur_sigma <= f32::EPSILON {
        return image.clone();
    }

    let orig = image.to_rgba8();
    let blurred = image::imageops::blur(&orig, blur_sigma);
    let mut out = orig.clone();

    let half_band = band_width * 0.5;
    let inner = half_band * 0.5;
    let outer = (half_band * 1.5).max(inner + 0.05);
    let h_f = height as f32;

    for y in 0..height {
        let ny = (y as f32 + 0.5) / h_f;
        let dist = (ny - focus_y).abs();
        let t = smoothstep(inner, outer, dist);
        if t <= 0.0 {
            continue;
        }
        for x in 0..width {
            let o = *orig.get_pixel(x, y);
            let b = *blurred.get_pixel(x, y);
            out.put_pixel(
                x,
                y,
                Rgba([
                    lerp_channel(f32::from(o[0]), f32::from(b[0]), t),
                    lerp_channel(f32::from(o[1]), f32::from(b[1]), t),
                    lerp_channel(f32::from(o[2]), f32::from(b[2]), t),
                    o[3],
                ]),
            );
        }
    }

    DynamicImage::ImageRgba8(out)
}

/// Full-image mosaic pixelation filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pixelate {
    pub block_size: u32,
}

impl Filter for Pixelate {
    fn name(&self) -> &'static str {
        "Pixelate"
    }

    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        pixelate(image, self.block_size)
    }
}

/// Pixelate the entire image by averaging colors over `block_size x block_size` blocks.
pub fn pixelate(image: &DynamicImage, block_size: u32) -> DynamicImage {
    let bs = block_size.max(1);
    let mut rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    if bs <= 1 || width == 0 || height == 0 {
        return DynamicImage::ImageRgba8(rgba);
    }
    apply_mosaic_region(&mut rgba, 0, 0, width, height, bs);
    DynamicImage::ImageRgba8(rgba)
}

/// Deterministic monochrome film grain filter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilmGrain {
    pub strength: f32,
}

impl Filter for FilmGrain {
    fn name(&self) -> &'static str {
        "Film Grain"
    }

    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        film_grain(image, self.strength)
    }
}

/// Add deterministic XorShift luma film grain (`strength` in `0.0..=1.0`) while preserving alpha.
pub fn film_grain(image: &DynamicImage, strength: f32) -> DynamicImage {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= f32::EPSILON {
        return image.clone();
    }

    let mut rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return DynamicImage::ImageRgba8(rgba);
    }

    let amplitude = strength * 64.0;
    let row_bytes = (width as usize) * 4;

    rgba.as_mut()
        .par_chunks_mut(row_bytes)
        .enumerate()
        .for_each(|(y, row)| {
            let seed = (y as u64)
                .wrapping_add(1)
                .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                ^ 0xbf58_476d_1ce4_e5b9;
            let mut rng = XorShift64::new(seed);
            for px in row.as_chunks_mut::<4>().0 {
                let noise = (rng.next_f32() + rng.next_f32() - 1.0) * amplitude;
                px[0] = clamp_channel(f32::from(px[0]) + noise);
                px[1] = clamp_channel(f32::from(px[1]) + noise);
                px[2] = clamp_channel(f32::from(px[2]) + noise);
            }
        });

    DynamicImage::ImageRgba8(rgba)
}

/// Directional 3x3 emboss filter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Emboss {
    pub strength: f32,
}

impl Filter for Emboss {
    fn name(&self) -> &'static str {
        "Emboss"
    }

    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        emboss(image, self.strength)
    }
}

/// Apply a 3x3 directional emboss kernel (`strength` in `0.0..=2.0`) blended with the original.
pub fn emboss(image: &DynamicImage, strength: f32) -> DynamicImage {
    let strength = strength.clamp(0.0, 2.0);
    if strength <= f32::EPSILON {
        return image.clone();
    }

    let src = image.to_rgba8();
    let (width, height) = src.dimensions();
    if width == 0 || height == 0 {
        return DynamicImage::ImageRgba8(src);
    }

    let mut out = RgbaImage::new(width, height);
    let blend = strength.min(1.0);
    let max_x = width as i32 - 1;
    let max_y = height as i32 - 1;
    let row_bytes = (width as usize) * 4;

    const KERNEL: [(i32, i32, f32); 6] = [
        (-1, -1, -2.0),
        (0, -1, -1.0),
        (-1, 0, -1.0),
        (1, 0, 1.0),
        (0, 1, 1.0),
        (1, 1, 2.0),
    ];

    out.as_mut()
        .par_chunks_mut(row_bytes)
        .enumerate()
        .for_each(|(y, row)| {
            let yi = y as i32;
            for x in 0..width {
                let xi = x as i32;
                let mut conv = [0.0f32; 3];
                for &(dx, dy, weight) in &KERNEL {
                    let sx = (xi + dx).clamp(0, max_x) as u32;
                    let sy = (yi + dy).clamp(0, max_y) as u32;
                    let p = src.get_pixel(sx, sy);
                    conv[0] += f32::from(p[0]) * weight;
                    conv[1] += f32::from(p[1]) * weight;
                    conv[2] += f32::from(p[2]) * weight;
                }

                let orig = src.get_pixel(x, y as u32);
                let idx = (x as usize) * 4;
                for (c, slot) in row[idx..idx + 3].iter_mut().enumerate() {
                    let relief = (128.0 + conv[c] * strength).clamp(0.0, 255.0);
                    *slot = lerp_channel(f32::from(orig[c]), relief, blend);
                }
                row[idx + 3] = orig[3];
            }
        });

    DynamicImage::ImageRgba8(out)
}

/// Sobel gradient magnitude edge-detection filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeDetect {
    pub invert_edges: bool,
}

impl Filter for EdgeDetect {
    fn name(&self) -> &'static str {
        "Edge Detect"
    }

    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        edge_detect(image, self.invert_edges)
    }
}

/// Compute a Sobel edge magnitude map preserving alpha.
///
/// Setting `invert_edges = true` yields dark ink edges on a white background.
pub fn edge_detect(image: &DynamicImage, invert_edges: bool) -> DynamicImage {
    let src = image.to_rgba8();
    let (width, height) = src.dimensions();
    if width == 0 || height == 0 {
        return DynamicImage::ImageRgba8(src);
    }

    let luma: Vec<f32> = src.pixels().map(|p| luma_bt709(*p)).collect();
    let mut out = RgbaImage::new(width, height);
    let w_usize = width as usize;
    let max_x = width as i32 - 1;
    let max_y = height as i32 - 1;
    let row_bytes = w_usize * 4;

    out.as_mut()
        .par_chunks_mut(row_bytes)
        .enumerate()
        .for_each(|(y, row)| {
            let yi = y as i32;
            let sample = |sx: i32, sy: i32| -> f32 {
                let cx = sx.clamp(0, max_x) as usize;
                let cy = sy.clamp(0, max_y) as usize;
                luma[cy * w_usize + cx]
            };

            for x in 0..width {
                let xi = x as i32;
                let tl = sample(xi - 1, yi - 1);
                let tc = sample(xi, yi - 1);
                let tr = sample(xi + 1, yi - 1);
                let ml = sample(xi - 1, yi);
                let mr = sample(xi + 1, yi);
                let bl = sample(xi - 1, yi + 1);
                let bc = sample(xi, yi + 1);
                let br = sample(xi + 1, yi + 1);

                let gx = -tl + tr - 2.0 * ml + 2.0 * mr - bl + br;
                let gy = -tl - 2.0 * tc - tr + bl + 2.0 * bc + br;
                let mag = clamp_channel(gx.hypot(gy) * 0.5);
                let val = if invert_edges { 255 - mag } else { mag };

                let idx = (x as usize) * 4;
                row[idx] = val;
                row[idx + 1] = val;
                row[idx + 2] = val;
                row[idx + 3] = src.get_pixel(x, y as u32)[3];
            }
        });

    DynamicImage::ImageRgba8(out)
}

/// Painterly oil-paint intensity-bucket mode filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OilPaint {
    pub radius: u32,
    pub levels: u8,
}

impl Filter for OilPaint {
    fn name(&self) -> &'static str {
        "Oil Paint"
    }

    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        oil_paint(image, self.radius, self.levels)
    }
}

/// Apply an intensity-bucket oil painting filter (`radius` in `1..=5`, `levels` in `4..=32`).
pub fn oil_paint(image: &DynamicImage, radius: u32, levels: u8) -> DynamicImage {
    let src = image.to_rgba8();
    let (width, height) = src.dimensions();
    if width == 0 || height == 0 {
        return DynamicImage::ImageRgba8(src);
    }

    let r = radius.clamp(1, 5) as i32;
    let lvls = levels.clamp(4, 32) as usize;
    let lvl_scale = (lvls - 1) as f32 / 255.0;

    let mut out = RgbaImage::new(width, height);
    let max_x = width as i32 - 1;
    let max_y = height as i32 - 1;
    let row_bytes = (width as usize) * 4;

    out.as_mut()
        .par_chunks_mut(row_bytes)
        .enumerate()
        .for_each(|(y, row)| {
            let yi = y as i32;
            for x in 0..width {
                let xi = x as i32;
                let mut counts = [0u32; 32];
                let mut sum_r = [0u32; 32];
                let mut sum_g = [0u32; 32];
                let mut sum_b = [0u32; 32];

                for dy in -r..=r {
                    let sy = (yi + dy).clamp(0, max_y) as u32;
                    for dx in -r..=r {
                        let sx = (xi + dx).clamp(0, max_x) as u32;
                        let p = src.get_pixel(sx, sy);
                        let bucket = ((luma_bt709(*p) * lvl_scale).round() as usize).min(lvls - 1);
                        counts[bucket] += 1;
                        sum_r[bucket] += u32::from(p[0]);
                        sum_g[bucket] += u32::from(p[1]);
                        sum_b[bucket] += u32::from(p[2]);
                    }
                }

                let mut best = 0usize;
                let mut best_count = 0u32;
                for (b, &cnt) in counts.iter().enumerate().take(lvls) {
                    if cnt > best_count {
                        best_count = cnt;
                        best = b;
                    }
                }

                let idx = (x as usize) * 4;
                if best_count > 0 {
                    let half = best_count / 2;
                    row[idx] = ((sum_r[best] + half) / best_count) as u8;
                    row[idx + 1] = ((sum_g[best] + half) / best_count) as u8;
                    row[idx + 2] = ((sum_b[best] + half) / best_count) as u8;
                }
                row[idx + 3] = src.get_pixel(x, y as u32)[3];
            }
        });

    DynamicImage::ImageRgba8(out)
}

// ── 4. Gradient & Clone Stamp Raster Primitives ──

/// Gradient geometry mode for [`draw_gradient`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientKind {
    Linear,
    Radial,
}

/// Alpha-blend a smooth linear or radial gradient from `(x0, y0)` to `(x1, y1)` over `img`.
///
/// When `mask` is provided, only pixels with non-zero mask coverage are shaded.
#[allow(clippy::too_many_arguments)]
pub fn draw_gradient(
    img: &mut RgbaImage,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    color1: Rgba<u8>,
    color2: Rgba<u8>,
    kind: GradientKind,
    mask: Option<&GrayImage>,
) {
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return;
    }

    let dx = x1 - x0;
    let dy = y1 - y0;
    let len_sq = dx * dx + dy * dy;
    let radius = len_sq.sqrt();

    for y in 0..height {
        let py = y as f32;
        for x in 0..width {
            let mask_alpha = match mask {
                Some(m) => {
                    if x < m.width() && y < m.height() {
                        let v = m.get_pixel(x, y)[0];
                        if v == 0 {
                            continue;
                        }
                        f32::from(v) / 255.0
                    } else {
                        continue;
                    }
                }
                None => 1.0,
            };

            let px = x as f32;
            let t = match kind {
                GradientKind::Linear => {
                    if len_sq <= 1e-6 {
                        0.0
                    } else {
                        (((px - x0) * dx + (py - y0) * dy) / len_sq).clamp(0.0, 1.0)
                    }
                }
                GradientKind::Radial => {
                    if radius <= 1e-6 {
                        1.0
                    } else {
                        ((px - x0).hypot(py - y0) / radius).clamp(0.0, 1.0)
                    }
                }
            };

            let mut grad_px = lerp_premultiplied(color1, color2, t);
            if mask_alpha < 1.0 {
                grad_px[3] = (f32::from(grad_px[3]) * mask_alpha)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            let dst = *img.get_pixel(x, y);
            img.put_pixel(x, y, blend_pixel(dst, grad_px));
        }
    }
}

/// Stamp a circular patch copied from `(src_x, src_y)` onto `(dst_x, dst_y)`.
///
/// Snapshots all source pixels before writing so overlapping source and destination
/// areas never feedback-smear within a single dab.
pub fn clone_stamp_dab(
    img: &mut RgbaImage,
    src_x: i32,
    src_y: i32,
    dst_x: i32,
    dst_y: i32,
    radius: f32,
    hardness: f32,
) {
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 || radius <= 0.0 {
        return;
    }

    let r = radius;
    let hardness = hardness.clamp(0.0, 1.0);
    let inner_r = r * hardness;
    let r_ceil = r.ceil() as i32;
    let w_i = width as i32;
    let h_i = height as i32;

    let side = (r_ceil * 2 + 1) as usize;
    let mut patch = Vec::with_capacity(side * side);

    for dy in -r_ceil..=r_ceil {
        let sy = src_y + dy;
        let ty = dst_y + dy;
        if sy < 0 || sy >= h_i || ty < 0 || ty >= h_i {
            continue;
        }
        for dx in -r_ceil..=r_ceil {
            let sx = src_x + dx;
            let tx = dst_x + dx;
            if sx < 0 || sx >= w_i || tx < 0 || tx >= w_i {
                continue;
            }

            let dist = (dx as f32).hypot(dy as f32);
            if dist > r {
                continue;
            }

            let weight = if hardness >= 1.0 || dist <= inner_r {
                1.0
            } else {
                1.0 - smoothstep(inner_r, r, dist)
            };

            if weight > 0.0 {
                let sampled = *img.get_pixel(sx as u32, sy as u32);
                patch.push((tx as u32, ty as u32, weight, sampled));
            }
        }
    }

    for (tx, ty, weight, sampled) in patch {
        let dst = *img.get_pixel(tx, ty);
        img.put_pixel(tx, ty, lerp_premultiplied(dst, sampled, weight));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filters::FilterPipeline;

    fn solid_rgba(w: u32, h: u32, color: [u8; 4]) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, Rgba(color)))
    }

    #[test]
    fn magic_wand_out_of_bounds_returns_zero_mask() {
        let img = solid_rgba(8, 6, [100, 150, 200, 255]);
        let mask = magic_wand_mask(&img, 8, 2, 20, true);
        assert_eq!(mask.dimensions(), (8, 6));
        assert!(mask.pixels().all(|p| p[0] == 0));

        let mask2 = magic_wand_mask(&img, 2, 10, 20, false);
        assert_eq!(mask2.dimensions(), (8, 6));
        assert!(mask2.pixels().all(|p| p[0] == 0));
    }

    #[test]
    fn magic_wand_contiguous_vs_global() {
        let mut rgba = RgbaImage::from_pixel(10, 10, Rgba([255, 255, 255, 255]));
        // Draw vertical red divider at x = 5 splitting white image into two islands
        for y in 0..10 {
            rgba.put_pixel(5, y, Rgba([255, 0, 0, 255]));
        }
        let img = DynamicImage::ImageRgba8(rgba);

        let contig = magic_wand_mask(&img, 2, 5, 5, true);
        assert_eq!(contig.get_pixel(0, 0)[0], 255);
        assert_eq!(contig.get_pixel(4, 9)[0], 255);
        assert_eq!(contig.get_pixel(5, 5)[0], 0);
        assert_eq!(contig.get_pixel(7, 5)[0], 0);

        let global = magic_wand_mask(&img, 2, 5, 5, false);
        assert_eq!(global.get_pixel(0, 0)[0], 255);
        assert_eq!(global.get_pixel(5, 5)[0], 0);
        assert_eq!(global.get_pixel(7, 5)[0], 255);
    }

    #[test]
    fn remove_background_color_edge_connected_and_feather() {
        let mut rgba = RgbaImage::from_pixel(12, 12, Rgba([255, 255, 255, 255]));
        // Bordered dark square with interior white center
        for y in 3..=8 {
            for x in 3..=8 {
                rgba.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        for y in 5..=6 {
            for x in 5..=6 {
                rgba.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        // Semi-matching fringe pixel on the perimeter
        rgba.put_pixel(0, 0, Rgba([238, 238, 238, 255]));
        let img = DynamicImage::ImageRgba8(rgba);

        // Edge-connected preserves interior white island
        let cut_edge = remove_background_color(&img, [255, 255, 255], 10.0, 30.0, true).to_rgba8();
        assert_eq!(cut_edge.get_pixel(1, 1)[3], 0);
        assert_eq!(cut_edge.get_pixel(5, 5)[3], 255);
        assert_eq!(cut_edge.get_pixel(3, 3)[3], 255);
        // Distance of [238, 238, 238] from [255, 255, 255] is ~29.4, inside (10, 40) feather zone
        let fringe_a = cut_edge.get_pixel(0, 0)[3];
        assert!(
            fringe_a > 0 && fringe_a < 255,
            "expected partial alpha, got {fringe_a}"
        );

        // Global mode removes interior white as well
        let cut_global =
            remove_background_color(&img, [255, 255, 255], 10.0, 0.0, false).to_rgba8();
        assert_eq!(cut_global.get_pixel(5, 5)[3], 0);
        assert_eq!(cut_global.get_pixel(3, 3)[3], 255);
    }

    #[test]
    fn auto_trim_transparent_and_corner_and_empty_safe() {
        // Transparent outer border around 4x3 content at (3, 2)
        let mut rgba = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        for y in 2..5 {
            for x in 3..7 {
                rgba.put_pixel(x, y, Rgba([20, 40, 60, 255]));
            }
        }
        let img = DynamicImage::ImageRgba8(rgba);
        let (trimmed, info) = auto_trim(&img, TrimMode::Transparent);
        assert_eq!(
            info,
            TrimResult {
                x: 3,
                y: 2,
                width: 4,
                height: 3
            }
        );
        assert_eq!(trimmed.dimensions(), (4, 3));

        // CornerColor mode
        let mut bg_img = RgbaImage::from_pixel(8, 8, Rgba([240, 240, 240, 255]));
        bg_img.put_pixel(2, 3, Rgba([10, 20, 30, 255]));
        bg_img.put_pixel(5, 6, Rgba([10, 20, 30, 255]));
        let (trimmed_c, info_c) =
            auto_trim(&DynamicImage::ImageRgba8(bg_img), TrimMode::CornerColor(5));
        assert_eq!(
            info_c,
            TrimResult {
                x: 2,
                y: 3,
                width: 4,
                height: 4
            }
        );
        assert_eq!(trimmed_c.dimensions(), (4, 4));

        // Fully transparent & fully uniform safe fallback
        let empty = solid_rgba(6, 5, [0, 0, 0, 0]);
        let (res_empty, info_empty) = auto_trim(&empty, TrimMode::Transparent);
        assert_eq!(res_empty.dimensions(), (6, 5));
        assert_eq!(info_empty.width, 6);

        let uniform = solid_rgba(4, 4, [50, 60, 70, 255]);
        let (res_uni, info_uni) = auto_trim(&uniform, TrimMode::CornerColor(0));
        assert_eq!(res_uni.dimensions(), (4, 4));
        assert_eq!(info_uni.height, 4);
    }

    #[test]
    fn round_corners_cuts_corners_and_keeps_center_and_edges() {
        let img = solid_rgba(40, 40, [80, 160, 240, 255]);
        let rounded = round_corners(&img, 10.0).to_rgba8();
        assert_eq!(rounded.get_pixel(0, 0)[3], 0);
        assert_eq!(rounded.get_pixel(39, 39)[3], 0);
        assert_eq!(rounded.get_pixel(20, 0)[3], 255);
        assert_eq!(rounded.get_pixel(0, 20)[3], 255);
        assert_eq!(rounded.get_pixel(20, 20)[3], 255);
    }

    #[test]
    fn add_drop_shadow_expands_and_composites() {
        let img = solid_rgba(20, 16, [255, 0, 0, 255]);
        let shadowed = add_drop_shadow(&img, 4, 6, 3.0, [0, 0, 0, 180], 10).to_rgba8();
        assert!(shadowed.width() > 20 + 20);
        assert!(shadowed.height() > 16 + 20);

        // Original solid red rectangle should be intact underneath center offset
        let blur_margin = (3.0f32 * 3.0).ceil() as u32;
        let orig_x = 10 + blur_margin;
        let orig_y = 10 + blur_margin;
        assert_eq!(
            *shadowed.get_pixel(orig_x + 5, orig_y + 5),
            Rgba([255, 0, 0, 255])
        );
        // Below original box (offset_y = 6), shadow pixels should have non-zero alpha and dark RGB
        let below = *shadowed.get_pixel(orig_x + 5, orig_y + 18);
        assert!(below[3] > 0, "expected shadow below card");
        assert_eq!(below[0], 0);
    }

    #[test]
    fn add_border_frame_renders_background_border_and_shadow() {
        let img = solid_rgba(30, 20, [20, 100, 200, 255]);
        let opts = FrameCardOptions {
            padding: 12,
            corner_radius: 6.0,
            shadow_sigma: 4.0,
            shadow_offset_x: 0,
            shadow_offset_y: 4,
            background_color: [250, 250, 252, 255],
            border_width: 3,
            border_color: [255, 255, 255, 255],
        };
        let framed = add_border_frame(&img, opts).to_rgba8();
        assert!(framed.width() > 30 + 2 * (12 + 3));
        assert!(framed.height() > 20 + 2 * (12 + 3));
        // Outer corner shows background color
        assert_eq!(*framed.get_pixel(0, 0), Rgba([250, 250, 252, 255]));
        // Center shows original color
        assert_eq!(
            *framed.get_pixel(framed.width() / 2, framed.height() / 2 - 2),
            Rgba([20, 100, 200, 255])
        );
    }

    #[test]
    fn tilt_shift_keeps_focus_band_sharp_and_blurs_edges() {
        let mut rgba = RgbaImage::from_pixel(20, 40, Rgba([0, 0, 0, 180]));
        // Put high-frequency checkerboard across all rows
        for y in 0..40 {
            for x in 0..20 {
                if (x + y) % 2 == 0 {
                    rgba.put_pixel(x, y, Rgba([255, 255, 255, 180]));
                }
            }
        }
        let img = DynamicImage::ImageRgba8(rgba);
        let shifted = tilt_shift(&img, 0.5, 0.2, 3.0).to_rgba8();

        // Center focus row stays sharp (0 or 255) and preserves alpha 180
        let center_px = *shifted.get_pixel(10, 20);
        assert!(center_px[0] == 0 || center_px[0] == 255);
        assert_eq!(center_px[3], 180);

        // Top row should be blurred toward gray (~128)
        let top_px = *shifted.get_pixel(10, 0);
        assert!(
            top_px[0] > 40 && top_px[0] < 215,
            "expected blurred top pixel, got {:?}",
            top_px
        );
        assert_eq!(top_px[3], 180);
    }

    #[test]
    fn pixelate_averages_blocks_and_preserves_dimensions() {
        let mut rgba = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 255]));
        rgba.put_pixel(0, 0, Rgba([200, 100, 0, 200]));
        rgba.put_pixel(1, 0, Rgba([0, 100, 200, 200]));
        rgba.put_pixel(0, 1, Rgba([200, 100, 0, 200]));
        rgba.put_pixel(1, 1, Rgba([0, 100, 200, 200]));

        let pix = pixelate(&DynamicImage::ImageRgba8(rgba), 2).to_rgba8();
        assert_eq!(pix.dimensions(), (4, 4));
        assert_eq!(*pix.get_pixel(0, 0), Rgba([100, 100, 100, 200]));
        assert_eq!(*pix.get_pixel(1, 1), Rgba([100, 100, 100, 200]));
        assert_eq!(*pix.get_pixel(3, 3), Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn film_grain_is_deterministic_and_preserves_alpha() {
        let img = solid_rgba(16, 16, [120, 130, 140, 177]);
        let g1 = film_grain(&img, 0.5).to_rgba8();
        let g2 = film_grain(&img, 0.5).to_rgba8();
        assert_eq!(g1.as_raw(), g2.as_raw());
        assert!(g1.pixels().all(|p| p[3] == 177));
        assert!(g1.pixels().any(|p| p[0] != 120));

        // Zero strength is identity
        let g0 = film_grain(&img, 0.0).to_rgba8();
        assert!(g0.pixels().all(|p| p.0 == [120, 130, 140, 177]));
    }

    #[test]
    fn emboss_highlights_edges_and_preserves_alpha() {
        let mut rgba = RgbaImage::from_pixel(10, 10, Rgba([40, 40, 40, 210]));
        for y in 0..10 {
            for x in 5..10 {
                rgba.put_pixel(x, y, Rgba([220, 220, 220, 210]));
            }
        }
        let img = DynamicImage::ImageRgba8(rgba);
        let embossed = emboss(&img, 1.0).to_rgba8();
        assert!(embossed.pixels().all(|p| p[3] == 210));
        // Flat region settles near mid-gray 128
        assert_eq!(embossed.get_pixel(1, 5)[0], 128);
        // Step transition creates strong relief away from 128
        assert!(embossed.get_pixel(4, 5)[0] > 180);
    }

    #[test]
    fn edge_detect_finds_edges_and_inverts_for_sketch() {
        let mut rgba = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 150]));
        for y in 0..10 {
            for x in 5..10 {
                rgba.put_pixel(x, y, Rgba([255, 255, 255, 150]));
            }
        }
        let img = DynamicImage::ImageRgba8(rgba);
        let edges = edge_detect(&img, false).to_rgba8();
        assert!(edges.pixels().all(|p| p[3] == 150));
        assert_eq!(edges.get_pixel(1, 5)[0], 0);
        assert_eq!(edges.get_pixel(4, 5)[0], 255);

        let sketch = edge_detect(&img, true).to_rgba8();
        assert!(sketch.pixels().all(|p| p[3] == 150));
        assert_eq!(sketch.get_pixel(1, 5)[0], 255);
        assert_eq!(sketch.get_pixel(4, 5)[0], 0);
    }

    #[test]
    fn oil_paint_clamps_params_and_preserves_alpha() {
        let mut rgba = RgbaImage::from_pixel(12, 12, Rgba([30, 60, 90, 190]));
        rgba.put_pixel(6, 6, Rgba([250, 250, 250, 190])); // single outlier pixel smoothed out
        let img = DynamicImage::ImageRgba8(rgba);
        let painted = oil_paint(&img, 10, 1).to_rgba8();
        assert_eq!(painted.dimensions(), (12, 12));
        assert!(painted.pixels().all(|p| p[3] == 190));
        assert_eq!(*painted.get_pixel(6, 6), Rgba([30, 60, 90, 190]));
    }

    #[test]
    fn filter_pipeline_runs_pro_effect_structs() {
        let img = solid_rgba(16, 16, [100, 120, 140, 255]);
        let mut pipeline = FilterPipeline::new();
        pipeline.push(Box::new(Pixelate { block_size: 2 }));
        pipeline.push(Box::new(TiltShift {
            focus_y: 0.5,
            band_width: 0.3,
            blur_sigma: 1.5,
        }));
        pipeline.push(Box::new(FilmGrain { strength: 0.2 }));
        pipeline.push(Box::new(Emboss { strength: 0.5 }));
        pipeline.push(Box::new(EdgeDetect { invert_edges: true }));
        pipeline.push(Box::new(OilPaint {
            radius: 1,
            levels: 8,
        }));
        let out = pipeline.apply_all(&img);
        assert_eq!(out.dimensions(), (16, 16));
    }

    #[test]
    fn draw_gradient_linear_radial_and_masked() {
        let mut img = RgbaImage::from_pixel(11, 5, Rgba([255, 255, 255, 255]));
        let red = Rgba([255, 0, 0, 255]);
        let blue = Rgba([0, 0, 255, 255]);
        draw_gradient(
            &mut img,
            0.0,
            2.0,
            10.0,
            2.0,
            red,
            blue,
            GradientKind::Linear,
            None,
        );
        assert_eq!(*img.get_pixel(0, 2), red);
        assert_eq!(*img.get_pixel(10, 2), blue);
        let mid = *img.get_pixel(5, 2);
        assert!((mid[0] as i32 - 128).abs() <= 1);
        assert!((mid[2] as i32 - 128).abs() <= 1);

        // Radial gradient with mask restricting to center pixel
        let mut img2 = RgbaImage::from_pixel(9, 9, Rgba([10, 20, 30, 255]));
        let mut mask = GrayImage::new(9, 9);
        mask.put_pixel(4, 4, Luma([255]));
        draw_gradient(
            &mut img2,
            4.0,
            4.0,
            8.0,
            4.0,
            Rgba([0, 255, 0, 255]),
            Rgba([0, 0, 0, 0]),
            GradientKind::Radial,
            Some(&mask),
        );
        assert_eq!(*img2.get_pixel(4, 4), Rgba([0, 255, 0, 255]));
        assert_eq!(*img2.get_pixel(0, 0), Rgba([10, 20, 30, 255]));
    }

    #[test]
    fn clone_stamp_dab_snapshots_before_writing_on_overlap() {
        // Vertical stripe pattern: x=0..=2 is red, x=3..=8 is black
        let mut img = RgbaImage::from_pixel(10, 5, Rgba([0, 0, 0, 255]));
        for y in 0..5 {
            for x in 0..=2 {
                img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }
        // Source center (2, 2), dest center (4, 2), radius 2.0, hardness 1.0 -> overlapping circles!
        // Source patch covers x=0..=4 (x=0..=2 red, x=3..=4 black).
        // If feedback smeared, x=4 reading from x=2 after writing or vice versa would corrupt results.
        clone_stamp_dab(&mut img, 2, 2, 4, 2, 2.0, 1.0);

        // Dest x=2..=4 (shifted by +2 from src x=0..=2) should be red
        assert_eq!(*img.get_pixel(2, 2), Rgba([255, 0, 0, 255]));
        assert_eq!(*img.get_pixel(3, 2), Rgba([255, 0, 0, 255]));
        assert_eq!(*img.get_pixel(4, 2), Rgba([255, 0, 0, 255]));
        // Dest x=5..=6 (shifted by +2 from src x=3..=4 which were originally black) must remain black!
        assert_eq!(*img.get_pixel(5, 2), Rgba([0, 0, 0, 255]));
        assert_eq!(*img.get_pixel(6, 2), Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn clone_stamp_dab_soft_edge_and_bounds_safety() {
        let mut img = RgbaImage::from_pixel(15, 15, Rgba([0, 0, 200, 255]));
        for y in 0..7 {
            for x in 0..7 {
                img.put_pixel(x, y, Rgba([200, 0, 0, 255]));
            }
        }
        // Soft dab with hardness = 0.0
        clone_stamp_dab(&mut img, 3, 3, 10, 10, 3.0, 0.0);
        // Exact center is 100% source red
        assert_eq!(*img.get_pixel(10, 10), Rgba([200, 0, 0, 255]));
        // Mid-radius is blended between red and blue
        let edge_px = *img.get_pixel(12, 10);
        assert!(edge_px[0] > 0 && edge_px[0] < 200);
        assert!(edge_px[2] > 0 && edge_px[2] < 200);
        // Outside radius remains blue
        assert_eq!(*img.get_pixel(14, 14), Rgba([0, 0, 200, 255]));

        // Out-of-bounds and zero-radius safety
        clone_stamp_dab(&mut img, -5, -5, 1, 1, 2.0, 1.0);
        clone_stamp_dab(&mut img, 1, 1, 1, 1, 0.0, 1.0);
    }

    #[test]
    fn draw_gradient_premultiplied_alpha_and_degenerate_safe() {
        // White backdrop + red-to-transparent gradient should not introduce black mud at midpoint
        let mut img = RgbaImage::from_pixel(11, 1, Rgba([255, 255, 255, 255]));
        draw_gradient(
            &mut img,
            0.0,
            0.0,
            10.0,
            0.0,
            Rgba([255, 0, 0, 255]),
            Rgba([0, 0, 0, 0]),
            GradientKind::Linear,
            None,
        );
        let mid = *img.get_pixel(5, 0);
        assert_eq!(mid[0], 255);
        assert!((mid[1] as i32 - 128).abs() <= 2);
        assert!((mid[2] as i32 - 128).abs() <= 2);

        // Degenerate zero-length gradients must not NaN/panic
        draw_gradient(
            &mut img,
            3.0,
            0.0,
            3.0,
            0.0,
            Rgba([0, 255, 0, 255]),
            Rgba([0, 0, 255, 255]),
            GradientKind::Radial,
            None,
        );
    }

    #[test]
    fn add_border_frame_without_shadow_or_border() {
        let img = solid_rgba(10, 8, [50, 100, 150, 255]);
        let opts = FrameCardOptions {
            padding: 6,
            corner_radius: 0.0,
            shadow_sigma: 0.0,
            shadow_offset_x: 0,
            shadow_offset_y: 0,
            background_color: [10, 20, 30, 255],
            border_width: 0,
            border_color: [255, 255, 255, 255],
        };
        let framed = add_border_frame(&img, opts).to_rgba8();
        assert_eq!(framed.dimensions(), (22, 20));
        assert_eq!(*framed.get_pixel(0, 0), Rgba([10, 20, 30, 255]));
        assert_eq!(*framed.get_pixel(6, 6), Rgba([50, 100, 150, 255]));
    }

    #[test]
    fn serde_roundtrips_and_empty_image_safety() {
        let opts = FrameCardOptions::default();
        let json = serde_json::to_string(&opts).unwrap();
        let back: FrameCardOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, back);

        let trim = TrimResult {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        };
        let mode = TrimMode::CornerColor(12);
        assert_eq!(
            serde_json::from_str::<TrimResult>(&serde_json::to_string(&trim).unwrap()).unwrap(),
            trim
        );
        assert_eq!(
            serde_json::from_str::<TrimMode>(&serde_json::to_string(&mode).unwrap()).unwrap(),
            mode
        );

        let tiny_cases = [
            solid_rgba(0, 0, [0, 0, 0, 0]),
            solid_rgba(1, 1, [42, 84, 126, 200]),
        ];
        for img in &tiny_cases {
            let _ = magic_wand_mask(img, 0, 0, 10, true);
            let _ = magic_wand_mask(img, 0, 0, 10, false);
            let _ = remove_background_color(img, [0, 0, 0], 10.0, 5.0, true);
            let _ = auto_trim(img, TrimMode::Transparent);
            let _ = round_corners(img, 5.0);
            let _ = add_drop_shadow(img, 2, 2, 2.0, [0, 0, 0, 128], 4);
            let _ = add_border_frame(img, FrameCardOptions::default());
            let _ = tilt_shift(img, 0.5, 0.2, 2.0);
            let _ = pixelate(img, 4);
            let _ = film_grain(img, 0.4);
            let _ = emboss(img, 1.0);
            let _ = edge_detect(img, false);
            let _ = oil_paint(img, 2, 8);
        }
    }
}
