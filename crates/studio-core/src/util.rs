/// Clamp a float value to valid 8-bit channel range [0, 255].
pub fn clamp_channel(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

/// Linear interpolation between two channel values.
pub fn lerp_channel(from: f32, to: f32, t: f32) -> u8 {
    clamp_channel(from + (to - from) * t)
}

/// Convert RGB to HSL color space.
/// Returns (hue [0..1), saturation [0..1], lightness [0..1]).
pub fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = f32::from(r) / 255.0;
    let g = f32::from(g) / 255.0;
    let b = f32::from(b) / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < f32::EPSILON {
        let mut hue = (g - b) / d;
        if g < b {
            hue += 6.0;
        }
        hue
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;

    (h.rem_euclid(1.0), s, l)
}

/// Convert HSL to RGB color space.
pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    if s.abs() < f32::EPSILON {
        let v = clamp_channel(l * 255.0);
        return (v, v, v);
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;

    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);
    (
        clamp_channel(r * 255.0),
        clamp_channel(g * 255.0),
        clamp_channel(b * 255.0),
    )
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 1.0 / 2.0 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_channel_clamps_low() {
        assert_eq!(clamp_channel(-10.0), 0);
    }

    #[test]
    fn clamp_channel_clamps_high() {
        assert_eq!(clamp_channel(300.0), 255);
    }

    #[test]
    fn clamp_channel_passes_through() {
        assert_eq!(clamp_channel(127.0), 127);
    }

    #[test]
    fn rgb_hsl_roundtrip() {
        let cases = [
            (255u8, 0u8, 0u8),
            (0, 255, 0),
            (0, 0, 255),
            (128, 128, 128),
            (200, 100, 50),
        ];
        for (r, g, b) in cases {
            let (h, s, l) = rgb_to_hsl(r, g, b);
            let (rr, gg, bb) = hsl_to_rgb(h, s, l);
            assert_eq!(
                (rr, gg, bb),
                (r, g, b),
                "roundtrip failed for ({r},{g},{b}) -> hsl({h:.4},{s:.4},{l:.4})"
            );
        }
    }
}
