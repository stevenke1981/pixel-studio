//! Destructive redaction: sensitive-text classification and pixel-flattening redaction styles.

use image::{DynamicImage, Rgba, RgbaImage};
use regex::Regex;
use std::sync::OnceLock;

/// An axis-aligned redaction candidate in image pixel coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct RedactBox {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Recognized text (may be empty for manual boxes or unrecognized scripts).
    pub text: String,
    /// Whether this box will be redacted when applied.
    pub selected: bool,
    /// Sensitive data category detected in `text`, if any.
    pub kind: Option<SensitiveKind>,
}

impl RedactBox {
    pub fn manual(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            text: String::new(),
            selected: true,
            kind: None,
        }
    }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x as f32
            && py >= self.y as f32
            && px < (self.x + self.width as i32) as f32
            && py < (self.y + self.height as i32) as f32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitiveKind {
    Email,
    Phone,
    TaiwanId,
    CreditCard,
    Url,
    Address,
}

impl SensitiveKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Email => "Email",
            Self::Phone => "Phone",
            Self::TaiwanId => "TW ID",
            Self::CreditCard => "Card",
            Self::Url => "URL",
            Self::Address => "Address",
        }
    }
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("valid regex"))
}

/// Taiwan national ID checksum (A123456789 style).
pub fn is_valid_taiwan_id(id: &str) -> bool {
    let b: Vec<char> = id.chars().collect();
    if b.len() != 10 || !b[0].is_ascii_uppercase() || !matches!(b[1], '1' | '2' | '8' | '9') {
        return false;
    }
    if !b[1..].iter().all(|c| c.is_ascii_digit()) {
        return false;
    }
    const CODES: [u32; 26] = [
        10, 11, 12, 13, 14, 15, 16, 17, 34, 18, 19, 20, 21, 22, 35, 23, 24, 25, 26, 27, 28, 29, 32,
        30, 31, 33,
    ];
    let code = CODES[(b[0] as u8 - b'A') as usize];
    let mut sum = code / 10 + (code % 10) * 9;
    let weights = [8, 7, 6, 5, 4, 3, 2, 1, 1];
    for (i, w) in weights.iter().enumerate() {
        sum += b[i + 1].to_digit(10).unwrap() * w;
    }
    sum % 10 == 0
}

pub fn luhn_valid(digits: &str) -> bool {
    let d: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    if !(13..=19).contains(&d.len()) {
        return false;
    }
    let mut sum = 0;
    for (i, v) in d.iter().rev().enumerate() {
        let mut v = *v;
        if i % 2 == 1 {
            v *= 2;
            if v > 9 {
                v -= 9;
            }
        }
        sum += v;
    }
    sum % 10 == 0
}

/// Classify text as a sensitive data category. OCR output is normalized first
/// (common O/0, l/1 confusions inside digit runs are tolerated by the patterns).
pub fn classify_sensitive(text: &str) -> Option<SensitiveKind> {
    static EMAIL: OnceLock<Regex> = OnceLock::new();
    static URL: OnceLock<Regex> = OnceLock::new();
    static TWID: OnceLock<Regex> = OnceLock::new();
    static CARD: OnceLock<Regex> = OnceLock::new();
    static PHONE: OnceLock<Regex> = OnceLock::new();
    static ADDR_EN: OnceLock<Regex> = OnceLock::new();
    static ADDR_ZH: OnceLock<Regex> = OnceLock::new();
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if re(&EMAIL, r"(?i)[a-z0-9._%+\-]+\s?@\s?[a-z0-9.\-]+\.[a-z]{2,}").is_match(t) {
        return Some(SensitiveKind::Email);
    }
    for m in re(&TWID, r"\b[A-Z][1289]\d{8}\b").find_iter(t) {
        if is_valid_taiwan_id(m.as_str()) {
            return Some(SensitiveKind::TaiwanId);
        }
    }
    for m in re(&CARD, r"\b(?:\d[ \-]?){12,18}\d\b").find_iter(t) {
        if luhn_valid(m.as_str()) {
            return Some(SensitiveKind::CreditCard);
        }
    }
    if re(
        &PHONE,
        r"(?x)
        (?:\+\d{1,3}[\s\-.]?)?\(?\d{1,4}\)?[\s\-.]?\d{3,4}[\s\-.]?\d{3,4}   # generic / international
        | \b09\d{2}[\s\-]?\d{3}[\s\-]?\d{3}\b                              # TW mobile
        ",
    )
    .find_iter(t)
    .any(|m| m.as_str().chars().filter(char::is_ascii_digit).count() >= 8)
    {
        return Some(SensitiveKind::Phone);
    }
    if re(
        &URL,
        r"(?i)\b(?:https?://|www\.)[^\s]+|\b[a-z0-9\-]+\.(?:com|net|org|tw|io|dev)(?:/[^\s]*)?\b",
    )
    .is_match(t)
    {
        return Some(SensitiveKind::Url);
    }
    if re(&ADDR_ZH, r"[市縣區鄉鎮].{0,12}[路街道巷弄].{0,10}\d+\s*號|\d+\s*號\s*\d*\s*樓?").is_match(t)
        || re(&ADDR_EN, r"(?i)\b\d{1,5}\s+[a-z][a-z .]{2,30}\b(?:street|st|road|rd|avenue|ave|blvd|lane|ln|drive|dr)\b").is_match(t)
    {
        return Some(SensitiveKind::Address);
    }
    None
}

/// Mark boxes as selected when their text is sensitive. Returns the number selected.
pub fn preselect_sensitive(boxes: &mut [RedactBox]) -> usize {
    let mut n = 0;
    for b in boxes.iter_mut() {
        b.kind = classify_sensitive(&b.text);
        b.selected = b.kind.is_some();
        n += b.selected as usize;
    }
    n
}

/// Select every box whose text contains `keyword` (case-insensitive). Returns matches.
pub fn select_by_keyword(boxes: &mut [RedactBox], keyword: &str) -> usize {
    let k = keyword.trim().to_lowercase();
    if k.is_empty() {
        return 0;
    }
    let mut n = 0;
    for b in boxes.iter_mut() {
        if b.text.to_lowercase().contains(&k) {
            b.selected = true;
            n += 1;
        }
    }
    n
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RedactStyle {
    /// Opaque fill. Irreversible, the default.
    Solid([u8; 4]),
    /// Pixelate with the given block size. Can sometimes be partially reversed.
    Mosaic(u32),
    /// Gaussian blur with sigma. Can sometimes be partially reversed.
    Blur(f32),
}

impl Default for RedactStyle {
    fn default() -> Self {
        Self::Solid([0, 0, 0, 255])
    }
}

/// Clamp a padded box to image bounds, returning (x, y, w, h) or None if empty.
pub fn padded_rect(b: &RedactBox, padding: u32, w: u32, h: u32) -> Option<(u32, u32, u32, u32)> {
    let p = padding as i64;
    let x0 = (b.x as i64 - p).clamp(0, w as i64);
    let y0 = (b.y as i64 - p).clamp(0, h as i64);
    let x1 = (b.x as i64 + b.width as i64 + p).clamp(0, w as i64);
    let y1 = (b.y as i64 + b.height as i64 + p).clamp(0, h as i64);
    (x1 > x0 && y1 > y0).then(|| (x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32))
}

/// Destructively redact one rectangle into `img`.
pub fn redact_rect(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, style: RedactStyle) {
    match style {
        RedactStyle::Solid(c) => {
            for py in y..(y + h).min(img.height()) {
                for px in x..(x + w).min(img.width()) {
                    img.put_pixel(px, py, Rgba(c));
                }
            }
        }
        RedactStyle::Mosaic(block) => {
            crate::drawing::apply_mosaic_region(img, x, y, w, h, block.max(2));
        }
        RedactStyle::Blur(sigma) => {
            let sub = image::imageops::crop_imm(img, x, y, w, h).to_image();
            let blurred = image::imageops::blur(&sub, sigma.max(1.0));
            image::imageops::replace(img, &blurred, x as i64, y as i64);
        }
    }
}

/// Apply all selected boxes destructively. Returns how many were applied.
pub fn apply_redactions(
    image: &DynamicImage,
    boxes: &[RedactBox],
    style: RedactStyle,
    padding: u32,
) -> (DynamicImage, usize) {
    let mut img = image.to_rgba8();
    let (w, h) = img.dimensions();
    let mut n = 0;
    for b in boxes.iter().filter(|b| b.selected) {
        if let Some((x, y, bw, bh)) = padded_rect(b, padding, w, h) {
            redact_rect(&mut img, x, y, bw, bh, style);
            n += 1;
        }
    }
    (DynamicImage::ImageRgba8(img), n)
}

/// Redact a round brush dab (solid colour) — used by the manual redact brush.
pub fn redact_brush_dab(img: &mut RgbaImage, cx: f32, cy: f32, radius: f32, color: [u8; 4]) {
    let r = radius.max(1.0);
    let (w, h) = img.dimensions();
    let x0 = (cx - r).floor().max(0.0) as u32;
    let y0 = (cy - r).floor().max(0.0) as u32;
    let x1 = ((cx + r).ceil() as u32).min(w);
    let y1 = ((cy + r).ceil() as u32).min(h);
    for y in y0..y1 {
        for x in x0..x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy <= r * r {
                img.put_pixel(x, y, Rgba(color));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_sensitive_strings() {
        assert_eq!(
            classify_sensitive("mail: alice@example.com"),
            Some(SensitiveKind::Email)
        );
        assert_eq!(
            classify_sensitive("0912-345-678"),
            Some(SensitiveKind::Phone)
        );
        assert_eq!(
            classify_sensitive("+886 2 2345 6789"),
            Some(SensitiveKind::Phone)
        );
        assert_eq!(
            classify_sensitive("+1 (415) 555-0132"),
            Some(SensitiveKind::Phone)
        );
        assert_eq!(
            classify_sensitive("ID A123456789"),
            Some(SensitiveKind::TaiwanId)
        );
        assert_eq!(
            classify_sensitive("4111 1111 1111 1111"),
            Some(SensitiveKind::CreditCard)
        );
        assert_eq!(
            classify_sensitive("https://example.com/x"),
            Some(SensitiveKind::Url)
        );
        assert_eq!(
            classify_sensitive("台北市信義區信義路五段7號"),
            Some(SensitiveKind::Address)
        );
        assert_eq!(
            classify_sensitive("221 Baker Street"),
            Some(SensitiveKind::Address)
        );
        assert_eq!(classify_sensitive("Hello world"), None);
        assert_eq!(classify_sensitive("Version 2026"), None);
    }

    #[test]
    fn taiwan_id_checksum() {
        assert!(is_valid_taiwan_id("A123456789"));
        assert!(!is_valid_taiwan_id("A123456788"));
        assert!(luhn_valid("4111111111111111"));
        assert!(!luhn_valid("4111111111111112"));
    }

    #[test]
    fn solid_redaction_is_destructive_and_padded() {
        let img =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(50, 50, Rgba([255, 255, 255, 255])));
        let boxes = vec![RedactBox::manual(10, 10, 10, 10)];
        let (out, n) = apply_redactions(&img, &boxes, RedactStyle::default(), 2);
        let out = out.to_rgba8();
        assert_eq!(n, 1);
        assert_eq!(out.get_pixel(8, 8), &Rgba([0, 0, 0, 255]));
        assert_eq!(out.get_pixel(21, 21), &Rgba([0, 0, 0, 255]));
        assert_eq!(out.get_pixel(7, 7), &Rgba([255, 255, 255, 255]));
        assert_eq!(out.get_pixel(22, 22), &Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn keyword_and_preselect() {
        let mut boxes = vec![
            RedactBox {
                text: "Call 0912345678".into(),
                selected: false,
                ..RedactBox::manual(0, 0, 1, 1)
            },
            RedactBox {
                text: "Project Alpha".into(),
                selected: false,
                ..RedactBox::manual(0, 0, 1, 1)
            },
        ];
        assert_eq!(preselect_sensitive(&mut boxes), 1);
        assert!(!boxes[1].selected);
        assert_eq!(select_by_keyword(&mut boxes, "alpha"), 1);
        assert!(boxes[1].selected);
    }

    #[test]
    fn padding_clamps_to_bounds() {
        let b = RedactBox::manual(-5, -5, 10, 10);
        assert_eq!(padded_rect(&b, 3, 20, 20), Some((0, 0, 8, 8)));
        assert_eq!(
            padded_rect(&RedactBox::manual(30, 30, 5, 5), 0, 20, 20),
            None
        );
    }
}
