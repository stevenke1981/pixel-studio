//! Professional tonal grading, histogram analysis, and color looks.
//!
//! Provides UI-independent pixel processing primitives and `Filter`
//! implementations for levels, shadows/highlights recovery, vibrance, clarity,
//! channel color balance, binary threshold, histogram computation/rendering, and
//! curated color grading looks.

use std::f32::consts::PI;

use image::imageops::blur as gaussian_blur_rgba;
use image::{DynamicImage, Rgba, RgbaImage};
use rayon::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::filters::Filter;
use crate::util::{clamp_channel, hsl_to_rgb, rgb_to_hsl};

const HIST_BINS: usize = 256;
const PARALLEL_BYTE_THRESHOLD: usize = 262_144;
const HIST_BG: Rgba<u8> = Rgba([0x18, 0x1E, 0x26, 255]);
const HIST_GRID: Rgba<u8> = Rgba([0x22, 0x2A, 0x35, 255]);

mod serde_hist_bins {
    use super::*;

    pub fn serialize<S>(bins: &[u32; HIST_BINS], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        bins.as_slice().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u32; HIST_BINS], D::Error>
    where
        D: Deserializer<'de>,
    {
        let values = Vec::<u32>::deserialize(deserializer)?;
        values.try_into().map_err(|vec: Vec<u32>| {
            serde::de::Error::invalid_length(vec.len(), &"exactly 256 histogram bins")
        })
    }
}

/// Per-channel and BT.709 luminance histogram with summary tonal statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistogramData {
    #[serde(with = "serde_hist_bins")]
    pub r: [u32; 256],
    #[serde(with = "serde_hist_bins")]
    pub g: [u32; 256],
    #[serde(with = "serde_hist_bins")]
    pub b: [u32; 256],
    #[serde(with = "serde_hist_bins")]
    pub luma: [u32; 256],
    pub pixel_count: u64,
    pub mean_luma: f32,
    pub std_dev_luma: f32,
    pub shadow_clip_pct: f32,
    pub highlight_clip_pct: f32,
}

impl Default for HistogramData {
    fn default() -> Self {
        Self {
            r: [0; 256],
            g: [0; 256],
            b: [0; 256],
            luma: [0; 256],
            pixel_count: 0,
            mean_luma: 0.0,
            std_dev_luma: 0.0,
            shadow_clip_pct: 0.0,
            highlight_clip_pct: 0.0,
        }
    }
}

/// Compute RGB and BT.709 luminance histograms plus clipping and spread metrics.
pub fn compute_histogram(image: &DynamicImage) -> HistogramData {
    let rgba = image.to_rgba8();
    let mut hist = HistogramData::default();
    let total = u64::from(rgba.width()) * u64::from(rgba.height());
    if total == 0 {
        return hist;
    }

    for pixel in rgba.pixels() {
        let r = pixel[0];
        let g = pixel[1];
        let b = pixel[2];
        let y = bt709_luma_u8(r, g, b);
        hist.r[usize::from(r)] += 1;
        hist.g[usize::from(g)] += 1;
        hist.b[usize::from(b)] += 1;
        hist.luma[usize::from(y)] += 1;
    }

    hist.pixel_count = total;

    let total_f64 = total as f64;
    let sum_luma: f64 = hist
        .luma
        .iter()
        .enumerate()
        .map(|(bin, &count)| bin as f64 * f64::from(count))
        .sum();
    let mean = sum_luma / total_f64;

    let variance: f64 = hist
        .luma
        .iter()
        .enumerate()
        .map(|(bin, &count)| {
            let diff = bin as f64 - mean;
            diff * diff * f64::from(count)
        })
        .sum::<f64>()
        / total_f64;

    let shadow_clip: u64 = hist.luma[..=2].iter().map(|&c| u64::from(c)).sum();
    let highlight_clip: u64 = hist.luma[253..].iter().map(|&c| u64::from(c)).sum();

    hist.mean_luma = mean as f32;
    hist.std_dev_luma = variance.max(0.0).sqrt() as f32;
    hist.shadow_clip_pct = (shadow_clip as f64 / total_f64 * 100.0) as f32;
    hist.highlight_clip_pct = (highlight_clip as f64 / total_f64 * 100.0) as f32;
    hist
}

/// Render a histogram overlay graphic onto a `#181E26` dark slate canvas.
///
/// Output dimensions are clamped to at least `64x32`. Renders translucent RGB
/// fills, channel outlines, and a crisp white luminance curve.
pub fn render_histogram_image(hist: &HistogramData, width: u32, height: u32) -> DynamicImage {
    let w = width.max(64);
    let h = height.max(32);
    let mut canvas = RgbaImage::from_pixel(w, h, HIST_BG);

    // Subtle interior quartile guides leaving borders untouched.
    for &gx in &[w / 4, w / 2, (w * 3) / 4] {
        for y in 1..h - 1 {
            canvas.put_pixel(gx, y, HIST_GRID);
        }
    }
    for &gy in &[h / 4, h / 2, (h * 3) / 4] {
        for x in 1..w - 1 {
            canvas.put_pixel(x, gy, HIST_GRID);
        }
    }

    let max_bin = hist
        .r
        .iter()
        .chain(hist.g.iter())
        .chain(hist.b.iter())
        .chain(hist.luma.iter())
        .copied()
        .max()
        .unwrap_or(0);

    if max_bin == 0 {
        return DynamicImage::ImageRgba8(canvas);
    }

    let max_f = max_bin as f32;
    let usable_h = (h.saturating_sub(2)) as f32;

    let mut r_ys = Vec::with_capacity(w as usize);
    let mut g_ys = Vec::with_capacity(w as usize);
    let mut b_ys = Vec::with_capacity(w as usize);
    let mut l_ys = Vec::with_capacity(w as usize);

    for x in 0..w {
        let rv = sample_histogram_bin(&hist.r, x, w) / max_f;
        let gv = sample_histogram_bin(&hist.g, x, w) / max_f;
        let bv = sample_histogram_bin(&hist.b, x, w) / max_f;
        let lv = sample_histogram_bin(&hist.luma, x, w) / max_f;

        let ry = bin_value_to_y(rv, h, usable_h);
        let gy = bin_value_to_y(gv, h, usable_h);
        let by = bin_value_to_y(bv, h, usable_h);
        let ly = bin_value_to_y(lv, h, usable_h);

        if rv > 0.0 {
            add_translucent_tint(&mut canvas, x, ry, h, [90, 14, 14]);
        }
        if gv > 0.0 {
            add_translucent_tint(&mut canvas, x, gy, h, [14, 90, 24]);
        }
        if bv > 0.0 {
            add_translucent_tint(&mut canvas, x, by, h, [14, 52, 102]);
        }

        r_ys.push((rv > 0.0, ry));
        g_ys.push((gv > 0.0, gy));
        b_ys.push((bv > 0.0, by));
        l_ys.push((lv > 0.0, ly));
    }

    draw_curve_outline(&mut canvas, &r_ys, Rgba([238, 86, 86, 200]));
    draw_curve_outline(&mut canvas, &g_ys, Rgba([78, 218, 114, 200]));
    draw_curve_outline(&mut canvas, &b_ys, Rgba([82, 152, 246, 200]));
    draw_curve_outline(&mut canvas, &l_ys, Rgba([245, 248, 252, 240]));

    DynamicImage::ImageRgba8(canvas)
}

/// Input/output levels and midpoint gamma adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LevelsAdjust {
    pub in_black: u8,
    pub gamma: f32,
    pub in_white: u8,
    pub out_black: u8,
    pub out_white: u8,
}

impl Default for LevelsAdjust {
    fn default() -> Self {
        Self {
            in_black: 0,
            gamma: 1.0,
            in_white: 255,
            out_black: 0,
            out_white: 255,
        }
    }
}

impl Filter for LevelsAdjust {
    fn name(&self) -> &'static str {
        "Levels"
    }

    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        adjust_levels(
            image,
            self.in_black,
            self.gamma,
            self.in_white,
            self.out_black,
            self.out_white,
        )
    }
}

/// Remap tonal levels using a 256-entry LUT while preserving alpha.
///
/// Ensures `in_white > in_black` safely and clamps `gamma` to `0.1..=5.0`.
pub fn adjust_levels(
    image: &DynamicImage,
    in_black: u8,
    gamma: f32,
    in_white: u8,
    out_black: u8,
    out_white: u8,
) -> DynamicImage {
    let safe_gamma = sanitize_finite(gamma, 1.0).clamp(0.1, 5.0);
    if in_black == 0
        && in_white == 255
        && out_black == 0
        && out_white == 255
        && (safe_gamma - 1.0).abs() < f32::EPSILON
    {
        return image.clone();
    }

    let lut = build_levels_lut(in_black, safe_gamma, in_white, out_black, out_white);
    map_rgb_preserving_alpha(image, |[r, g, b]| {
        [
            lut[usize::from(r)],
            lut[usize::from(g)],
            lut[usize::from(b)],
        ]
    })
}

/// Luminance-weighted shadow lift and highlight recovery filter.
///
/// Positive `shadows` lifts dark tones; negative deepens them.
/// Negative `highlights` recovers (darkens) bright tones; positive brightens them.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ShadowsHighlights {
    pub shadows: f32,
    pub highlights: f32,
}

impl Filter for ShadowsHighlights {
    fn name(&self) -> &'static str {
        "Shadows/Highlights"
    }

    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        adjust_shadows_highlights(image, self.shadows, self.highlights)
    }
}

/// Apply smooth luminance-weighted shadow and highlight adjustments (`-100..=100`).
///
/// Preserves color ratios and original alpha.
pub fn adjust_shadows_highlights(
    image: &DynamicImage,
    shadows: f32,
    highlights: f32,
) -> DynamicImage {
    let s = sanitize_finite(shadows, 0.0).clamp(-100.0, 100.0) / 100.0;
    let h = sanitize_finite(highlights, 0.0).clamp(-100.0, 100.0) / 100.0;
    if s.abs() < f32::EPSILON && h.abs() < f32::EPSILON {
        return image.clone();
    }

    map_rgb_preserving_alpha(image, |[r, g, b]| {
        let rf = f32::from(r) / 255.0;
        let gf = f32::from(g) / 255.0;
        let bf = f32::from(b) / 255.0;

        let luma = (0.2126 * rf + 0.7152 * gf + 0.0722 * bf).clamp(0.0, 1.0);
        let mut new_luma = luma;

        if s >= 0.0 {
            let w_s = (1.0 - new_luma).powi(2);
            new_luma += s * 0.30 * w_s;
        } else {
            let w_s = (1.0 - new_luma).powi(2);
            new_luma *= 1.0 + s * 0.65 * w_s;
        }

        if h <= 0.0 {
            let w_h = new_luma.powi(2);
            new_luma += h * 0.28 * w_h;
        } else {
            let w_h = new_luma.powi(2);
            new_luma += h * 0.30 * w_h * (1.0 - new_luma);
        }

        new_luma = new_luma.clamp(0.0, 1.0);
        rescale_rgb_luma(rf, gf, bf, luma, new_luma)
    })
}

/// Smart saturation adjustment that prioritizes muted tones and protects skin hues.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct VibranceAdjust {
    pub amount: f32,
}

impl Filter for VibranceAdjust {
    fn name(&self) -> &'static str {
        "Vibrance"
    }

    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        adjust_vibrance(image, self.amount)
    }
}

/// Adjust vibrance (`-100..=100`), protecting skin-tone hues (`~15..55°`) when boosting.
pub fn adjust_vibrance(image: &DynamicImage, amount: f32) -> DynamicImage {
    let amount = sanitize_finite(amount, 0.0).clamp(-100.0, 100.0);
    if amount.abs() < f32::EPSILON {
        return image.clone();
    }

    let strength = amount / 100.0;
    map_rgb_preserving_alpha(image, |[r, g, b]| {
        let (h, s, l) = rgb_to_hsl(r, g, b);
        if s <= f32::EPSILON {
            return [r, g, b];
        }

        let new_s = if strength >= 0.0 {
            let skin_protect = skin_tone_protection(h * 360.0);
            let gate = (s / 0.04).min(1.0);
            let delta = strength * 0.45 * skin_protect * gate * (1.0 - s).powi(2);
            (s + delta).clamp(0.0, 1.0)
        } else {
            (s * (1.0 + strength)).clamp(0.0, 1.0)
        };

        let (nr, ng, nb) = hsl_to_rgb(h, new_s, l);
        [nr, ng, nb]
    })
}

/// Midtone-weighted local micro-contrast adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ClarityAdjust {
    pub amount: f32,
}

impl Filter for ClarityAdjust {
    fn name(&self) -> &'static str {
        "Clarity"
    }

    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        adjust_clarity(image, self.amount)
    }
}

/// Adjust local midtone contrast (`-100..=100`) via Gaussian blur difference (`sigma = 2.5`).
pub fn adjust_clarity(image: &DynamicImage, amount: f32) -> DynamicImage {
    let amount = sanitize_finite(amount, 0.0).clamp(-100.0, 100.0);
    if amount.abs() < f32::EPSILON {
        return image.clone();
    }

    let mut rgba = image.to_rgba8();
    if rgba.width() == 0 || rgba.height() == 0 {
        return image.clone();
    }

    let blurred = gaussian_blur_rgba(&rgba, 2.5);
    let strength = if amount >= 0.0 {
        (amount / 100.0) * 1.25
    } else {
        (amount / 100.0) * 0.95
    };

    let orig_raw = rgba.as_mut();
    let blur_raw = blurred.as_raw();

    let apply_px = |orig: &mut [u8], blur: &[u8]| {
        let ol = bt709_luma_f32(orig[0], orig[1], orig[2]) / 255.0;
        let bl = bt709_luma_f32(blur[0], blur[1], blur[2]) / 255.0;
        let avg_luma = (0.5 * (ol + bl)).clamp(0.0, 1.0);
        let midtone_weight = 0.15 + 0.85 * (4.0 * avg_luma * (1.0 - avg_luma));
        let factor = strength * midtone_weight;

        for c in 0..3 {
            let o = f32::from(orig[c]);
            let b = f32::from(blur[c]);
            orig[c] = clamp_channel(o + (o - b) * factor);
        }
    };

    if orig_raw.len() >= PARALLEL_BYTE_THRESHOLD {
        orig_raw
            .par_chunks_exact_mut(4)
            .zip(blur_raw.par_chunks_exact(4))
            .for_each(|(orig, blur)| apply_px(orig, blur));
    } else {
        for (orig, blur) in orig_raw
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(blur_raw.as_chunks::<4>().0.iter())
        {
            apply_px(orig, blur);
        }
    }

    DynamicImage::ImageRgba8(rgba)
}

/// Independent RGB channel balance shift (`-100..=100` per channel).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ColorBalance {
    pub r_shift: f32,
    pub g_shift: f32,
    pub b_shift: f32,
}

impl Filter for ColorBalance {
    fn name(&self) -> &'static str {
        "Color Balance"
    }

    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        adjust_color_balance(image, self.r_shift, self.g_shift, self.b_shift)
    }
}

/// Shift red, green, and blue channels (`-100..=100` each) while preserving alpha.
pub fn adjust_color_balance(
    image: &DynamicImage,
    r_shift: f32,
    g_shift: f32,
    b_shift: f32,
) -> DynamicImage {
    let rs = sanitize_finite(r_shift, 0.0).clamp(-100.0, 100.0);
    let gs = sanitize_finite(g_shift, 0.0).clamp(-100.0, 100.0);
    let bs = sanitize_finite(b_shift, 0.0).clamp(-100.0, 100.0);
    if rs.abs() < f32::EPSILON && gs.abs() < f32::EPSILON && bs.abs() < f32::EPSILON {
        return image.clone();
    }

    let r_lut = build_shift_lut(rs * 1.275);
    let g_lut = build_shift_lut(gs * 1.275);
    let b_lut = build_shift_lut(bs * 1.275);

    map_rgb_preserving_alpha(image, |[r, g, b]| {
        [
            r_lut[usize::from(r)],
            g_lut[usize::from(g)],
            b_lut[usize::from(b)],
        ]
    })
}

/// Binary black-and-white luminance threshold filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Threshold {
    pub level: u8,
}

impl Default for Threshold {
    fn default() -> Self {
        Self { level: 128 }
    }
}

impl Filter for Threshold {
    fn name(&self) -> &'static str {
        "Threshold"
    }

    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        threshold(image, self.level)
    }
}

/// Convert image to binary black/white based on BT.709 luminance `>= level`, preserving alpha.
pub fn threshold(image: &DynamicImage, level: u8) -> DynamicImage {
    map_rgb_preserving_alpha(image, |[r, g, b]| {
        let val = if bt709_luma_u8(r, g, b) >= level {
            255
        } else {
            0
        };
        [val, val, val]
    })
}

/// Curated one-click creative color grading presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorLook {
    Vivid,
    CinematicTealOrange,
    MatteFilm,
    MoodyBw,
    WarmGolden,
    CrispCool,
    HighKey,
    Cyberpunk,
}

const ALL_COLOR_LOOKS: [ColorLook; 8] = [
    ColorLook::Vivid,
    ColorLook::CinematicTealOrange,
    ColorLook::MatteFilm,
    ColorLook::MoodyBw,
    ColorLook::WarmGolden,
    ColorLook::CrispCool,
    ColorLook::HighKey,
    ColorLook::Cyberpunk,
];

impl ColorLook {
    /// Return all available curated color looks in display order.
    pub fn all() -> &'static [ColorLook] {
        &ALL_COLOR_LOOKS
    }

    /// Parse a color look from display name, slug, or variant identifier.
    pub fn from_name(name: &str) -> Option<ColorLook> {
        let normalized: String = name
            .trim()
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_lowercase())
            .collect();

        match normalized.as_str() {
            "vivid" => Some(Self::Vivid),
            "cinematictealorange" | "tealorange" => Some(Self::CinematicTealOrange),
            "mattefilm" => Some(Self::MatteFilm),
            "moodybw" => Some(Self::MoodyBw),
            "warmgolden" => Some(Self::WarmGolden),
            "crispcool" => Some(Self::CrispCool),
            "highkey" => Some(Self::HighKey),
            "cyberpunk" => Some(Self::Cyberpunk),
            _ => None,
        }
    }

    /// Return the human-readable display name for this look.
    pub fn name(self) -> &'static str {
        match self {
            Self::Vivid => "Vivid",
            Self::CinematicTealOrange => "Cinematic Teal & Orange",
            Self::MatteFilm => "Matte Film",
            Self::MoodyBw => "Moody B&W",
            Self::WarmGolden => "Warm Golden",
            Self::CrispCool => "Crisp Cool",
            Self::HighKey => "High Key",
            Self::Cyberpunk => "Cyberpunk",
        }
    }
}

/// Filter wrapper that applies a [`ColorLook`] inside a `FilterPipeline`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LookFilter {
    pub look: ColorLook,
}

impl Filter for LookFilter {
    fn name(&self) -> &'static str {
        self.look.name()
    }

    fn apply(&self, image: &DynamicImage) -> DynamicImage {
        apply_color_look(image, self.look)
    }
}

/// Apply a curated [`ColorLook`] preset to an image while preserving alpha.
pub fn apply_color_look(image: &DynamicImage, look: ColorLook) -> DynamicImage {
    match look {
        ColorLook::Vivid => {
            let vibranced = adjust_vibrance(image, 45.0);
            map_rgb_preserving_alpha(&vibranced, |[r, g, b]| {
                [
                    clamp_channel(s_curve(f32::from(r) / 255.0, 0.18) * 255.0),
                    clamp_channel(s_curve(f32::from(g) / 255.0, 0.18) * 255.0),
                    clamp_channel(s_curve(f32::from(b) / 255.0, 0.18) * 255.0),
                ]
            })
        }
        ColorLook::CinematicTealOrange => map_rgb_preserving_alpha(image, |[r, g, b]| {
            let mut rf = s_curve(f32::from(r) / 255.0, 0.28);
            let mut gf = s_curve(f32::from(g) / 255.0, 0.28);
            let mut bf = s_curve(f32::from(b) / 255.0, 0.28);

            let luma = (0.2126 * rf + 0.7152 * gf + 0.0722 * bf).clamp(0.0, 1.0);
            let w_s = (1.0 - luma).powi(2);
            let w_h = luma.powi(2);

            rf = (rf - 0.10 * w_s + 0.12 * w_h) * 0.96 + 0.025;
            gf = (gf + 0.04 * w_s + 0.04 * w_h) * 0.96 + 0.025;
            bf = (bf + 0.13 * w_s - 0.11 * w_h) * 0.96 + 0.025;

            [
                clamp_channel(rf * 255.0),
                clamp_channel(gf * 255.0),
                clamp_channel(bf * 255.0),
            ]
        }),
        ColorLook::MatteFilm => map_rgb_preserving_alpha(image, |[r, g, b]| {
            let rf = f32::from(r) / 255.0;
            let gf = f32::from(g) / 255.0;
            let bf = f32::from(b) / 255.0;
            let luma = 0.2126 * rf + 0.7152 * gf + 0.0722 * bf;

            let desat = 0.84;
            let mut mr = luma + (rf - luma) * desat;
            let mut mg = luma + (gf - luma) * desat;
            let mut mb = luma + (bf - luma) * desat;

            // Lifted matte black point (~20) and soft highlight roll-off (~242)
            mr = 0.078 + s_curve(mr, 0.12) * 0.87 + 0.012 * mr;
            mg = 0.078 + s_curve(mg, 0.12) * 0.87 + 0.008 * mg;
            mb = 0.082 + s_curve(mb, 0.12) * 0.85 - 0.010 * mb;

            [
                clamp_channel(mr * 255.0),
                clamp_channel(mg * 255.0),
                clamp_channel(mb * 255.0),
            ]
        }),
        ColorLook::MoodyBw => {
            let mut rgba = image.to_rgba8();
            let width = rgba.width();
            let height = rgba.height();
            if width == 0 || height == 0 {
                return DynamicImage::ImageRgba8(rgba);
            }
            let cx = width as f32 * 0.5;
            let cy = height as f32 * 0.5;
            let max_dist = cx.hypot(cy).max(1.0);

            for y in 0..height {
                for x in 0..width {
                    let pixel = rgba.get_pixel_mut(x, y);
                    let rf = f32::from(pixel[0]) / 255.0;
                    let gf = f32::from(pixel[1]) / 255.0;
                    let bf = f32::from(pixel[2]) / 255.0;
                    let film_luma = (0.34 * rf + 0.50 * gf + 0.16 * bf).clamp(0.0, 1.0);
                    let graded = s_curve(film_luma, 0.44);

                    let dist = (x as f32 + 0.5 - cx).hypot(y as f32 + 0.5 - cy) / max_dist;
                    let vig = 1.0 - 0.22 * dist.powf(1.8);
                    let out = clamp_channel(graded * vig * 255.0);
                    pixel[0] = out;
                    pixel[1] = out;
                    pixel[2] = out;
                }
            }
            DynamicImage::ImageRgba8(rgba)
        }
        ColorLook::WarmGolden => {
            let vibranced = adjust_vibrance(image, 22.0);
            map_rgb_preserving_alpha(&vibranced, |[r, g, b]| {
                let rf = f32::from(r) / 255.0;
                let gf = f32::from(g) / 255.0;
                let bf = f32::from(b) / 255.0;
                let luma = (0.2126 * rf + 0.7152 * gf + 0.0722 * bf).clamp(0.0, 1.0);
                let warm_w = 0.35 + 0.65 * luma;

                let nr = s_curve((rf + 0.10 * warm_w).clamp(0.0, 1.0), 0.12);
                let ng = s_curve((gf + 0.04 * warm_w).clamp(0.0, 1.0), 0.12);
                let nb = s_curve((bf - 0.10 * warm_w).clamp(0.0, 1.0), 0.12);

                [
                    clamp_channel(nr * 255.0),
                    clamp_channel(ng * 255.0),
                    clamp_channel(nb * 255.0),
                ]
            })
        }
        ColorLook::CrispCool => {
            let clarified = adjust_clarity(image, 20.0);
            map_rgb_preserving_alpha(&clarified, |[r, g, b]| {
                let rf = (f32::from(r) / 255.0 - 0.055).clamp(0.0, 1.0);
                let gf = (f32::from(g) / 255.0 + 0.012).clamp(0.0, 1.0);
                let bf = (f32::from(b) / 255.0 + 0.085).clamp(0.0, 1.0);

                [
                    clamp_channel(s_curve(rf, 0.20) * 255.0),
                    clamp_channel(s_curve(gf, 0.20) * 255.0),
                    clamp_channel(s_curve(bf, 0.20) * 255.0),
                ]
            })
        }
        ColorLook::HighKey => {
            let lifted = adjust_shadows_highlights(image, 45.0, -15.0);
            map_rgb_preserving_alpha(&lifted, |[r, g, b]| {
                let rf = (f32::from(r) / 255.0).powf(0.80);
                let gf = (f32::from(g) / 255.0).powf(0.80);
                let bf = (f32::from(b) / 255.0).powf(0.80);
                let luma = 0.2126 * rf + 0.7152 * gf + 0.0722 * bf;

                let sat = 0.88;
                let nr = s_curve((luma + (rf - luma) * sat + 0.04).clamp(0.0, 1.0), -0.10);
                let ng = s_curve((luma + (gf - luma) * sat + 0.04).clamp(0.0, 1.0), -0.10);
                let nb = s_curve((luma + (bf - luma) * sat + 0.04).clamp(0.0, 1.0), -0.10);

                [
                    clamp_channel(nr * 255.0),
                    clamp_channel(ng * 255.0),
                    clamp_channel(nb * 255.0),
                ]
            })
        }
        ColorLook::Cyberpunk => map_rgb_preserving_alpha(image, |[r, g, b]| {
            let mut rf = s_curve(f32::from(r) / 255.0, 0.34);
            let mut gf = s_curve(f32::from(g) / 255.0, 0.34);
            let mut bf = s_curve(f32::from(b) / 255.0, 0.34);

            let luma = (0.2126 * rf + 0.7152 * gf + 0.0722 * bf).clamp(0.0, 1.0);
            let sat = 1.28;
            rf = luma + (rf - luma) * sat;
            gf = luma + (gf - luma) * sat;
            bf = luma + (bf - luma) * sat;

            let w_s = (1.0 - luma).powi(2);
            let w_h = luma.powi(2);
            rf += 0.11 * w_s - 0.08 * w_h;
            gf += -0.07 * w_s + 0.09 * w_h;
            bf += 0.14 * w_s + 0.14 * w_h;

            [
                clamp_channel(rf * 255.0),
                clamp_channel(gf * 255.0),
                clamp_channel(bf * 255.0),
            ]
        }),
    }
}

// ── Shared helpers ──

#[inline]
fn bt709_luma_f32(r: u8, g: u8, b: u8) -> f32 {
    0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b)
}

#[inline]
fn bt709_luma_u8(r: u8, g: u8, b: u8) -> u8 {
    clamp_channel(bt709_luma_f32(r, g, b))
}

#[inline]
fn sanitize_finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[inline]
fn s_curve(t: f32, strength: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    (t + strength * (smooth - t)).clamp(0.0, 1.0)
}

#[inline]
fn skin_tone_protection(hue_deg: f32) -> f32 {
    let dist = ((hue_deg - 35.0 + 540.0) % 360.0) - 180.0;
    if dist.abs() >= 25.0 {
        return 1.0;
    }
    let t = (dist.abs() / 25.0).clamp(0.0, 1.0);
    let skin_weight = 0.5 * (1.0 + (t * PI).cos());
    1.0 - 0.75 * skin_weight
}

fn rescale_rgb_luma(rf: f32, gf: f32, bf: f32, old_luma: f32, new_luma: f32) -> [u8; 3] {
    if old_luma <= 1e-5 {
        let v = clamp_channel(new_luma * 255.0);
        return [v, v, v];
    }

    let scale = new_luma / old_luma;
    let mut nr = rf * scale;
    let mut ng = gf * scale;
    let mut nb = bf * scale;

    let max_c = nr.max(ng).max(nb);
    if max_c > 1.0 && max_c > new_luma {
        let k = ((1.0 - new_luma) / (max_c - new_luma)).clamp(0.0, 1.0);
        nr = new_luma + (nr - new_luma) * k;
        ng = new_luma + (ng - new_luma) * k;
        nb = new_luma + (nb - new_luma) * k;
    }

    [
        clamp_channel(nr * 255.0),
        clamp_channel(ng * 255.0),
        clamp_channel(nb * 255.0),
    ]
}

fn build_levels_lut(
    in_black: u8,
    gamma: f32,
    in_white: u8,
    out_black: u8,
    out_white: u8,
) -> [u8; 256] {
    let in_b = in_black.min(254);
    let in_w = in_white.max(in_b + 1);
    let in_span = f32::from(in_w - in_b);
    let out_b = f32::from(out_black);
    let out_span = f32::from(out_white) - out_b;
    let inv_gamma = 1.0 / gamma;

    let mut lut = [0u8; 256];
    for (i, slot) in lut.iter_mut().enumerate() {
        let norm = ((i as f32 - f32::from(in_b)) / in_span).clamp(0.0, 1.0);
        let curved = norm.powf(inv_gamma);
        *slot = clamp_channel(out_b + curved * out_span);
    }
    lut
}

fn build_shift_lut(delta: f32) -> [u8; 256] {
    let mut lut = [0u8; 256];
    for (i, slot) in lut.iter_mut().enumerate() {
        *slot = clamp_channel(i as f32 + delta);
    }
    lut
}

fn map_rgb_preserving_alpha<F>(image: &DynamicImage, op: F) -> DynamicImage
where
    F: Fn([u8; 3]) -> [u8; 3] + Sync,
{
    let mut rgba = image.to_rgba8();
    let raw = rgba.as_mut();
    if raw.len() >= PARALLEL_BYTE_THRESHOLD {
        raw.par_chunks_exact_mut(4).for_each(|px| {
            let [r, g, b] = op([px[0], px[1], px[2]]);
            px[0] = r;
            px[1] = g;
            px[2] = b;
        });
    } else {
        for px in raw.as_chunks_mut::<4>().0 {
            let [r, g, b] = op([px[0], px[1], px[2]]);
            px[0] = r;
            px[1] = g;
            px[2] = b;
        }
    }
    DynamicImage::ImageRgba8(rgba)
}

fn sample_histogram_bin(bins: &[u32; 256], x: u32, width: u32) -> f32 {
    if width >= 256 {
        let pos = x as f32 * 255.0 / (width - 1) as f32;
        let i0 = (pos.floor() as usize).min(255);
        let i1 = (i0 + 1).min(255);
        let frac = pos - i0 as f32;
        let v0 = bins[i0] as f32;
        let v1 = bins[i1] as f32;
        v0 + (v1 - v0) * frac
    } else {
        let w = width as usize;
        let start = ((x as usize * 256) / w).min(255);
        let end = (((x as usize + 1) * 256) / w).max(start + 1).min(256);
        bins[start..end].iter().copied().max().unwrap_or(0) as f32
    }
}

fn bin_value_to_y(norm: f32, height: u32, usable_h: f32) -> u32 {
    let y = (height - 1) as f32 - norm.clamp(0.0, 1.0) * usable_h;
    (y.round() as u32).clamp(1, height - 1)
}

fn add_translucent_tint(canvas: &mut RgbaImage, x: u32, y_top: u32, height: u32, add: [u8; 3]) {
    for y in y_top..height {
        let px = canvas.get_pixel_mut(x, y);
        px[0] = px[0].saturating_add(add[0]).min(220);
        px[1] = px[1].saturating_add(add[1]).min(220);
        px[2] = px[2].saturating_add(add[2]).min(220);
    }
}

fn draw_curve_outline(canvas: &mut RgbaImage, points: &[(bool, u32)], color: Rgba<u8>) {
    let mut prev: Option<(bool, u32)> = None;
    for (x, &(active, y)) in points.iter().enumerate() {
        let ux = x as u32;
        if let Some((prev_active, prev_y)) = prev {
            if active || prev_active {
                let y0 = prev_y.min(y);
                let y1 = prev_y.max(y);
                for py in y0..=y1 {
                    blend_over(canvas, ux, py, color);
                }
            }
        } else if active {
            blend_over(canvas, ux, y, color);
        }
        prev = Some((active, y));
    }
}

fn blend_over(canvas: &mut RgbaImage, x: u32, y: u32, src: Rgba<u8>) {
    let dst = *canvas.get_pixel(x, y);
    let a = u16::from(src[3]);
    let inv_a = 255 - a;
    let r = ((u16::from(src[0]) * a + u16::from(dst[0]) * inv_a + 127) / 255) as u8;
    let g = ((u16::from(src[1]) * a + u16::from(dst[1]) * inv_a + 127) / 255) as u8;
    let b = ((u16::from(src[2]) * a + u16::from(dst[2]) * inv_a + 127) / 255) as u8;
    canvas.put_pixel(x, y, Rgba([r, g, b, 255]));
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

    use super::*;
    use crate::filters::FilterPipeline;

    fn pixel_img(r: u8, g: u8, b: u8, a: u8) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([r, g, b, a])))
    }

    #[test]
    fn histogram_clipping_mean_stddev_and_serde_roundtrip() {
        let mut img = RgbaImage::new(4, 1);
        img.put_pixel(0, 0, Rgba([0, 0, 0, 255])); // luma 0 (shadow clip)
        img.put_pixel(1, 0, Rgba([2, 2, 2, 128])); // luma 2 (shadow clip)
        img.put_pixel(2, 0, Rgba([253, 253, 253, 255])); // luma 253 (highlight clip)
        img.put_pixel(3, 0, Rgba([255, 255, 255, 255])); // luma 255 (highlight clip)

        let hist = compute_histogram(&DynamicImage::ImageRgba8(img));
        assert_eq!(hist.pixel_count, 4);
        assert_eq!(hist.luma[0], 1);
        assert_eq!(hist.luma[2], 1);
        assert_eq!(hist.luma[253], 1);
        assert_eq!(hist.luma[255], 1);
        assert!((hist.shadow_clip_pct - 50.0).abs() < 0.01);
        assert!((hist.highlight_clip_pct - 50.0).abs() < 0.01);
        assert!((hist.mean_luma - 127.5).abs() < 0.01);
        assert!(hist.std_dev_luma > 120.0);

        let json = serde_json::to_string(&hist).expect("serialize HistogramData");
        let parsed: HistogramData = serde_json::from_str(&json).expect("deserialize HistogramData");
        assert_eq!(parsed, hist);
    }

    #[test]
    fn histogram_empty_image_is_safe() {
        let empty = DynamicImage::ImageRgba8(RgbaImage::new(0, 0));
        let hist = compute_histogram(&empty);
        assert_eq!(hist.pixel_count, 0);
        assert_eq!(hist.mean_luma, 0.0);
        assert_eq!(hist.std_dev_luma, 0.0);
        assert_eq!(hist.shadow_clip_pct, 0.0);
        assert_eq!(hist.highlight_clip_pct, 0.0);
    }

    #[test]
    fn render_histogram_clamps_min_dimensions_and_uses_slate_bg() {
        let empty_hist = HistogramData::default();
        let tiny = render_histogram_image(&empty_hist, 10, 8);
        assert_eq!(tiny.dimensions(), (64, 32));
        assert_eq!(tiny.to_rgba8().get_pixel(0, 0).0, [0x18, 0x1E, 0x26, 255]);

        let img = pixel_img(200, 100, 50, 255);
        let hist = compute_histogram(&img);
        let small_non_empty = render_histogram_image(&hist, 32, 16);
        assert_eq!(small_non_empty.dimensions(), (64, 32));
        let rendered = render_histogram_image(&hist, 300, 120);
        assert_eq!(rendered.dimensions(), (300, 120));
        assert_eq!(
            rendered.to_rgba8().get_pixel(0, 0).0,
            [0x18, 0x1E, 0x26, 255]
        );
        // Rendered histogram should contain drawn curve/fill pixels differing from background
        assert!(
            small_non_empty
                .to_rgba8()
                .pixels()
                .any(|p| p.0 != [0x18, 0x1E, 0x26, 255])
        );
    }

    #[test]
    fn empty_and_nan_inputs_do_not_panic() {
        let empty = DynamicImage::ImageRgba8(RgbaImage::new(0, 0));
        assert_eq!(
            adjust_levels(&empty, 10, f32::NAN, 240, 0, 255).dimensions(),
            (0, 0)
        );
        assert_eq!(
            adjust_shadows_highlights(&empty, f32::NAN, f32::INFINITY).dimensions(),
            (0, 0)
        );
        assert_eq!(
            adjust_vibrance(&empty, f32::NEG_INFINITY).dimensions(),
            (0, 0)
        );
        assert_eq!(adjust_clarity(&empty, f32::NAN).dimensions(), (0, 0));
        assert_eq!(
            adjust_color_balance(&empty, f32::NAN, 10.0, -10.0).dimensions(),
            (0, 0)
        );
        assert_eq!(threshold(&empty, 128).dimensions(), (0, 0));
        for &look in ColorLook::all() {
            assert_eq!(apply_color_look(&empty, look).dimensions(), (0, 0));
        }

        let single =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([120, 80, 40, 200])));
        let clarity_1x1 = adjust_clarity(&single, 50.0).to_rgba8();
        assert_eq!(clarity_1x1.get_pixel(0, 0).0, [120, 80, 40, 200]);

        let neg_vib = adjust_vibrance(&single, -100.0).to_rgba8();
        let px = neg_vib.get_pixel(0, 0).0;
        assert_eq!(px[0], px[1]);
        assert_eq!(px[1], px[2]);
        assert_eq!(px[3], 200);
    }

    #[test]
    fn levels_identity_and_lut_and_alpha() {
        let img = pixel_img(80, 140, 200, 99);
        let id = adjust_levels(&img, 0, 1.0, 255, 0, 255);
        assert_eq!(id.to_rgba8().get_pixel(0, 0).0, [80, 140, 200, 99]);

        // Stretch [50..200] -> [0..255] and test degenerate in_black >= in_white safety
        let stretched = adjust_levels(&img, 50, 1.0, 200, 0, 255).to_rgba8();
        let px = stretched.get_pixel(0, 0).0;
        assert_eq!(px[3], 99);
        assert!(px[0] < 80);
        assert_eq!(px[2], 255);

        // Degenerate bounds must not panic
        let safe_deg = adjust_levels(&img, 255, 10.0, 10, 0, 255).to_rgba8();
        assert_eq!(safe_deg.get_pixel(0, 0)[3], 99);

        // Gamma > 1 brightens midtones
        let mid = pixel_img(128, 128, 128, 255);
        let brighter = adjust_levels(&mid, 0, 2.0, 255, 0, 255).to_rgba8();
        assert!(brighter.get_pixel(0, 0)[0] > 128);
    }

    #[test]
    fn shadows_highlights_identity_lift_recovery_and_alpha() {
        let dark = pixel_img(30, 20, 10, 140);
        let id = adjust_shadows_highlights(&dark, 0.0, 0.0);
        assert_eq!(id.to_rgba8().get_pixel(0, 0).0, [30, 20, 10, 140]);

        let lifted = adjust_shadows_highlights(&dark, 80.0, 0.0).to_rgba8();
        let lpx = lifted.get_pixel(0, 0).0;
        assert_eq!(lpx[3], 140);
        assert!(lpx[0] > 30 && lpx[1] > 20 && lpx[2] > 10);
        // Preserves approximate 3:2:1 color ratio
        assert!(lpx[0] > lpx[1] && lpx[1] > lpx[2]);

        let bright = pixel_img(240, 230, 220, 200);
        let recovered = adjust_shadows_highlights(&bright, 0.0, -80.0).to_rgba8();
        let rpx = recovered.get_pixel(0, 0).0;
        assert_eq!(rpx[3], 200);
        assert!(rpx[0] < 240 && rpx[1] < 230 && rpx[2] < 220);
    }

    #[test]
    fn vibrance_identity_skin_protection_and_saturation_priority() {
        let gray = pixel_img(120, 120, 120, 180);
        assert_eq!(
            adjust_vibrance(&gray, 80.0).to_rgba8().get_pixel(0, 0).0,
            [120, 120, 120, 180]
        );

        let muted_blue = pixel_img(110, 120, 145, 77);
        assert_eq!(
            adjust_vibrance(&muted_blue, 0.0)
                .to_rgba8()
                .get_pixel(0, 0)
                .0,
            [110, 120, 145, 77]
        );

        let boosted_blue = adjust_vibrance(&muted_blue, 100.0).to_rgba8();
        let (_, s_before, _) = rgb_to_hsl(110, 120, 145);
        let bpx = boosted_blue.get_pixel(0, 0).0;
        assert_eq!(bpx[3], 77);
        let (_, s_after, _) = rgb_to_hsl(bpx[0], bpx[1], bpx[2]);
        assert!(s_after > s_before);

        // Low-saturation blue should receive a larger saturation increase than vivid blue
        let vivid_blue = pixel_img(40, 80, 220, 255);
        let (_, vs_before, _) = rgb_to_hsl(40, 80, 220);
        let vpx = adjust_vibrance(&vivid_blue, 100.0)
            .to_rgba8()
            .get_pixel(0, 0)
            .0;
        let (_, vs_after, _) = rgb_to_hsl(vpx[0], vpx[1], vpx[2]);
        assert!((s_after - s_before) > (vs_after - vs_before));

        // Skin tone (~35 deg hue) should be boosted much less than a non-skin hue of similar sat
        let skin = pixel_img(160, 136, 110, 255); // hue ~31 deg, sat ~0.21
        let blue = pixel_img(110, 132, 160, 255); // hue ~214 deg, sat ~0.21
        let (_, skin_s0, _) = rgb_to_hsl(160, 136, 110);
        let (_, blue_s0, _) = rgb_to_hsl(110, 132, 160);
        let skin_out = adjust_vibrance(&skin, 100.0).to_rgba8().get_pixel(0, 0).0;
        let blue_out = adjust_vibrance(&blue, 100.0).to_rgba8().get_pixel(0, 0).0;
        let (_, skin_s1, _) = rgb_to_hsl(skin_out[0], skin_out[1], skin_out[2]);
        let (_, blue_s1, _) = rgb_to_hsl(blue_out[0], blue_out[1], blue_out[2]);
        assert!((skin_s1 - skin_s0) < (blue_s1 - blue_s0));
    }

    #[test]
    fn clarity_identity_local_contrast_and_alpha() {
        let mut img = RgbaImage::new(16, 16);
        for y in 0..16 {
            for x in 0..16 {
                let v = if x < 8 { 90 } else { 170 };
                img.put_pixel(x, y, Rgba([v, v, v, 155]));
            }
        }
        let dyn_img = DynamicImage::ImageRgba8(img);

        let id = adjust_clarity(&dyn_img, 0.0);
        assert_eq!(id.as_bytes(), dyn_img.as_bytes());

        let positive = adjust_clarity(&dyn_img, 80.0).to_rgba8();
        // Right next to the boundary (x=7 vs x=8), contrast should sharpen while alpha stays 155
        assert_eq!(positive.get_pixel(7, 8)[3], 155);
        assert!(positive.get_pixel(7, 8)[0] < 90);
        assert!(positive.get_pixel(8, 8)[0] > 170);

        let negative = adjust_clarity(&dyn_img, -80.0).to_rgba8();
        assert!(negative.get_pixel(7, 8)[0] > 90);
        assert!(negative.get_pixel(8, 8)[0] < 170);
    }

    #[test]
    fn color_balance_shifts_channels_and_preserves_alpha() {
        let img = pixel_img(128, 128, 128, 42);
        let id = adjust_color_balance(&img, 0.0, 0.0, 0.0);
        assert_eq!(id.to_rgba8().get_pixel(0, 0).0, [128, 128, 128, 42]);

        let shifted = adjust_color_balance(&img, 50.0, 0.0, -50.0).to_rgba8();
        let px = shifted.get_pixel(0, 0).0;
        assert!(px[0] > 128);
        assert_eq!(px[1], 128);
        assert!(px[2] < 128);
        assert_eq!(px[3], 42);
    }

    #[test]
    fn threshold_binary_bt709_and_alpha() {
        let mut img = RgbaImage::new(2, 1);
        img.put_pixel(0, 0, Rgba([50, 50, 50, 111]));
        img.put_pixel(1, 0, Rgba([200, 200, 200, 222]));

        let out = threshold(&DynamicImage::ImageRgba8(img), 128).to_rgba8();
        assert_eq!(out.get_pixel(0, 0).0, [0, 0, 0, 111]);
        assert_eq!(out.get_pixel(1, 0).0, [255, 255, 255, 222]);

        let all_white = threshold(&pixel_img(0, 0, 0, 90), 0).to_rgba8();
        assert_eq!(all_white.get_pixel(0, 0).0, [255, 255, 255, 90]);

        let near_white = threshold(&pixel_img(254, 254, 254, 90), 255).to_rgba8();
        assert_eq!(near_white.get_pixel(0, 0).0, [0, 0, 0, 90]);
    }

    #[test]
    fn parallel_path_large_image_matches_expected_behavior() {
        let img =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(256, 256, Rgba([60, 120, 180, 150])));
        let out = adjust_levels(&img, 20, 1.2, 220, 0, 255).to_rgba8();
        assert_eq!(out.dimensions(), (256, 256));
        assert_eq!(out.get_pixel(128, 128)[3], 150);

        let clar = adjust_clarity(&img, 40.0).to_rgba8();
        assert_eq!(clar.get_pixel(128, 128).0, [60, 120, 180, 150]);
    }

    #[test]
    fn shadows_highlights_bidirectional_and_levels_inverted_output() {
        let mid_dark = pixel_img(60, 60, 60, 255);
        let crushed = adjust_shadows_highlights(&mid_dark, -80.0, 0.0).to_rgba8();
        assert!(crushed.get_pixel(0, 0)[0] < 60);

        let mid_bright = pixel_img(190, 190, 190, 255);
        let boosted = adjust_shadows_highlights(&mid_bright, 0.0, 80.0).to_rgba8();
        assert!(boosted.get_pixel(0, 0)[0] > 190);

        // Inverted output levels: black maps to 255, white maps to 0
        let inv = adjust_levels(&pixel_img(0, 255, 128, 75), 0, 1.0, 255, 255, 0).to_rgba8();
        assert_eq!(inv.get_pixel(0, 0).0, [255, 0, 127, 75]);
    }

    #[test]
    fn color_looks_all_variants_parse_filter_and_alpha() {
        let all = ColorLook::all();
        assert_eq!(all.len(), 8);

        let mut img = RgbaImage::new(8, 8);
        for y in 0..8 {
            for x in 0..8 {
                img.put_pixel(
                    x,
                    y,
                    Rgba([60 + x as u8 * 20, 110, 180 - y as u8 * 15, 173]),
                );
            }
        }
        let dyn_img = DynamicImage::ImageRgba8(img);

        let mut outputs = Vec::with_capacity(all.len());
        let mut pipeline = FilterPipeline::new();
        for &look in all {
            assert!(!look.name().is_empty());
            assert_eq!(ColorLook::from_name(look.name()), Some(look));

            let filter = LookFilter { look };
            assert_eq!(filter.name(), look.name());

            let out = filter.apply(&dyn_img).to_rgba8();
            assert_eq!(out.dimensions(), (8, 8));
            assert_eq!(out.get_pixel(3, 4)[3], 173);

            if look == ColorLook::MoodyBw {
                let p = out.get_pixel(2, 2).0;
                assert_eq!(p[0], p[1]);
                assert_eq!(p[1], p[2]);
            }

            outputs.push(out.into_raw());
            pipeline.push(Box::new(filter));
        }

        // Slug parsing checks
        assert_eq!(
            ColorLook::from_name("cinematic_teal_orange"),
            Some(ColorLook::CinematicTealOrange)
        );
        assert_eq!(ColorLook::from_name("moody-bw"), Some(ColorLook::MoodyBw));
        assert_eq!(ColorLook::from_name("nonexistent"), None);

        // Every look produces a distinct visual result
        for i in 0..outputs.len() {
            for j in (i + 1)..outputs.len() {
                assert_ne!(outputs[i], outputs[j], "looks {i} and {j} collided");
            }
        }

        assert_eq!(pipeline.len(), 8);
        assert_eq!(pipeline.apply_all(&dyn_img).dimensions(), (8, 8));
    }

    #[test]
    fn filter_structs_work_in_pipeline() {
        let img = pixel_img(100, 120, 140, 210);
        let mut pipeline = FilterPipeline::new();
        pipeline.push(Box::new(LevelsAdjust::default()));
        pipeline.push(Box::new(ShadowsHighlights {
            shadows: 10.0,
            highlights: -10.0,
        }));
        pipeline.push(Box::new(VibranceAdjust { amount: 25.0 }));
        pipeline.push(Box::new(ClarityAdjust { amount: 15.0 }));
        pipeline.push(Box::new(ColorBalance {
            r_shift: 5.0,
            g_shift: 0.0,
            b_shift: -5.0,
        }));
        pipeline.push(Box::new(Threshold::default()));

        let out = pipeline.apply_all(&img).to_rgba8();
        assert_eq!(out.get_pixel(0, 0)[3], 210);
    }
}
