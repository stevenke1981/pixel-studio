//! Procedural washi-tape patterns rendered into an editable transparent layer.

use image::{Rgba, RgbaImage};

pub fn draw_segment(
    image: &mut RgbaImage,
    from: (f32, f32),
    to: (f32, f32),
    width: f32,
    pattern: usize,
) {
    let radius = width.max(1.0) / 2.0;
    let x0 = (from.0.min(to.0) - radius).floor().max(0.0) as u32;
    let y0 = (from.1.min(to.1) - radius).floor().max(0.0) as u32;
    let x1 = ((from.0.max(to.0) + radius).ceil().max(0.0) as u32).min(image.width());
    let y1 = ((from.1.max(to.1) + radius).ceil().max(0.0) as u32).min(image.height());
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length2 = dx * dx + dy * dy;
    let palettes = [
        ([247, 166, 184], [255, 240, 235]),
        ([154, 205, 235], [250, 252, 255]),
        ([171, 219, 199], [237, 252, 237]),
        ([248, 214, 115], [255, 248, 210]),
        ([194, 168, 225], [255, 226, 239]),
        ([244, 161, 145], [255, 235, 222]),
        ([142, 181, 139], [231, 246, 204]),
        ([119, 195, 208], [223, 249, 249]),
        ([244, 169, 177], [253, 231, 158]),
        ([203, 173, 133], [255, 237, 202]),
    ];
    let pattern = pattern.min(9);
    for y in y0..y1 {
        for x in x0..x1 {
            let (px, py) = (x as f32 + 0.5 - from.0, y as f32 + 0.5 - from.1);
            let t = if length2 > 0.0 {
                ((px * dx + py * dy) / length2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            if (px - t * dx).hypot(py - t * dy) > radius {
                continue;
            }
            let (u, v) = (x as i32 % 24, y as i32 % 24);
            let motif = match pattern {
                0 => (x + y) % 24 < 8,
                1 => (u - 12).pow(2) + (v - 12).pow(2) < 20,
                2 => u < 5 || v < 5,
                3 => {
                    (u - 12).abs() + (v - 12).abs() < 6
                        || ((u - 12).abs() < 2 && (v - 12).abs() < 10)
                        || ((v - 12).abs() < 2 && (u - 12).abs() < 10)
                }
                4 => {
                    (u - 9).pow(2) + (v - 9).pow(2) < 17
                        || (u - 15).pow(2) + (v - 9).pow(2) < 17
                        || ((v - 9) >= 0 && (v - 9) < 9 && (u - 12).abs() < 9 - (v - 9))
                }
                5 => (v - 12 - ((u as f32 / 4.0).sin() * 5.0) as i32).abs() < 2,
                6 => (u - 12).pow(2) * 2 + (v - 12).pow(2) < 45 && (u + v) % 4 != 0,
                7 => (u - 12).abs() + (v - 12).abs() < 8,
                8 => (x + y) % 30 < 15,
                _ => {
                    ((u - 6).pow(2) + (v - 7).pow(2) < 6) || ((u - 18).pow(2) + (v - 17).pow(2) < 6)
                }
            };
            let color = if motif {
                palettes[pattern].1
            } else {
                palettes[pattern].0
            };
            image.put_pixel(x, y, Rgba([color[0], color[1], color[2], 230]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_ten_patterns_are_distinct_and_draw_continuously() {
        let mut patterns = Vec::new();
        for pattern in 0..10 {
            let mut image = RgbaImage::new(96, 48);
            draw_segment(&mut image, (8.0, 24.0), (88.0, 24.0), 20.0, pattern);
            assert!((8..88).all(|x| image.get_pixel(x, 24).0[3] == 230));
            assert_eq!(image.get_pixel(48, 2).0[3], 0);
            assert!(!patterns.contains(&image));
            patterns.push(image);
        }
    }
}
