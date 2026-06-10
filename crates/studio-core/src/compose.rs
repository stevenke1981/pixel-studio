use std::path::PathBuf;

use ab_glyph::{FontArc, PxScale};
use image::imageops::FilterType;
use image::{DynamicImage, Rgba, RgbaImage};
use imageproc::drawing::{draw_text_mut, text_size};

#[derive(Clone)]
pub struct ComposeDocument {
    pub background: DynamicImage,
    pub layers: Vec<ComposeLayer>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone)]
pub enum ComposeLayer {
    Text(TextLayer),
    Image(ImageLayer),
}

impl ComposeLayer {
    pub fn name(&self) -> &str {
        match self {
            Self::Text(layer) => &layer.name,
            Self::Image(layer) => &layer.name,
        }
    }

    pub fn name_mut(&mut self) -> &mut String {
        match self {
            Self::Text(layer) => &mut layer.name,
            Self::Image(layer) => &mut layer.name,
        }
    }

    pub fn is_visible(&self) -> bool {
        match self {
            Self::Text(layer) => layer.visible,
            Self::Image(layer) => layer.visible,
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        match self {
            Self::Text(layer) => layer.visible = visible,
            Self::Image(layer) => layer.visible = visible,
        }
    }
}

#[derive(Clone)]
pub struct TextLayer {
    pub name: String,
    pub text: String,
    pub font_path: Option<PathBuf>,
    pub x: i32,
    pub y: i32,
    pub font_size: f32,
    pub color: [u8; 4],
    pub opacity: f32,
    pub stroke: bool,
    pub shadow: bool,
    pub visible: bool,
}

impl Default for TextLayer {
    fn default() -> Self {
        Self {
            name: "Text".to_owned(),
            text: "文字".to_owned(),
            font_path: None,
            x: 32,
            y: 32,
            font_size: 48.0,
            color: [255, 255, 255, 255],
            opacity: 1.0,
            stroke: true,
            shadow: true,
            visible: true,
        }
    }
}

#[derive(Clone)]
pub struct ImageLayer {
    pub name: String,
    pub image: DynamicImage,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub opacity: f32,
    pub visible: bool,
}

impl ImageLayer {
    pub fn new(name: String, image: DynamicImage) -> Self {
        let width = image.width().max(1);
        let height = image.height().max(1);
        Self {
            name,
            image,
            x: 32,
            y: 32,
            width,
            height,
            opacity: 1.0,
            visible: true,
        }
    }
}

pub fn render_composition(document: &ComposeDocument, font_bytes: Option<&[u8]>) -> DynamicImage {
    let mut canvas = document.background.to_rgba8();
    let default_font = font_bytes.and_then(|bytes| FontArc::try_from_vec(bytes.to_vec()).ok());

    for layer in &document.layers {
        match layer {
            ComposeLayer::Text(layer) if layer.visible => {
                let layer_font = load_layer_font(layer);
                if let Some(font) = layer_font.as_ref().or(default_font.as_ref()) {
                    draw_text_layer(&mut canvas, layer, font);
                }
            }
            ComposeLayer::Image(layer) if layer.visible => {
                draw_image_layer(&mut canvas, layer);
            }
            _ => {}
        }
    }

    DynamicImage::ImageRgba8(canvas)
}

pub fn text_layer_bounds(layer: &TextLayer, font_bytes: Option<&[u8]>) -> LayerBounds {
    let font = load_layer_font(layer)
        .or_else(|| font_bytes.and_then(|bytes| FontArc::try_from_vec(bytes.to_vec()).ok()));
    let (width, height) = font
        .as_ref()
        .map(|font| measure_text_block(font, layer.font_size, &layer.text))
        .unwrap_or_else(|| estimate_text_block(layer));
    let mut padding = 2;
    if layer.stroke {
        padding += 2;
    }
    if layer.shadow {
        padding += 4;
    }

    LayerBounds {
        x: layer.x - padding,
        y: layer.y - padding,
        width: width.saturating_add((padding * 2) as u32).max(1),
        height: height.saturating_add((padding * 2) as u32).max(1),
    }
}

fn draw_text_layer(canvas: &mut RgbaImage, layer: &TextLayer, font: &FontArc) {
    let scale = PxScale::from(layer.font_size.max(1.0));
    let color = color_with_opacity(layer.color, layer.opacity);
    let line_height = line_height(font, scale);

    for (line_index, line) in layer.text.split('\n').enumerate() {
        let y = layer.y + (line_index as i32 * line_height as i32);

        if layer.shadow {
            draw_text_mut(
                canvas,
                Rgba([0, 0, 0, ((180.0 * layer.opacity).round() as u8)]),
                layer.x + 3,
                y + 3,
                scale,
                font,
                line,
            );
        }

        if layer.stroke {
            let stroke_color = Rgba([0, 0, 0, ((220.0 * layer.opacity).round() as u8)]);
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                draw_text_mut(
                    canvas,
                    stroke_color,
                    layer.x + dx,
                    y + dy,
                    scale,
                    font,
                    line,
                );
            }
        }

        draw_text_mut(canvas, Rgba(color), layer.x, y, scale, font, line);
    }
}

fn load_layer_font(layer: &TextLayer) -> Option<FontArc> {
    layer
        .font_path
        .as_ref()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| FontArc::try_from_vec(bytes).ok())
}

fn measure_text_block(font: &FontArc, font_size: f32, text: &str) -> (u32, u32) {
    let scale = PxScale::from(font_size.max(1.0));
    let line_height = line_height(font, scale);
    let mut width = 1;
    let mut line_count = 0_u32;

    for line in text.split('\n') {
        let (line_width, _) = text_size(scale, font, line);
        width = width.max(line_width.max(1));
        line_count += 1;
    }

    (width, line_height.saturating_mul(line_count.max(1)).max(1))
}

fn line_height(font: &FontArc, scale: PxScale) -> u32 {
    let (_, height) = text_size(scale, font, "Hg");
    height.max(scale.y.ceil() as u32).max(1)
}

fn estimate_text_block(layer: &TextLayer) -> (u32, u32) {
    let max_chars = layer
        .text
        .split('\n')
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    let lines = layer.text.split('\n').count().max(1) as f32;
    let width = (max_chars * layer.font_size * 0.62).max(layer.font_size);
    let height = (lines * layer.font_size * 1.25).max(layer.font_size);
    (width.ceil() as u32, height.ceil() as u32)
}

fn draw_image_layer(canvas: &mut RgbaImage, layer: &ImageLayer) {
    let resized = layer
        .image
        .resize_exact(
            layer.width.max(1),
            layer.height.max(1),
            FilterType::Lanczos3,
        )
        .to_rgba8();
    blend_rgba(canvas, &resized, layer.x, layer.y, layer.opacity);
}

fn blend_rgba(canvas: &mut RgbaImage, overlay: &RgbaImage, x: i32, y: i32, opacity: f32) {
    let opacity = opacity.clamp(0.0, 1.0);
    for (overlay_x, overlay_y, pixel) in overlay.enumerate_pixels() {
        let target_x = x + overlay_x as i32;
        let target_y = y + overlay_y as i32;

        if target_x < 0
            || target_y < 0
            || target_x >= canvas.width() as i32
            || target_y >= canvas.height() as i32
        {
            continue;
        }

        let src_alpha = (f32::from(pixel[3]) / 255.0) * opacity;
        if src_alpha <= 0.0 {
            continue;
        }

        let dst = canvas.get_pixel_mut(target_x as u32, target_y as u32);
        let inv_alpha = 1.0 - src_alpha;
        dst[0] = (f32::from(pixel[0]) * src_alpha + f32::from(dst[0]) * inv_alpha).round() as u8;
        dst[1] = (f32::from(pixel[1]) * src_alpha + f32::from(dst[1]) * inv_alpha).round() as u8;
        dst[2] = (f32::from(pixel[2]) * src_alpha + f32::from(dst[2]) * inv_alpha).round() as u8;
        dst[3] = 255;
    }
}

fn color_with_opacity(mut color: [u8; 4], opacity: f32) -> [u8; 4] {
    color[3] = ((f32::from(color[3]) * opacity.clamp(0.0, 1.0)).round() as u8).max(1);
    color
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, RgbaImage};

    use super::{ComposeDocument, ComposeLayer, ImageLayer, render_composition};

    #[test]
    fn image_layer_renders_over_background() {
        let mut icon = RgbaImage::new(8, 8);
        for pixel in icon.pixels_mut() {
            *pixel = image::Rgba([255, 0, 0, 255]);
        }

        let document = ComposeDocument {
            background: DynamicImage::ImageRgba8(RgbaImage::new(20, 20)),
            layers: vec![ComposeLayer::Image(ImageLayer {
                name: "icon".to_owned(),
                image: DynamicImage::ImageRgba8(icon),
                x: 4,
                y: 5,
                width: 8,
                height: 8,
                opacity: 1.0,
                visible: true,
            })],
        };

        let rendered = render_composition(&document, None).to_rgba8();
        assert_eq!(rendered.get_pixel(4, 5), &image::Rgba([255, 0, 0, 255]));
    }
}
