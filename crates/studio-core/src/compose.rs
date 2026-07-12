use std::path::PathBuf;

use ab_glyph::{FontArc, PxScale};
use image::imageops::FilterType;
use image::{DynamicImage, Rgba, RgbaImage};
use imageproc::drawing::{
    draw_filled_rect_mut, draw_hollow_rect_mut, draw_line_segment_mut, draw_text_mut, text_size,
};
use imageproc::rect::Rect;
use serde::{Deserialize, Serialize};

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
    Shape(ShapeLayer),
}

impl ComposeLayer {
    pub fn name(&self) -> &str {
        match self {
            Self::Text(layer) => &layer.name,
            Self::Image(layer) => &layer.name,
            Self::Shape(layer) => &layer.name,
        }
    }

    pub fn name_mut(&mut self) -> &mut String {
        match self {
            Self::Text(layer) => &mut layer.name,
            Self::Image(layer) => &mut layer.name,
            Self::Shape(layer) => &mut layer.name,
        }
    }

    pub fn is_visible(&self) -> bool {
        match self {
            Self::Text(layer) => layer.visible,
            Self::Image(layer) => layer.visible,
            Self::Shape(layer) => layer.visible,
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        match self {
            Self::Text(layer) => layer.visible = visible,
            Self::Image(layer) => layer.visible = visible,
            Self::Shape(layer) => layer.visible = visible,
        }
    }
}

/// The vector annotation primitives supported by the quick annotation workflow.
///
/// Shape layers stay as data until export, so the app can move, recolor, resize,
/// and undo an annotation without losing the original pixels.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    Line,
    Arrow,
    Rectangle,
}

/// A serializable vector annotation layer.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ShapeLayer {
    pub name: String,
    pub kind: ShapeKind,
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub stroke_color: [u8; 4],
    pub stroke_width: f32,
    pub opacity: f32,
    pub filled: bool,
    pub fill_color: [u8; 4],
    pub visible: bool,
}

impl ShapeLayer {
    /// Build a shape with the quick-annotation defaults (red, 4 px, opaque).
    pub fn new(kind: ShapeKind, x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self {
            name: match kind {
                ShapeKind::Line => "Line",
                ShapeKind::Arrow => "Arrow",
                ShapeKind::Rectangle => "Rectangle",
            }
            .to_owned(),
            kind,
            x1,
            y1,
            x2,
            y2,
            stroke_color: [255, 0, 0, 255],
            stroke_width: 4.0,
            opacity: 1.0,
            filled: false,
            fill_color: [255, 0, 0, 64],
            visible: true,
        }
    }

    /// Return the pixel-space bounds, including half the stroke width.
    pub fn bounds(&self) -> LayerBounds {
        shape_layer_bounds(self)
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
            ComposeLayer::Shape(layer) if layer.visible => {
                draw_shape_layer(&mut canvas, layer);
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

/// Calculate the bounds for a vector shape, including the stroke footprint.
pub fn shape_layer_bounds(layer: &ShapeLayer) -> LayerBounds {
    let min_x = layer.x1.min(layer.x2);
    let min_y = layer.y1.min(layer.y2);
    let max_x = layer.x1.max(layer.x2);
    let max_y = layer.y1.max(layer.y2);
    let padding = (layer.stroke_width.max(1.0) / 2.0).ceil() as i32;
    let width = (max_x - min_x)
        .unsigned_abs()
        .saturating_add((padding * 2) as u32)
        .max(1);
    let height = (max_y - min_y)
        .unsigned_abs()
        .saturating_add((padding * 2) as u32)
        .max(1);

    LayerBounds {
        x: min_x - padding,
        y: min_y - padding,
        width,
        height,
    }
}

/// Calculate bounds for any composition layer. Image layers use their
/// configured destination rectangle; text and shapes use their content bounds.
pub fn compose_layer_bounds(layer: &ComposeLayer, font_bytes: Option<&[u8]>) -> LayerBounds {
    match layer {
        ComposeLayer::Text(layer) => text_layer_bounds(layer, font_bytes),
        ComposeLayer::Image(layer) => LayerBounds {
            x: layer.x,
            y: layer.y,
            width: layer.width.max(1),
            height: layer.height.max(1),
        },
        ComposeLayer::Shape(layer) => shape_layer_bounds(layer),
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

fn draw_shape_layer(canvas: &mut RgbaImage, layer: &ShapeLayer) {
    let stroke = Rgba(color_with_opacity(layer.stroke_color, layer.opacity));
    let fill = Rgba(color_with_opacity(layer.fill_color, layer.opacity));
    let width = layer.stroke_width.max(1.0).round() as u32;
    let min_x = layer.x1.min(layer.x2);
    let min_y = layer.y1.min(layer.y2);
    let max_x = layer.x1.max(layer.x2);
    let max_y = layer.y1.max(layer.y2);

    match layer.kind {
        ShapeKind::Line => draw_thick_line(
            canvas, layer.x1, layer.y1, layer.x2, layer.y2, stroke, width,
        ),
        ShapeKind::Arrow => {
            draw_thick_line(
                canvas, layer.x1, layer.y1, layer.x2, layer.y2, stroke, width,
            );
            draw_arrow_head(
                canvas, layer.x1, layer.y1, layer.x2, layer.y2, stroke, width,
            );
        }
        ShapeKind::Rectangle => {
            let rect_width = (max_x - min_x).unsigned_abs().max(1);
            let rect_height = (max_y - min_y).unsigned_abs().max(1);
            let rect = Rect::at(min_x, min_y).of_size(rect_width, rect_height);
            if layer.filled {
                draw_filled_rect_mut(canvas, rect, fill);
            }
            if width <= 1 {
                draw_hollow_rect_mut(canvas, rect, stroke);
            } else {
                // imageproc's hollow rectangle is one pixel wide; draw nested
                // rectangles to preserve the configured annotation thickness.
                for inset in 0..width {
                    let Some(inner_width) = rect_width.checked_sub(inset.saturating_mul(2)) else {
                        break;
                    };
                    let Some(inner_height) = rect_height.checked_sub(inset.saturating_mul(2))
                    else {
                        break;
                    };
                    if inner_width == 0 || inner_height == 0 {
                        break;
                    }
                    draw_hollow_rect_mut(
                        canvas,
                        Rect::at(min_x + inset as i32, min_y + inset as i32)
                            .of_size(inner_width, inner_height),
                        stroke,
                    );
                }
            }
        }
    }
}

fn draw_thick_line(
    canvas: &mut RgbaImage,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    color: Rgba<u8>,
    width: u32,
) {
    let width = width.max(1);
    let half = (width.saturating_sub(1) as f32) / 2.0;
    let dx = (x2 - x1) as f32;
    let dy = (y2 - y1) as f32;
    let length = (dx * dx + dy * dy).sqrt();
    let (normal_x, normal_y) = if length > f32::EPSILON {
        (-dy / length, dx / length)
    } else {
        (1.0, 0.0)
    };
    let steps = width.max(1);
    for index in 0..steps {
        let offset = index as f32 - half;
        draw_line_segment_mut(
            canvas,
            (x1 as f32 + normal_x * offset, y1 as f32 + normal_y * offset),
            (x2 as f32 + normal_x * offset, y2 as f32 + normal_y * offset),
            color,
        );
    }
}

fn draw_arrow_head(
    canvas: &mut RgbaImage,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    color: Rgba<u8>,
    width: u32,
) {
    let dx = (x2 - x1) as f32;
    let dy = (y2 - y1) as f32;
    let length = (dx * dx + dy * dy).sqrt();
    if length <= f32::EPSILON {
        return;
    }
    let ux = dx / length;
    let uy = dy / length;
    let head_length = (10.0 + width as f32 * 2.0).min(length.max(4.0));
    let head_width = head_length * 0.55;
    let base_x = x2 as f32 - ux * head_length;
    let base_y = y2 as f32 - uy * head_length;
    let left = (base_x - uy * head_width, base_y + ux * head_width);
    let right = (base_x + uy * head_width, base_y - ux * head_width);
    draw_thick_line(
        canvas,
        x2,
        y2,
        left.0.round() as i32,
        left.1.round() as i32,
        color,
        width,
    );
    draw_thick_line(
        canvas,
        x2,
        y2,
        right.0.round() as i32,
        right.1.round() as i32,
        color,
        width,
    );
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

    use super::{
        ComposeDocument, ComposeLayer, ImageLayer, ShapeKind, ShapeLayer, compose_layer_bounds,
        render_composition, shape_layer_bounds,
    };

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

    #[test]
    fn shape_layer_serializes_and_round_trips() {
        let shape = ShapeLayer::new(ShapeKind::Arrow, 2, 3, 18, 22);
        let json = serde_json::to_string(&shape).expect("shape should serialize");
        let restored: ShapeLayer = serde_json::from_str(&json).expect("shape should deserialize");
        assert_eq!(restored, shape);
        assert!(json.contains("\"kind\":\"arrow\""));
    }

    #[test]
    fn shape_bounds_include_stroke_and_normalize_coordinates() {
        let mut shape = ShapeLayer::new(ShapeKind::Rectangle, 20, 30, 5, 10);
        shape.stroke_width = 4.0;
        assert_eq!(shape_layer_bounds(&shape).x, 3);
        assert_eq!(shape_layer_bounds(&shape).y, 8);
        assert_eq!(shape_layer_bounds(&shape).width, 19);
        assert_eq!(shape_layer_bounds(&shape).height, 24);
    }

    #[test]
    fn shape_renders_line_arrow_and_rectangle() {
        let background = DynamicImage::ImageRgba8(RgbaImage::new(64, 64));
        let mut rectangle = ShapeLayer::new(ShapeKind::Rectangle, 5, 5, 24, 24);
        rectangle.stroke_width = 2.0;
        rectangle.stroke_color = [255, 0, 0, 255];
        let line = ShapeLayer::new(ShapeKind::Line, 0, 0, 30, 30);
        let arrow = ShapeLayer::new(ShapeKind::Arrow, 40, 40, 58, 40);
        let document = ComposeDocument {
            background,
            layers: vec![
                ComposeLayer::Shape(rectangle),
                ComposeLayer::Shape(line),
                ComposeLayer::Shape(arrow),
            ],
        };

        let rendered = render_composition(&document, None).to_rgba8();
        assert!(
            rendered.get_pixel(5, 5)[0] > 200,
            "rectangle stroke missing"
        );
        assert!(rendered.get_pixel(15, 15)[3] > 0, "line stroke missing");
        assert!(rendered.get_pixel(58, 40)[0] > 200, "arrow tip missing");
        assert_eq!(
            compose_layer_bounds(&document.layers[0], None),
            shape_layer_bounds(match &document.layers[0] {
                ComposeLayer::Shape(shape) => shape,
                _ => unreachable!(),
            })
        );
    }
}
