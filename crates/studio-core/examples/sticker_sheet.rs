//! Renders every built-in sticker into one contact sheet: `cargo run --example sticker_sheet out.png`
use studio_core::stickers::{StickerStyle, library, render_sticker};
fn main() {
    let lib = library();
    let (cell, cols) = (96u32, 10u32);
    let rows = (lib.len() as u32).div_ceil(cols);
    let mut sheet =
        image::RgbaImage::from_pixel(cell * cols, cell * rows, image::Rgba([235, 238, 242, 255]));
    for (i, s) in lib.iter().enumerate() {
        let style = StickerStyle {
            fill: s.default_fill,
            ..Default::default()
        };
        let img = render_sticker(s, &style, cell - 8, cell - 8).to_rgba8();
        let (x, y) = ((i as u32 % cols) * cell + 4, (i as u32 / cols) * cell + 4);
        image::imageops::overlay(&mut sheet, &img, x as i64, y as i64);
    }
    sheet
        .save(std::env::args().nth(1).unwrap_or("stickers.png".into()))
        .unwrap();
}
