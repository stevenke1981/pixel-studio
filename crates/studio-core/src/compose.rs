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

/// Compositing blend mode applied when rendering a layer over the canvas.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    SoftLight,
    ColorDodge,
    Difference,
    Exclusion,
}

impl BlendMode {
    pub const ALL: [BlendMode; 10] = [
        Self::Normal,
        Self::Multiply,
        Self::Screen,
        Self::Overlay,
        Self::Darken,
        Self::Lighten,
        Self::SoftLight,
        Self::ColorDodge,
        Self::Difference,
        Self::Exclusion,
    ];

    /// Return the blend mode at `i`, defaulting to [`BlendMode::Normal`] when out of range.
    pub fn from_index(i: usize) -> Self {
        Self::ALL.get(i).copied().unwrap_or_default()
    }

    /// Return the 0-based index of this blend mode in [`BlendMode::ALL`].
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&mode| mode == self).unwrap_or(0)
    }

    /// Return the English display label for UI pickers.
    pub fn label_en(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Multiply => "Multiply",
            Self::Screen => "Screen",
            Self::Overlay => "Overlay",
            Self::Darken => "Darken",
            Self::Lighten => "Lighten",
            Self::SoftLight => "Soft Light",
            Self::ColorDodge => "Color Dodge",
            Self::Difference => "Difference",
            Self::Exclusion => "Exclusion",
        }
    }

    /// Return the Traditional Chinese (`zh-TW`) display label for UI pickers.
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Normal => "正常",
            Self::Multiply => "色彩增值",
            Self::Screen => "濾色",
            Self::Overlay => "覆蓋",
            Self::Darken => "變暗",
            Self::Lighten => "變亮",
            Self::SoftLight => "柔光",
            Self::ColorDodge => "加亮顏色",
            Self::Difference => "差異化",
            Self::Exclusion => "排除",
        }
    }
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

    /// Return the layer's blend mode.
    pub fn blend_mode(&self) -> BlendMode {
        match self {
            Self::Text(layer) => layer.blend_mode,
            Self::Image(layer) => layer.blend_mode,
            Self::Shape(layer) => layer.blend_mode,
        }
    }

    /// Update the layer's blend mode.
    pub fn set_blend_mode(&mut self, mode: BlendMode) {
        match self {
            Self::Text(layer) => layer.blend_mode = mode,
            Self::Image(layer) => layer.blend_mode = mode,
            Self::Shape(layer) => layer.blend_mode = mode,
        }
    }

    /// Return the layer's opacity in `[0.0, 1.0]`.
    pub fn opacity(&self) -> f32 {
        match self {
            Self::Text(layer) => layer.opacity,
            Self::Image(layer) => layer.opacity,
            Self::Shape(layer) => layer.opacity,
        }
    }

    /// Update the layer's opacity, clamped to `[0.0, 1.0]`.
    pub fn set_opacity(&mut self, opacity: f32) {
        let clamped = if opacity.is_nan() {
            1.0
        } else {
            opacity.clamp(0.0, 1.0)
        };
        match self {
            Self::Text(layer) => layer.opacity = clamped,
            Self::Image(layer) => layer.opacity = clamped,
            Self::Shape(layer) => layer.opacity = clamped,
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
    Ellipse,
    Triangle,
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
    #[serde(default)]
    pub blend_mode: BlendMode,
}

impl ShapeLayer {
    /// Build a shape with the quick-annotation defaults (red, 4 px, opaque).
    pub fn new(kind: ShapeKind, x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self {
            name: match kind {
                ShapeKind::Line => "Line",
                ShapeKind::Arrow => "Arrow",
                ShapeKind::Rectangle => "Rectangle",
                ShapeKind::Ellipse => "Ellipse",
                ShapeKind::Triangle => "Triangle",
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
            blend_mode: BlendMode::Normal,
        }
    }

    /// Return the pixel-space bounds, including half the stroke width.
    pub fn bounds(&self) -> LayerBounds {
        shape_layer_bounds(self)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
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
    #[serde(default)]
    pub blend_mode: BlendMode,
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
            blend_mode: BlendMode::Normal,
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
    /// Set when this layer is a recolorable built-in sticker.
    pub sticker: Option<StickerInstance>,
    pub blend_mode: BlendMode,
}

/// Sticker identity and style; the app re-rasterizes the layer when these or the size change.
#[derive(Clone, Debug, PartialEq)]
pub struct StickerInstance {
    pub id: String,
    pub style: crate::stickers::StickerStyle,
    /// (width, height, style) of the last rasterization.
    pub rendered: Option<(u32, u32, crate::stickers::StickerStyle)>,
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
            sticker: None,
            blend_mode: BlendMode::Normal,
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

/// Flip a composition layer horizontally around its own center.
///
/// Image layers mirror their pixel buffer in-place; shape layers mirror their
/// endpoints around their horizontal midpoint; text layers are left unchanged.
pub fn flip_layer_horizontal(layer: &mut ComposeLayer) {
    match layer {
        ComposeLayer::Image(img) => {
            img.image = img.image.fliph();
        }
        ComposeLayer::Shape(shape) => {
            std::mem::swap(&mut shape.x1, &mut shape.x2);
        }
        ComposeLayer::Text(_) => {}
    }
}

/// Flip a composition layer vertically around its own center.
///
/// Image layers mirror their pixel buffer in-place; shape layers mirror their
/// endpoints around their vertical midpoint; text layers are left unchanged.
pub fn flip_layer_vertical(layer: &mut ComposeLayer) {
    match layer {
        ComposeLayer::Image(img) => {
            img.image = img.image.flipv();
        }
        ComposeLayer::Shape(shape) => {
            std::mem::swap(&mut shape.y1, &mut shape.y2);
        }
        ComposeLayer::Text(_) => {}
    }
}

/// Rotate a composition layer 90 degrees clockwise or counter-clockwise around its center.
///
/// Image layers rotate their pixel buffer and swap `width` and `height` while
/// preserving center coordinates; shape layers rotate their endpoints 90 degrees
/// around their midpoint; text layers are left unchanged.
pub fn rotate_layer_90(layer: &mut ComposeLayer, clockwise: bool) {
    match layer {
        ComposeLayer::Image(img) => {
            img.image = if clockwise {
                img.image.rotate90()
            } else {
                img.image.rotate270()
            };
            let old_w = img.width.max(1);
            let old_h = img.height.max(1);
            img.width = old_h;
            img.height = old_w;
            img.x += (old_w as i32 - img.width as i32) / 2;
            img.y += (old_h as i32 - img.height as i32) / 2;
        }
        ComposeLayer::Shape(shape) => {
            let sx = i64::from(shape.x1) + i64::from(shape.x2);
            let sy = i64::from(shape.y1) + i64::from(shape.y2);
            let dx = i64::from(shape.x1) - i64::from(shape.x2);
            let dy = i64::from(shape.y1) - i64::from(shape.y2);
            let (nx1_2, ny1_2, nx2_2, ny2_2) = if clockwise {
                (sx - dy, sy + dx, sx + dy, sy - dx)
            } else {
                (sx + dy, sy - dx, sx - dy, sy + dx)
            };
            shape.x1 = (nx1_2 + 1).div_euclid(2) as i32;
            shape.y1 = (ny1_2 + 1).div_euclid(2) as i32;
            shape.x2 = (nx2_2 + 1).div_euclid(2) as i32;
            shape.y2 = (ny2_2 + 1).div_euclid(2) as i32;
        }
        ComposeLayer::Text(_) => {}
    }
}

fn draw_text_layer(canvas: &mut RgbaImage, layer: &TextLayer, font: &FontArc) {
    if layer.opacity <= 0.0 || layer.opacity.is_nan() {
        return;
    }
    let (tw, th) = measure_text_block(font, layer.font_size, &layer.text);
    let margin = (layer.font_size.max(1.0) * 0.5).ceil() as i32 + 16;
    let patch_x0 = (layer.x - margin).max(0);
    let patch_y0 = (layer.y - margin).max(0);
    let patch_x1 = layer
        .x
        .saturating_add(tw as i32)
        .saturating_add(margin)
        .min(canvas.width() as i32);
    let patch_y1 = layer
        .y
        .saturating_add(th as i32)
        .saturating_add(margin)
        .min(canvas.height() as i32);
    if patch_x0 >= patch_x1 || patch_y0 >= patch_y1 {
        return;
    }

    let mut patch = RgbaImage::new((patch_x1 - patch_x0) as u32, (patch_y1 - patch_y0) as u32);
    let scale = PxScale::from(layer.font_size.max(1.0));
    let line_height = line_height(font, scale);
    let local_x = layer.x - patch_x0;

    for (line_index, line) in layer.text.split('\n').enumerate() {
        let local_y = (layer.y - patch_y0) + (line_index as i32 * line_height as i32);

        if layer.shadow {
            draw_text_mut(
                &mut patch,
                Rgba([0, 0, 0, 180]),
                local_x + 3,
                local_y + 3,
                scale,
                font,
                line,
            );
        }

        if layer.stroke {
            let stroke_color = Rgba([0, 0, 0, 220]);
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                draw_text_mut(
                    &mut patch,
                    stroke_color,
                    local_x + dx,
                    local_y + dy,
                    scale,
                    font,
                    line,
                );
            }
        }

        draw_text_mut(
            &mut patch,
            Rgba(layer.color),
            local_x,
            local_y,
            scale,
            font,
            line,
        );
    }

    blend_rgba_mode(
        canvas,
        &patch,
        patch_x0,
        patch_y0,
        layer.opacity,
        layer.blend_mode,
    );
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
    if layer.opacity <= 0.0 || layer.opacity.is_nan() {
        return;
    }
    let target_w = layer.width.max(1);
    let target_h = layer.height.max(1);
    let rgba = if layer.image.width() == target_w && layer.image.height() == target_h {
        layer.image.to_rgba8()
    } else {
        layer
            .image
            .resize_exact(target_w, target_h, FilterType::Lanczos3)
            .to_rgba8()
    };
    blend_rgba_mode(
        canvas,
        &rgba,
        layer.x,
        layer.y,
        layer.opacity,
        layer.blend_mode,
    );
}

fn draw_shape_layer(canvas: &mut RgbaImage, layer: &ShapeLayer) {
    if layer.opacity <= 0.0 || layer.opacity.is_nan() {
        return;
    }
    let min_x = layer.x1.min(layer.x2);
    let min_y = layer.y1.min(layer.y2);
    let max_x = layer.x1.max(layer.x2);
    let max_y = layer.y1.max(layer.y2);
    let width = layer.stroke_width.max(1.0).round() as u32;

    let mut pad = width as i32 + 4;
    if layer.kind == ShapeKind::Arrow {
        let head_len = (10.0 + width as f32 * 2.0).ceil() as i32;
        pad += head_len + 4;
    }

    let patch_x0 = (min_x - pad).max(0);
    let patch_y0 = (min_y - pad).max(0);
    let patch_x1 = max_x
        .saturating_add(pad)
        .saturating_add(1)
        .min(canvas.width() as i32);
    let patch_y1 = max_y
        .saturating_add(pad)
        .saturating_add(1)
        .min(canvas.height() as i32);
    if patch_x0 >= patch_x1 || patch_y0 >= patch_y1 {
        return;
    }

    let patch_w = (patch_x1 - patch_x0) as u32;
    let patch_h = (patch_y1 - patch_y0) as u32;
    let mut patch = RgbaImage::new(patch_w, patch_h);

    if layer.filled && layer.fill_color[3] > 0 {
        draw_shape_fill(&mut patch, layer, patch_x0, patch_y0);
    }

    if layer.stroke_color[3] > 0 {
        let stroke = Rgba(layer.stroke_color);
        if layer.filled && layer.fill_color[3] > 0 && layer.stroke_color[3] < 255 {
            let mut stroke_patch = RgbaImage::new(patch_w, patch_h);
            draw_shape_stroke(&mut stroke_patch, layer, patch_x0, patch_y0, stroke, width);
            blend_rgba_mode(&mut patch, &stroke_patch, 0, 0, 1.0, BlendMode::Normal);
        } else {
            draw_shape_stroke(&mut patch, layer, patch_x0, patch_y0, stroke, width);
        }
    }

    blend_rgba_mode(
        canvas,
        &patch,
        patch_x0,
        patch_y0,
        layer.opacity,
        layer.blend_mode,
    );
}

fn draw_shape_fill(patch: &mut RgbaImage, layer: &ShapeLayer, offset_x: i32, offset_y: i32) {
    let min_x = layer.x1.min(layer.x2);
    let min_y = layer.y1.min(layer.y2);
    let max_x = layer.x1.max(layer.x2);
    let max_y = layer.y1.max(layer.y2);
    let fill = Rgba(layer.fill_color);

    match layer.kind {
        ShapeKind::Ellipse => {
            let (cx, cy) = (
                (min_x + max_x) / 2 - offset_x,
                (min_y + max_y) / 2 - offset_y,
            );
            let (rx, ry) = (((max_x - min_x) / 2).max(1), ((max_y - min_y) / 2).max(1));
            imageproc::drawing::draw_filled_ellipse_mut(patch, (cx, cy), rx, ry, fill);
        }
        ShapeKind::Triangle => {
            if max_x > min_x && max_y > min_y {
                let top = ((min_x + max_x) / 2 - offset_x, min_y - offset_y);
                let bl = (min_x - offset_x, max_y - offset_y);
                let br = (max_x - offset_x, max_y - offset_y);
                let pts = [
                    imageproc::point::Point::new(top.0, top.1),
                    imageproc::point::Point::new(br.0, br.1),
                    imageproc::point::Point::new(bl.0, bl.1),
                ];
                imageproc::drawing::draw_polygon_mut(patch, &pts, fill);
            }
        }
        ShapeKind::Rectangle => {
            let rect_width = (max_x - min_x).unsigned_abs().max(1);
            let rect_height = (max_y - min_y).unsigned_abs().max(1);
            let rect =
                Rect::at(min_x - offset_x, min_y - offset_y).of_size(rect_width, rect_height);
            draw_filled_rect_mut(patch, rect, fill);
        }
        ShapeKind::Line | ShapeKind::Arrow => {}
    }
}

fn draw_shape_stroke(
    patch: &mut RgbaImage,
    layer: &ShapeLayer,
    offset_x: i32,
    offset_y: i32,
    stroke: Rgba<u8>,
    width: u32,
) {
    let min_x = layer.x1.min(layer.x2);
    let min_y = layer.y1.min(layer.y2);
    let max_x = layer.x1.max(layer.x2);
    let max_y = layer.y1.max(layer.y2);

    match layer.kind {
        ShapeKind::Ellipse => {
            let (cx, cy) = (
                (min_x + max_x) / 2 - offset_x,
                (min_y + max_y) / 2 - offset_y,
            );
            let (rx, ry) = (((max_x - min_x) / 2).max(1), ((max_y - min_y) / 2).max(1));
            for i in 0..width as i32 {
                if rx - i < 1 || ry - i < 1 {
                    break;
                }
                imageproc::drawing::draw_hollow_ellipse_mut(
                    patch,
                    (cx, cy),
                    rx - i,
                    ry - i,
                    stroke,
                );
            }
        }
        ShapeKind::Triangle => {
            let top = ((min_x + max_x) / 2 - offset_x, min_y - offset_y);
            let bl = (min_x - offset_x, max_y - offset_y);
            let br = (max_x - offset_x, max_y - offset_y);
            for (a, b) in [(top, br), (br, bl), (bl, top)] {
                draw_thick_line(patch, a.0, a.1, b.0, b.1, stroke, width);
            }
        }
        ShapeKind::Line => draw_thick_line(
            patch,
            layer.x1 - offset_x,
            layer.y1 - offset_y,
            layer.x2 - offset_x,
            layer.y2 - offset_y,
            stroke,
            width,
        ),
        ShapeKind::Arrow => {
            let x1 = layer.x1 - offset_x;
            let y1 = layer.y1 - offset_y;
            let x2 = layer.x2 - offset_x;
            let y2 = layer.y2 - offset_y;
            draw_thick_line(patch, x1, y1, x2, y2, stroke, width);
            draw_arrow_head(patch, x1, y1, x2, y2, stroke, width);
        }
        ShapeKind::Rectangle => {
            let rect_width = (max_x - min_x).unsigned_abs().max(1);
            let rect_height = (max_y - min_y).unsigned_abs().max(1);
            let lx = min_x - offset_x;
            let ly = min_y - offset_y;
            let rect = Rect::at(lx, ly).of_size(rect_width, rect_height);
            if width <= 1 {
                draw_hollow_rect_mut(patch, rect, stroke);
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
                        patch,
                        Rect::at(lx + inset as i32, ly + inset as i32)
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

#[inline]
fn blend_channel(b: f32, s: f32, mode: BlendMode) -> f32 {
    match mode {
        BlendMode::Normal => s,
        BlendMode::Multiply => b * s,
        BlendMode::Screen => b + s - b * s,
        BlendMode::Overlay => {
            if b <= 0.5 {
                2.0 * b * s
            } else {
                1.0 - 2.0 * (1.0 - b) * (1.0 - s)
            }
        }
        BlendMode::Darken => b.min(s),
        BlendMode::Lighten => b.max(s),
        BlendMode::SoftLight => {
            if s <= 0.5 {
                b - (1.0 - 2.0 * s) * b * (1.0 - b)
            } else {
                let d = if b <= 0.25 {
                    ((16.0 * b - 12.0) * b + 4.0) * b
                } else {
                    b.sqrt()
                };
                b + (2.0 * s - 1.0) * (d - b)
            }
        }
        BlendMode::ColorDodge => {
            if b <= 0.0 {
                0.0
            } else if s >= 1.0 {
                1.0
            } else {
                (b / (1.0 - s)).min(1.0)
            }
        }
        BlendMode::Difference => (b - s).abs(),
        BlendMode::Exclusion => b + s - 2.0 * b * s,
    }
    .clamp(0.0, 1.0)
}

fn blend_rgba_mode(
    canvas: &mut RgbaImage,
    overlay: &RgbaImage,
    x: i32,
    y: i32,
    opacity: f32,
    mode: BlendMode,
) {
    let opacity = if opacity.is_nan() {
        0.0
    } else {
        opacity.clamp(0.0, 1.0)
    };
    if opacity <= 0.0 {
        return;
    }

    let cw = canvas.width() as i32;
    let ch = canvas.height() as i32;
    let ow = overlay.width() as i32;
    let oh = overlay.height() as i32;

    let start_ox = (0 - x).max(0);
    let start_oy = (0 - y).max(0);
    let end_ox = (cw - x).min(ow);
    let end_oy = (ch - y).min(oh);
    if start_ox >= end_ox || start_oy >= end_oy {
        return;
    }

    for oy in start_oy..end_oy {
        let ty = (y + oy) as u32;
        for ox in start_ox..end_ox {
            let pixel = overlay.get_pixel(ox as u32, oy as u32);
            let sa = (f32::from(pixel[3]) / 255.0) * opacity;
            if sa <= 0.0 {
                continue;
            }

            let tx = (x + ox) as u32;
            let dst = canvas.get_pixel_mut(tx, ty);
            let da = f32::from(dst[3]) / 255.0;

            let sr = f32::from(pixel[0]) / 255.0;
            let sg = f32::from(pixel[1]) / 255.0;
            let sb = f32::from(pixel[2]) / 255.0;

            if da <= 0.0 {
                dst[0] = pixel[0];
                dst[1] = pixel[1];
                dst[2] = pixel[2];
                dst[3] = (sa * 255.0).round().clamp(0.0, 255.0) as u8;
                continue;
            }

            let dr = f32::from(dst[0]) / 255.0;
            let dg = f32::from(dst[1]) / 255.0;
            let db = f32::from(dst[2]) / 255.0;

            let br = blend_channel(dr, sr, mode);
            let bg = blend_channel(dg, sg, mode);
            let bb = blend_channel(db, sb, mode);

            // W3C Compositing and Blending Level 1 general alpha-compositing equation.
            let cr = (1.0 - da) * sr + da * br;
            let cg = (1.0 - da) * sg + da * bg;
            let cb = (1.0 - da) * sb + da * bb;

            let out_a = sa + da * (1.0 - sa);
            if out_a <= 0.0 {
                *dst = Rgba([0, 0, 0, 0]);
                continue;
            }

            let out_r = (sa * cr + da * (1.0 - sa) * dr) / out_a;
            let out_g = (sa * cg + da * (1.0 - sa) * dg) / out_a;
            let out_b = (sa * cb + da * (1.0 - sa) * db) / out_a;

            dst[0] = (out_r * 255.0).round().clamp(0.0, 255.0) as u8;
            dst[1] = (out_g * 255.0).round().clamp(0.0, 255.0) as u8;
            dst[2] = (out_b * 255.0).round().clamp(0.0, 255.0) as u8;
            dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, Rgba, RgbaImage};

    use super::{
        BlendMode, ComposeDocument, ComposeLayer, ImageLayer, ShapeKind, ShapeLayer, TextLayer,
        compose_layer_bounds, flip_layer_horizontal, flip_layer_vertical, render_composition,
        rotate_layer_90, shape_layer_bounds,
    };

    fn solid_image(w: u32, h: u32, rgba: [u8; 4]) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, Rgba(rgba)))
    }

    fn load_test_font() -> Option<Vec<u8>> {
        [
            r"C:\Windows\Fonts\arial.ttf",
            r"C:\Windows\Fonts\segoeui.ttf",
            r"C:\Windows\Fonts\msjh.ttc",
            "/System/Library/Fonts/Helvetica.ttc",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ]
        .into_iter()
        .find_map(|path| std::fs::read(path).ok())
    }

    fn render_blend_pixel(bg: [u8; 4], fg: [u8; 4], opacity: f32, mode: BlendMode) -> [u8; 4] {
        let mut layer = ImageLayer::new("fg".to_owned(), solid_image(4, 4, fg));
        layer.x = 0;
        layer.y = 0;
        layer.opacity = opacity;
        layer.blend_mode = mode;

        let doc = ComposeDocument {
            background: solid_image(4, 4, bg),
            layers: vec![ComposeLayer::Image(layer)],
        };
        render_composition(&doc, None).to_rgba8().get_pixel(1, 1).0
    }

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
                sticker: None,
                blend_mode: BlendMode::Normal,
            })],
        };

        let rendered = render_composition(&document, None).to_rgba8();
        assert_eq!(rendered.get_pixel(4, 5), &image::Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn shape_layer_serializes_and_round_trips() {
        let mut shape = ShapeLayer::new(ShapeKind::Arrow, 2, 3, 18, 22);
        shape.blend_mode = BlendMode::SoftLight;
        let json = serde_json::to_string(&shape).expect("shape should serialize");
        let restored: ShapeLayer = serde_json::from_str(&json).expect("shape should deserialize");
        assert_eq!(restored, shape);
        assert!(json.contains("\"kind\":\"arrow\""));
        assert!(json.contains("\"blend_mode\":\"soft_light\""));
    }

    #[test]
    fn serde_backward_compatibility_defaults_missing_blend_mode() {
        let legacy_shape_json = r#"{
            "name":"Rect",
            "kind":"rectangle",
            "x1":1,"y1":2,"x2":10,"y2":12,
            "stroke_color":[255,0,0,255],
            "stroke_width":2.0,
            "opacity":0.8,
            "filled":false,
            "fill_color":[0,0,0,0],
            "visible":true
        }"#;
        let restored_shape: ShapeLayer =
            serde_json::from_str(legacy_shape_json).expect("legacy shape json should deserialize");
        assert_eq!(restored_shape.blend_mode, BlendMode::Normal);

        let legacy_text_json = r#"{
            "name":"Title",
            "text":"Hello",
            "font_path":null,
            "x":10,"y":20,
            "font_size":32.0,
            "color":[255,255,255,255],
            "opacity":1.0,
            "stroke":true,
            "shadow":false,
            "visible":true
        }"#;
        let restored_text: TextLayer =
            serde_json::from_str(legacy_text_json).expect("legacy text json should deserialize");
        assert_eq!(restored_text.blend_mode, BlendMode::Normal);
    }

    #[test]
    fn blend_mode_metadata_and_indexing() {
        assert_eq!(BlendMode::ALL.len(), 10);
        assert_eq!(BlendMode::default(), BlendMode::Normal);
        for (idx, &mode) in BlendMode::ALL.iter().enumerate() {
            assert_eq!(mode.index(), idx);
            assert_eq!(BlendMode::from_index(idx), mode);
            assert!(!mode.label_en().is_empty());
            assert!(!mode.label_zh().is_empty());
        }
        assert_eq!(BlendMode::from_index(999), BlendMode::Normal);
    }

    #[test]
    fn compose_layer_accessors_for_blend_mode_and_opacity() {
        let mut layers = [
            ComposeLayer::Text(TextLayer::default()),
            ComposeLayer::Image(ImageLayer::new("img".to_owned(), solid_image(4, 4, [0; 4]))),
            ComposeLayer::Shape(ShapeLayer::new(ShapeKind::Line, 0, 0, 5, 5)),
        ];
        for layer in &mut layers {
            assert_eq!(layer.blend_mode(), BlendMode::Normal);
            assert!((layer.opacity() - 1.0).abs() < 1e-6);

            layer.set_blend_mode(BlendMode::Difference);
            assert_eq!(layer.blend_mode(), BlendMode::Difference);

            layer.set_opacity(0.45);
            assert!((layer.opacity() - 0.45).abs() < 1e-6);

            layer.set_opacity(2.5);
            assert!((layer.opacity() - 1.0).abs() < 1e-6);

            layer.set_opacity(-0.5);
            assert!((layer.opacity() - 0.0).abs() < 1e-6);
        }
    }

    #[test]
    fn every_blend_mode_produces_expected_output() {
        // Test pair exercising both <= 0.5 and > 0.5 branches across channels.
        let bg = [64, 192, 128, 255];
        let fg = [128, 64, 192, 255];

        assert_eq!(
            render_blend_pixel(bg, fg, 1.0, BlendMode::Normal),
            [128, 64, 192, 255]
        );

        // Multiply: (64*128)/255 = 32, (192*64)/255 = 48, (128*192)/255 = 96
        let mul = render_blend_pixel(bg, fg, 1.0, BlendMode::Multiply);
        assert!((mul[0] as i32 - 32).abs() <= 1 && (mul[1] as i32 - 48).abs() <= 1);
        assert!((mul[2] as i32 - 96).abs() <= 1);

        // Screen: 1 - (1-b)(1-s)
        let scr = render_blend_pixel(bg, fg, 1.0, BlendMode::Screen);
        assert!((scr[0] as i32 - 160).abs() <= 1);
        assert!((scr[1] as i32 - 208).abs() <= 1);
        assert!((scr[2] as i32 - 224).abs() <= 1);

        // Overlay: b<=0.5 -> 2bs, b>0.5 -> 1 - 2(1-b)(1-s)
        let ovl = render_blend_pixel(bg, fg, 1.0, BlendMode::Overlay);
        assert!((ovl[0] as i32 - 64).abs() <= 1); // 2 * (64/255) * (128/255) * 255 ≈ 64
        assert!((ovl[1] as i32 - 161).abs() <= 1); // 255*(1 - 2*(63/255)*(191/255)) ≈ 161
        assert!((ovl[2] as i32 - 192).abs() <= 1); // 255*(1 - 2*(127/255)*(63/255)) ≈ 192

        // Darken & Lighten
        assert_eq!(
            render_blend_pixel(bg, fg, 1.0, BlendMode::Darken),
            [64, 64, 128, 255]
        );
        assert_eq!(
            render_blend_pixel(bg, fg, 1.0, BlendMode::Lighten),
            [128, 192, 192, 255]
        );

        // SoftLight: for b=128/255 ≈ 0.502, s=64/255 ≈ 0.251 (darkens); s=192/255 ≈ 0.753 (lightens)
        let soft = render_blend_pixel(
            [128, 128, 32, 255],
            [64, 192, 200, 255],
            1.0,
            BlendMode::SoftLight,
        );
        assert!(soft[0] < 128, "SoftLight with dark source should darken");
        assert!(soft[1] > 128, "SoftLight with bright source should lighten");
        assert!(soft[2] > 32, "SoftLight <=0.25 branch should lighten");

        // ColorDodge: b=100, s=128 -> ~201; b=0 stays 0; s=255 -> 255 (when b>0)
        let dodge = render_blend_pixel(
            [100, 0, 200, 255],
            [128, 255, 255, 255],
            1.0,
            BlendMode::ColorDodge,
        );
        assert!((dodge[0] as i32 - 201).abs() <= 2);
        assert_eq!(dodge[1], 0);
        assert_eq!(dodge[2], 255);

        // Difference: |b - s|
        assert_eq!(
            render_blend_pixel(bg, fg, 1.0, BlendMode::Difference),
            [64, 128, 64, 255]
        );

        // Exclusion: b + s - 2*b*s
        let excl = render_blend_pixel(bg, fg, 1.0, BlendMode::Exclusion);
        assert!((excl[0] as i32 - 128).abs() <= 1);
        assert!((excl[1] as i32 - 160).abs() <= 1);
        assert!((excl[2] as i32 - 128).abs() <= 2);

        // Opacity 0.0 leaves background unchanged, 0.5 interpolates, transparent bg preserves alpha.
        assert_eq!(render_blend_pixel(bg, fg, 0.0, BlendMode::Multiply), bg);
        let half_mul = render_blend_pixel(
            [200, 100, 50, 255],
            [0, 255, 255, 255],
            0.5,
            BlendMode::Multiply,
        );
        assert!((half_mul[0] as i32 - 100).abs() <= 1);
        assert_eq!(half_mul[1], 100);
        assert_eq!(
            render_blend_pixel([0, 0, 0, 0], [200, 100, 50, 255], 0.5, BlendMode::Multiply),
            [200, 100, 50, 128]
        );
    }

    #[test]
    fn shape_layer_blends_without_double_blending_overlap() {
        let mut rect = ShapeLayer::new(ShapeKind::Rectangle, 4, 4, 24, 24);
        rect.filled = true;
        rect.fill_color = [255, 255, 255, 255];
        rect.stroke_color = [255, 0, 0, 255];
        rect.stroke_width = 4.0;
        rect.opacity = 0.5;
        rect.blend_mode = BlendMode::Normal;

        let doc = ComposeDocument {
            background: solid_image(32, 32, [0, 0, 0, 255]),
            layers: vec![ComposeLayer::Shape(rect.clone())],
        };
        let rendered = render_composition(&doc, None).to_rgba8();
        // Overlapping thick stroke corners must remain uniformly 50% red (no double-blend).
        let corner = rendered.get_pixel(4, 4).0;
        let edge = rendered.get_pixel(14, 4).0;
        assert_eq!(corner, edge);
        assert!((corner[0] as i32 - 128).abs() <= 1);

        // Difference blend mode on filled shape over white inverts fill & stroke properly.
        rect.opacity = 1.0;
        rect.blend_mode = BlendMode::Difference;
        let doc_diff = ComposeDocument {
            background: solid_image(32, 32, [255, 255, 255, 255]),
            layers: vec![ComposeLayer::Shape(rect)],
        };
        let diff_out = render_composition(&doc_diff, None).to_rgba8();
        assert_eq!(diff_out.get_pixel(14, 14).0, [0, 0, 0, 255]); // |255 - 255|
        assert_eq!(diff_out.get_pixel(4, 4).0, [0, 255, 255, 255]); // |255 - [255,0,0]|
    }

    #[test]
    fn text_layer_honors_blend_mode_and_opacity() {
        let Some(font_bytes) = load_test_font() else {
            return;
        };
        let bg = [180, 120, 90, 255];
        let text = TextLayer {
            name: "test".to_owned(),
            text: "M".to_owned(),
            font_path: None,
            x: 8,
            y: 8,
            font_size: 48.0,
            color: [255, 255, 255, 255],
            opacity: 1.0,
            stroke: false,
            shadow: false,
            visible: true,
            blend_mode: BlendMode::Difference,
        };

        let doc = ComposeDocument {
            background: solid_image(80, 80, bg),
            layers: vec![ComposeLayer::Text(text.clone())],
        };
        let rendered = render_composition(&doc, Some(&font_bytes)).to_rgba8();

        // Difference of white glyph against [180, 120, 90] yields [75, 135, 165] inside glyph.
        let expected_glyph = [255 - bg[0], 255 - bg[1], 255 - bg[2], 255];
        let has_expected = rendered.pixels().any(|p| {
            (p[0] as i32 - expected_glyph[0] as i32).abs() <= 2
                && (p[1] as i32 - expected_glyph[1] as i32).abs() <= 2
                && (p[2] as i32 - expected_glyph[2] as i32).abs() <= 2
        });
        assert!(has_expected, "expected Difference-blended text pixels");

        // Text with stroke + shadow at opacity 0.5 + Multiply never exceeds 50% opacity darkening
        // (and with white glyph over [200, 200, 200], Multiply keeps interior white unchanged).
        let mut styled = text;
        styled.stroke = true;
        styled.shadow = true;
        styled.opacity = 0.5;
        styled.blend_mode = BlendMode::Multiply;
        let doc_styled = ComposeDocument {
            background: solid_image(80, 80, [200, 200, 200, 255]),
            layers: vec![ComposeLayer::Text(styled)],
        };
        let rendered_styled = render_composition(&doc_styled, Some(&font_bytes)).to_rgba8();
        // Stroke/shadow pixels at 50% black Multiply over [200,200,200] should be >= 98 (never darker from double-blend).
        assert!(
            rendered_styled.pixels().all(|p| p[0] >= 98),
            "text stroke/shadow must not double-blend below 50% opacity"
        );
        assert!(
            rendered_styled.pixels().any(|p| p[0] < 150),
            "expected stroke/shadow darkening from styled text"
        );
    }

    #[test]
    fn flip_layer_horizontal_and_vertical_work_for_all_variants() {
        let mut img_buf = RgbaImage::new(2, 2);
        img_buf.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        img_buf.put_pixel(1, 0, Rgba([0, 255, 0, 255]));
        img_buf.put_pixel(0, 1, Rgba([0, 0, 255, 255]));
        img_buf.put_pixel(1, 1, Rgba([255, 255, 255, 255]));

        let mut img_layer = ComposeLayer::Image(ImageLayer::new(
            "quad".to_owned(),
            DynamicImage::ImageRgba8(img_buf),
        ));
        flip_layer_horizontal(&mut img_layer);
        if let ComposeLayer::Image(img) = &img_layer {
            let rgba = img.image.to_rgba8();
            assert_eq!(rgba.get_pixel(0, 0).0, [0, 255, 0, 255]);
            assert_eq!(rgba.get_pixel(1, 0).0, [255, 0, 0, 255]);
        } else {
            unreachable!();
        }
        flip_layer_vertical(&mut img_layer);
        if let ComposeLayer::Image(img) = &img_layer {
            let rgba = img.image.to_rgba8();
            assert_eq!(rgba.get_pixel(0, 0).0, [255, 255, 255, 255]);
            assert_eq!(rgba.get_pixel(1, 0).0, [0, 0, 255, 255]);
        } else {
            unreachable!();
        }

        let mut shape_layer =
            ComposeLayer::Shape(ShapeLayer::new(ShapeKind::Arrow, 10, 20, 50, 40));
        flip_layer_horizontal(&mut shape_layer);
        if let ComposeLayer::Shape(s) = &shape_layer {
            assert_eq!((s.x1, s.y1, s.x2, s.y2), (50, 20, 10, 40));
        }
        flip_layer_vertical(&mut shape_layer);
        if let ComposeLayer::Shape(s) = &shape_layer {
            assert_eq!((s.x1, s.y1, s.x2, s.y2), (50, 40, 10, 20));
        }

        let text_orig = TextLayer::default();
        let mut text_layer = ComposeLayer::Text(text_orig.clone());
        flip_layer_horizontal(&mut text_layer);
        flip_layer_vertical(&mut text_layer);
        if let ComposeLayer::Text(t) = &text_layer {
            assert_eq!(t, &text_orig);
        }
    }

    #[test]
    fn rotate_layer_90_preserves_center_and_roundtrips() {
        let mut buf = RgbaImage::new(11, 6);
        buf.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        buf.put_pixel(10, 5, Rgba([0, 255, 0, 255]));
        let mut img = ImageLayer::new("rect".to_owned(), DynamicImage::ImageRgba8(buf));
        img.x = 10;
        img.y = 20;
        let orig_buf = img.image.to_rgba8();
        let mut img_layer = ComposeLayer::Image(img);

        rotate_layer_90(&mut img_layer, true);
        if let ComposeLayer::Image(rotated) = &img_layer {
            assert_eq!((rotated.width, rotated.height), (6, 11));
            assert_eq!((rotated.x, rotated.y), (12, 18));
            assert_eq!(rotated.image.to_rgba8().get_pixel(5, 0).0, [255, 0, 0, 255]);
        }

        // CW followed by CCW restores exact original position, size, and pixels.
        rotate_layer_90(&mut img_layer, false);
        if let ComposeLayer::Image(restored) = &img_layer {
            assert_eq!(
                (restored.x, restored.y, restored.width, restored.height),
                (10, 20, 11, 6)
            );
            assert_eq!(restored.image.to_rgba8(), orig_buf);
        }

        // 4 clockwise rotations also restore exact original.
        for _ in 0..4 {
            rotate_layer_90(&mut img_layer, true);
        }
        if let ComposeLayer::Image(restored) = &img_layer {
            assert_eq!(
                (restored.x, restored.y, restored.width, restored.height),
                (10, 20, 11, 6)
            );
            assert_eq!(restored.image.to_rgba8(), orig_buf);
        }

        // Arrow pointing right (10,20 -> 30,20): CW points down (20,10 -> 20,30), CCW from orig points up.
        let mut arrow = ComposeLayer::Shape(ShapeLayer::new(ShapeKind::Arrow, 10, 20, 30, 20));
        rotate_layer_90(&mut arrow, true);
        if let ComposeLayer::Shape(s) = &arrow {
            assert_eq!((s.x1, s.y1, s.x2, s.y2), (20, 10, 20, 30));
        }
        rotate_layer_90(&mut arrow, false);
        if let ComposeLayer::Shape(s) = &arrow {
            assert_eq!((s.x1, s.y1, s.x2, s.y2), (10, 20, 30, 20));
        }
        rotate_layer_90(&mut arrow, false);
        if let ComposeLayer::Shape(s) = &arrow {
            assert_eq!((s.x1, s.y1, s.x2, s.y2), (20, 30, 20, 10));
        }
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
