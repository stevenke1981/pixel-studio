//! UI-independent document model: pixels, layers, selection, undo/redo, tools,
//! stickers and redaction. The Slint front-end only forwards input and displays
//! the composite; everything here is unit-testable.

use std::path::PathBuf;

use base64::{Engine as _, engine::general_purpose};
use image::{DynamicImage, Rgba, RgbaImage};
use studio_core::compose::StickerInstance;
use studio_core::redact::{RedactBox, RedactStyle, apply_redactions, redact_brush_dab};
use studio_core::stickers::{self, Sticker, StickerStyle};
use studio_core::transform::SelectionMask;
use studio_core::{
    ComposeDocument, ComposeLayer, ImageLayer, ShapeKind, ShapeLayer, TextLayer,
    compose_layer_bounds, render_composition,
};

const MAX_UNDO: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    Select,
    Pencil,
    #[default]
    Brush,
    Highlighter,
    Airbrush,
    Calligraphy,
    Eraser,
    Fill,
    Picker,
    Text,
    Mosaic,
    Crop,
    Hand,
    Shapes,
    RedactBox,
    RedactBrush,
}

impl Tool {
    pub const ALL: [Tool; 16] = [
        Tool::Select,
        Tool::Pencil,
        Tool::Brush,
        Tool::Highlighter,
        Tool::Airbrush,
        Tool::Calligraphy,
        Tool::Eraser,
        Tool::Fill,
        Tool::Picker,
        Tool::Text,
        Tool::Mosaic,
        Tool::Crop,
        Tool::Hand,
        Tool::Shapes,
        Tool::RedactBox,
        Tool::RedactBrush,
    ];
    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0) as i32
    }
    pub fn from_index(i: i32) -> Self {
        Self::ALL
            .get(i.max(0) as usize)
            .copied()
            .unwrap_or_default()
    }
    pub fn is_raster(self) -> bool {
        matches!(
            self,
            Tool::Pencil
                | Tool::Brush
                | Tool::Highlighter
                | Tool::Airbrush
                | Tool::Calligraphy
                | Tool::Eraser
                | Tool::Mosaic
        )
    }
}

/// Integer pixel rectangle (x0, y0) inclusive .. (x1, y1) exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IRect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl IRect {
    pub fn around(ax: f32, ay: f32, bx: f32, by: f32, pad: f32, w: u32, h: u32) -> Option<Self> {
        let x0 = (ax.min(bx) - pad).floor().max(0.0) as u32;
        let y0 = (ay.min(by) - pad).floor().max(0.0) as u32;
        let x1 = ((ax.max(bx) + pad).ceil().max(0.0) as u32).min(w);
        let y1 = ((ay.max(by) + pad).ceil().max(0.0) as u32).min(h);
        (x1 > x0 && y1 > y0).then_some(Self { x0, y0, x1, y1 })
    }
    pub fn union(self, o: Self) -> Self {
        Self {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }
    pub fn full(w: u32, h: u32) -> Self {
        Self {
            x0: 0,
            y0: 0,
            x1: w,
            y1: h,
        }
    }
}

/// Selection rectangle in image coordinates (float, may be circle).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Selection {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub circle: bool,
}

impl Selection {
    pub fn from_points(a: (f32, f32), b: (f32, f32), circle: bool, iw: u32, ih: u32) -> Self {
        let cl = |v: f32, m: u32| v.clamp(0.0, m as f32);
        let (x0, x1) = (cl(a.0.min(b.0), iw), cl(a.0.max(b.0), iw));
        let (y0, y1) = (cl(a.1.min(b.1), ih), cl(a.1.max(b.1), ih));
        let (mut w, mut h) = (x1 - x0, y1 - y0);
        if circle {
            let d = w.min(h);
            w = d;
            h = d;
        }
        Self {
            x: x0,
            y: y0,
            w,
            h,
            circle,
        }
    }
    pub fn mask(&self, iw: u32, ih: u32) -> Option<SelectionMask> {
        let x = (self.x.floor().max(0.0) as u32).min(iw.saturating_sub(1));
        let y = (self.y.floor().max(0.0) as u32).min(ih.saturating_sub(1));
        let w = (self.w.ceil().max(1.0) as u32).min(iw - x).max(1);
        let h = (self.h.ceil().max(1.0) as u32).min(ih - y).max(1);
        if self.w < 1.0 || self.h < 1.0 {
            return None;
        }
        Some(if self.circle {
            SelectionMask::Circle {
                x,
                y,
                diameter: w.min(h),
            }
        } else {
            SelectionMask::Rectangle {
                x,
                y,
                width: w,
                height: h,
            }
        })
    }
}

#[derive(Clone)]
pub struct Snapshot {
    image: RgbaImage,
    layers: Vec<ComposeLayer>,
    selected: Option<usize>,
    revision: u64,
    selection: Option<Selection>,
    boxes: Vec<RedactBox>,
}

/// Brush/tool settings shared by the UI.
#[derive(Debug, Clone)]
pub struct ToolSettings {
    pub stroke_width: f32,
    pub color1: [u8; 4],
    pub color2: [u8; 4],
    pub shape: ShapeKind,
    pub shape_filled: bool,
    pub fill_tolerance: u8,
    pub redact_color: [u8; 4],
    pub redact_brush_radius: f32,
}

impl Default for ToolSettings {
    fn default() -> Self {
        Self {
            stroke_width: 4.0,
            color1: [0, 0, 0, 255],
            color2: [255, 255, 255, 255],
            shape: ShapeKind::Rectangle,
            shape_filled: false,
            fill_tolerance: 16,
            redact_color: [0, 0, 0, 255],
            redact_brush_radius: 12.0,
        }
    }
}

pub struct Doc {
    pub image: RgbaImage,
    pub original: RgbaImage,
    pub layers: Vec<ComposeLayer>,
    pub selected: Option<usize>,
    pub selection: Option<Selection>,
    pub path: Option<PathBuf>,
    pub font_bytes: Option<Vec<u8>>,
    pub boxes: Vec<RedactBox>,
    pub library: Vec<Sticker>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    revision: u64,
    next_revision: u64,
    saved_revision: u64,
    /// Layers rendered over a transparent canvas (cached until layers change).
    overlay: Option<RgbaImage>,
    overlay_dirty: bool,
    /// Region of the composite that must be refreshed on screen.
    pub dirty: Option<IRect>,
    /// The canvas size changed (UI must reallocate its pixel buffer).
    pub resized: bool,
}

impl Doc {
    pub fn new(w: u32, h: u32) -> Self {
        let image = RgbaImage::from_pixel(w.max(1), h.max(1), Rgba([255, 255, 255, 255]));
        Self {
            original: image.clone(),
            image,
            layers: Vec::new(),
            selected: None,
            selection: None,
            path: None,
            font_bytes: None,
            boxes: Vec::new(),
            library: stickers::library(),
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            next_revision: 0,
            saved_revision: 0,
            overlay: None,
            overlay_dirty: true,
            dirty: None,
            resized: true,
        }
    }

    pub fn width(&self) -> u32 {
        self.image.width()
    }
    pub fn height(&self) -> u32 {
        self.image.height()
    }

    /// Replace the whole document with an image (open file / paste / new canvas).
    pub fn load(&mut self, image: DynamicImage, path: Option<PathBuf>) {
        self.image = image.to_rgba8();
        self.original = self.image.clone();
        self.layers.clear();
        self.selected = None;
        self.selection = None;
        self.boxes.clear();
        self.undo.clear();
        self.redo.clear();
        self.path = path;
        self.invalidate_all();
        self.mark_saved();
    }

    pub fn invalidate_all(&mut self) {
        self.overlay_dirty = true;
        self.resized = true;
        self.mark(IRect::full(self.width(), self.height()));
    }

    pub fn layers_changed(&mut self) {
        self.overlay_dirty = true;
        self.mark(IRect::full(self.width(), self.height()));
    }

    fn mark(&mut self, r: IRect) {
        self.next_revision += 1;
        self.revision = self.next_revision;
        self.dirty = Some(match self.dirty {
            Some(d) => d.union(r),
            None => r,
        });
    }

    // ------------------------------------------------------------ history

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            image: self.image.clone(),
            layers: self.layers.clone(),
            selected: self.selected,
            revision: self.revision,
            selection: self.selection,
            boxes: self.boxes.clone(),
        }
    }

    /// Record the current state before a modification.
    pub fn checkpoint(&mut self) {
        self.undo.push(self.snapshot());
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_modified(&self) -> bool {
        self.revision != self.saved_revision
    }

    pub fn mark_saved(&mut self) {
        self.saved_revision = self.revision;
    }

    fn restore(&mut self, s: Snapshot) {
        let resized = s.image.dimensions() != self.image.dimensions();
        self.image = s.image;
        self.layers = s.layers;
        self.selected = s.selected.filter(|i| *i < self.layers.len());
        self.selection = s.selection;
        self.boxes = s.boxes;
        self.overlay_dirty = true;
        self.resized |= resized;
        self.mark(IRect::full(self.width(), self.height()));
        self.revision = s.revision;
    }

    pub fn undo(&mut self) -> bool {
        let Some(s) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.restore(s);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(s) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snapshot());
        self.restore(s);
        true
    }

    pub fn begin_gesture(&self) -> Snapshot {
        self.snapshot()
    }

    pub fn cancel_gesture(&mut self, before: Snapshot) {
        self.restore(before);
    }

    pub fn commit_gesture(&mut self, before: Snapshot) {
        if before.revision == self.revision && before.boxes == self.boxes {
            return;
        }
        self.undo.push(before);
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    // ------------------------------------------------------------ compositing

    fn ensure_overlay(&mut self) {
        if !self.overlay_dirty
            && self
                .overlay
                .as_ref()
                .is_some_and(|o| o.dimensions() == self.image.dimensions())
        {
            return;
        }
        self.refresh_stickers();
        self.overlay = if self.layers.iter().any(|l| l.is_visible()) {
            let transparent = DynamicImage::ImageRgba8(RgbaImage::new(self.width(), self.height()));
            let doc = ComposeDocument {
                background: transparent,
                layers: self.layers.clone(),
            };
            Some(render_composition(&doc, self.font_bytes.as_deref()).to_rgba8())
        } else {
            None
        };
        self.overlay_dirty = false;
    }

    /// Re-rasterize sticker layers whose size/style changed.
    pub fn refresh_stickers(&mut self) {
        for layer in &mut self.layers {
            if let ComposeLayer::Image(img) = layer
                && let Some(inst) = &mut img.sticker
            {
                let key = (img.width.max(1), img.height.max(1), inst.style);
                if inst.rendered != Some(key)
                    && let Some(s) = stickers::find(&self.library, &inst.id)
                {
                    img.image = stickers::render_sticker(s, &inst.style, key.0, key.1);
                    inst.rendered = Some(key);
                }
            }
        }
    }

    /// Write the composite (image + layers) for `r` into an RGBA8 buffer of the
    /// canvas size. Only the dirty rectangle is recomputed.
    pub fn composite_into(&mut self, buf: &mut [u8], r: IRect) {
        self.ensure_overlay();
        let w = self.width() as usize;
        for y in r.y0..r.y1 {
            let row = y as usize * w;
            for x in r.x0..r.x1 {
                let i = (row + x as usize) * 4;
                let mut p = self.image.get_pixel(x, y).0;
                if let Some(o) = &self.overlay {
                    p = over(p, o.get_pixel(x, y).0);
                }
                buf[i..i + 4].copy_from_slice(&p);
            }
        }
    }

    /// Full-quality flattened output (used for export and redaction).
    pub fn flattened(&self) -> DynamicImage {
        let bg = DynamicImage::ImageRgba8(self.image.clone());
        if self.layers.is_empty() {
            return bg;
        }
        let mut layers = self.layers.clone();
        for l in &mut layers {
            if let ComposeLayer::Image(img) = l
                && let Some(inst) = &img.sticker
                && let Some(s) = stickers::find(&self.library, &inst.id)
                && inst.rendered != Some((img.width, img.height, inst.style))
            {
                img.image =
                    stickers::render_sticker(s, &inst.style, img.width.max(1), img.height.max(1));
            }
        }
        render_composition(
            &ComposeDocument {
                background: bg,
                layers,
            },
            self.font_bytes.as_deref(),
        )
    }

    pub fn sample(&self, x: f32, y: f32) -> Option<[u8; 4]> {
        let (x, y) = (x.floor(), y.floor());
        (x >= 0.0 && y >= 0.0 && (x as u32) < self.width() && (y as u32) < self.height())
            .then(|| self.image.get_pixel(x as u32, y as u32).0)
    }

    // ------------------------------------------------------------ painting

    /// Apply one raster stroke segment. Returns false for non-raster tools.
    pub fn stroke(
        &mut self,
        tool: Tool,
        from: (f32, f32),
        to: (f32, f32),
        secondary: bool,
        s: &ToolSettings,
    ) -> bool {
        use studio_core::drawing as d;
        let (x0, y0, x1, y1) = (
            from.0.round() as i32,
            from.1.round() as i32,
            to.0.round() as i32,
            to.1.round() as i32,
        );
        let color = Rgba(if secondary || tool == Tool::Eraser {
            s.color2
        } else {
            s.color1
        });
        let r = (s.stroke_width / 2.0).max(0.5);
        let img = &mut self.image;
        let pad = match tool {
            Tool::Pencil => {
                d::draw_pencil_line(img, x0, y0, x1, y1, color);
                2.0
            }
            Tool::Brush => {
                d::draw_brush_stroke(img, x0, y0, x1, y1, r, color);
                r + 2.0
            }
            Tool::Highlighter => {
                d::draw_highlighter_stroke(img, x0, y0, x1, y1, r, color, 128);
                r + 2.0
            }
            Tool::Airbrush => {
                let ar = (s.stroke_width * 1.5).clamp(4.0, 64.0);
                d::draw_airbrush(img, x1, y1, ar, 25, color);
                ar + 2.0
            }
            Tool::Calligraphy => {
                let cw = (s.stroke_width * 1.5).clamp(3.0, 64.0);
                d::draw_calligraphy_stroke(img, x0, y0, x1, y1, cw, 45.0, color);
                cw + 2.0
            }
            Tool::Eraser => {
                d::draw_eraser(img, x0, y0, x1, y1, r, color);
                r + 2.0
            }
            Tool::Mosaic => {
                let block = (s.stroke_width as u32).clamp(4, 32);
                let bs = block * 3;
                let rx = (x1.max(0) as u32).saturating_sub(bs / 2);
                let ry = (y1.max(0) as u32).saturating_sub(bs / 2);
                d::apply_mosaic_region(img, rx, ry, bs, bs, block);
                // mosaic snaps to the box it touched
                if let Some(rect) = IRect::around(
                    rx as f32,
                    ry as f32,
                    (rx + bs) as f32,
                    (ry + bs) as f32,
                    1.0,
                    self.image.width(),
                    self.image.height(),
                ) {
                    self.mark(rect);
                }
                return true;
            }
            Tool::RedactBrush => {
                let rr = s.redact_brush_radius;
                let steps = (((to.0 - from.0).hypot(to.1 - from.1)) / (rr * 0.4).max(1.0))
                    .ceil()
                    .max(1.0) as usize;
                for i in 0..=steps {
                    let t = i as f32 / steps as f32;
                    redact_brush_dab(
                        img,
                        from.0 + (to.0 - from.0) * t,
                        from.1 + (to.1 - from.1) * t,
                        rr,
                        s.redact_color,
                    );
                }
                rr + 2.0
            }
            _ => return false,
        };
        if let Some(rect) =
            IRect::around(from.0, from.1, to.0, to.1, pad, self.width(), self.height())
        {
            self.mark(rect);
        }
        true
    }

    pub fn flood_fill(&mut self, x: f32, y: f32, color: [u8; 4], tolerance: u8) -> bool {
        if self.sample(x, y).is_none() {
            return false;
        }
        self.checkpoint();
        studio_core::drawing::flood_fill(
            &mut self.image,
            x as u32,
            y as u32,
            Rgba(color),
            tolerance,
        );
        self.mark(IRect::full(self.width(), self.height()));
        true
    }

    // ------------------------------------------------------------ whole-image ops

    /// Apply an image→image filter, limited to the selection if there is one.
    pub fn apply_filter(&mut self, f: impl FnOnce(&DynamicImage) -> DynamicImage) {
        self.checkpoint();
        let src = DynamicImage::ImageRgba8(self.image.clone());
        let edited = f(&src);
        let out = match self
            .selection
            .and_then(|s| s.mask(self.width(), self.height()))
        {
            Some(m) if edited.width() == src.width() && edited.height() == src.height() => {
                studio_core::transform::apply_selection_mask(&src, &edited, m)
            }
            _ => edited,
        };
        self.set_image(out.to_rgba8());
    }

    /// Geometry ops apply to the visible composition. The UI confirms flattening
    /// first; undo restores the original editable layers in one step.
    pub fn apply_transform(&mut self, f: impl FnOnce(&DynamicImage) -> DynamicImage) {
        self.checkpoint();
        let out = f(&self.flattened()).to_rgba8();
        self.layers.clear();
        self.selected = None;
        self.boxes.clear();
        self.overlay_dirty = true;
        self.selection = None;
        self.set_image(out);
    }

    fn set_image(&mut self, img: RgbaImage) {
        if img.dimensions() != self.image.dimensions() {
            self.resized = true;
            self.overlay_dirty = true;
        }
        self.image = img;
        self.mark(IRect::full(self.width(), self.height()));
    }

    pub fn crop_to_selection(&mut self) -> bool {
        let Some(sel) = self.selection else {
            return false;
        };
        let Some(
            SelectionMask::Rectangle {
                x,
                y,
                width,
                height,
            }
            | SelectionMask::Circle {
                x,
                y,
                diameter: width @ height,
            },
        ) = Selection {
            circle: false,
            ..sel
        }
        .mask(self.width(), self.height())
        else {
            return false;
        };
        self.checkpoint();
        let cropped = image::imageops::crop_imm(&self.image, x, y, width, height).to_image();
        for l in &mut self.layers {
            move_layer(l, -(x as i32), -(y as i32));
        }
        self.selection = None;
        self.overlay_dirty = true;
        self.set_image(cropped);
        true
    }

    /// Resize the canvas (not the pixels), filling new area with colour 2.
    pub fn resize_canvas(&mut self, w: u32, h: u32, bg: [u8; 4]) {
        if w == 0 || h == 0 || (w, h) == self.image.dimensions() {
            return;
        }
        self.checkpoint();
        let out = studio_core::canvas_resize(&self.image, w, h, Rgba(bg));
        self.set_image(out);
    }

    pub fn reset_to_original(&mut self) {
        self.checkpoint();
        self.layers.clear();
        self.selected = None;
        self.overlay_dirty = true;
        let o = self.original.clone();
        self.set_image(o);
    }

    pub fn flatten(&mut self) {
        if self.layers.is_empty() {
            return;
        }
        self.checkpoint();
        let flat = self.flattened().to_rgba8();
        self.layers.clear();
        self.selected = None;
        self.overlay_dirty = true;
        self.set_image(flat);
    }

    // ------------------------------------------------------------ layers

    pub fn layer_bounds(&self, i: usize) -> Option<(f32, f32, f32, f32)> {
        let l = self.layers.get(i)?;
        let b = compose_layer_bounds(l, self.font_bytes.as_deref());
        Some((b.x as f32, b.y as f32, b.width as f32, b.height as f32))
    }

    pub fn hit_test(&self, x: f32, y: f32) -> Option<usize> {
        (0..self.layers.len()).rev().find(|&i| {
            self.layers[i].is_visible()
                && self.layer_bounds(i).is_some_and(|(bx, by, bw, bh)| {
                    x >= bx && y >= by && x < bx + bw && y < by + bh
                })
        })
    }

    /// Is (x, y) on the bottom-right resize handle of the selected image/sticker layer?
    pub fn on_resize_handle(&self, x: f32, y: f32, tolerance: f32) -> bool {
        let Some(i) = self.selected else { return false };
        let Some(ComposeLayer::Image(l)) = self.layers.get(i) else {
            return false;
        };
        let (cx, cy) = (
            (l.x + l.width as i32) as f32,
            (l.y + l.height as i32) as f32,
        );
        (x - cx).abs() <= tolerance && (y - cy).abs() <= tolerance
    }

    pub fn move_selected(&mut self, dx: i32, dy: i32) {
        if let Some(i) = self.selected
            && let Some(l) = self.layers.get_mut(i)
        {
            move_layer(l, dx, dy);
            self.layers_changed();
        }
    }

    /// A keyboard move is its own undoable edit (pointer drags checkpoint once).
    pub fn nudge_selected(&mut self, dx: i32, dy: i32) -> bool {
        if (dx == 0 && dy == 0) || self.selected.is_none_or(|i| i >= self.layers.len()) {
            return false;
        }
        self.checkpoint();
        self.move_selected(dx, dy);
        true
    }

    /// Resize the selected image layer so its bottom-right corner is at (x, y), keeping aspect.
    pub fn resize_selected_to(&mut self, x: f32, aspect: f32) {
        if let Some(i) = self.selected
            && let Some(ComposeLayer::Image(l)) = self.layers.get_mut(i)
        {
            let w = (x - l.x as f32).max(8.0);
            l.width = w.round() as u32;
            l.height = (w * aspect).round().max(1.0) as u32;
            self.layers_changed();
        }
    }

    pub fn push_layer(&mut self, layer: ComposeLayer) {
        self.checkpoint();
        self.layers.push(layer);
        self.selected = Some(self.layers.len() - 1);
        self.layers_changed();
    }

    pub fn add_text(&mut self, x: f32, y: f32, name: String) {
        let layer = TextLayer {
            name,
            x: x.round().max(0.0) as i32,
            y: y.round().max(0.0) as i32,
            font_size: (self.height() as f32 / 12.0).clamp(24.0, 96.0),
            ..Default::default()
        };
        self.push_layer(ComposeLayer::Text(layer));
    }

    /// Quick text styles: 0 outlined, 1 shadow, 2 outline+shadow, 3 plain.
    pub fn add_styled_text(&mut self, preset: usize, text: &str, color1: [u8; 4]) {
        let size = (self.height() as f32 * 0.09).clamp(28.0, 200.0);
        let (color, stroke, shadow, name) = match preset {
            0 => ([255, 255, 255, 255], true, false, "Outlined title"),
            1 => (color1, false, true, "Shadow title"),
            2 => ([255, 214, 0, 255], true, true, "Outline + shadow"),
            _ => (color1, false, false, "Plain text"),
        };
        self.push_layer(ComposeLayer::Text(TextLayer {
            name: name.to_owned(),
            text: text.to_owned(),
            x: (self.width() as f32 * 0.08) as i32,
            y: (self.height() as f32 * 0.08) as i32,
            font_size: size,
            color,
            stroke,
            shadow,
            ..Default::default()
        }));
    }

    pub fn add_shape(&mut self, kind: ShapeKind, a: (f32, f32), b: (f32, f32), s: &ToolSettings) {
        let mut layer = ShapeLayer::new(
            kind,
            a.0.round() as i32,
            a.1.round() as i32,
            b.0.round() as i32,
            b.1.round() as i32,
        );
        layer.stroke_color = s.color1;
        layer.fill_color = s.color2;
        layer.stroke_width = s.stroke_width;
        layer.filled = s.shape_filled;
        self.push_layer(ComposeLayer::Shape(layer));
    }

    pub fn add_image_layer(&mut self, name: String, img: DynamicImage) {
        let mut l = ImageLayer::new(name, img);
        // fit inside the canvas
        let (cw, ch) = (self.width() as f32, self.height() as f32);
        let k = (cw * 0.8 / l.width as f32)
            .min(ch * 0.8 / l.height as f32)
            .min(1.0);
        l.width = ((l.width as f32 * k).round() as u32).max(1);
        l.height = ((l.height as f32 * k).round() as u32).max(1);
        self.push_layer(ComposeLayer::Image(l));
    }

    pub fn insert_sticker(&mut self, id: &str, at: Option<(f32, f32)>, base: StickerStyle) -> bool {
        let Some(s) = stickers::find(&self.library, id).cloned() else {
            return false;
        };
        let side = ((self.width().min(self.height()) as f32) * 0.22).clamp(48.0, 480.0) as u32;
        let mut style = base;
        style.fill = s.default_fill;
        if s.default_fill[..3] == [255, 255, 255] {
            style.stroke = [40, 40, 40, 255];
        }
        let (cx, cy) = at.unwrap_or((self.width() as f32 / 2.0, self.height() as f32 / 2.0));
        let mut layer = ImageLayer::new(
            s.id.clone(),
            stickers::render_sticker(&s, &style, side, side),
        );
        layer.x = (cx - side as f32 / 2.0) as i32;
        layer.y = (cy - side as f32 / 2.0) as i32;
        layer.sticker = Some(StickerInstance {
            id: s.id.clone(),
            style,
            rendered: Some((side, side, style)),
        });
        self.push_layer(ComposeLayer::Image(layer));
        true
    }

    pub fn delete_selected(&mut self) -> bool {
        let Some(i) = self.selected.filter(|i| *i < self.layers.len()) else {
            return false;
        };
        self.checkpoint();
        self.layers.remove(i);
        self.selected = None;
        self.layers_changed();
        true
    }

    pub fn duplicate_selected(&mut self) {
        if let Some(i) = self.selected
            && let Some(l) = self.layers.get(i).cloned()
        {
            let mut l = l;
            move_layer(&mut l, 16, 16);
            *l.name_mut() = format!("{} copy", l.name());
            self.push_layer(l);
        }
    }

    /// Move the selected layer up (+1, towards the top) or down (-1).
    pub fn reorder_selected(&mut self, dir: i32) {
        let Some(i) = self.selected else { return };
        let j = i as i32 + dir;
        if j < 0 || j as usize >= self.layers.len() {
            return;
        }
        self.checkpoint();
        self.layers.swap(i, j as usize);
        self.selected = Some(j as usize);
        self.layers_changed();
    }

    /// Mutate the selected layer (no history: caller checkpoints at edit start).
    pub fn with_selected(&mut self, f: impl FnOnce(&mut ComposeLayer)) {
        if let Some(i) = self.selected
            && let Some(l) = self.layers.get_mut(i)
        {
            f(l);
            self.layers_changed();
        }
    }

    // ------------------------------------------------------------ redaction

    /// Flatten layers and apply selected redaction boxes destructively.
    pub fn apply_redaction(&mut self, style: RedactStyle, padding: u32) -> usize {
        if !self.boxes.iter().any(|b| b.selected) {
            return 0;
        }
        self.checkpoint();
        let flat = self.flattened();
        let (out, n) = apply_redactions(&flat, &self.boxes, style, padding);
        self.layers.clear();
        self.selected = None;
        self.overlay_dirty = true;
        self.boxes.retain(|b| !b.selected);
        self.set_image(out.to_rgba8());
        n
    }

    pub fn box_at(&self, x: f32, y: f32) -> Option<usize> {
        self.boxes.iter().rposition(|b| b.contains(x, y))
    }

    // ------------------------------------------------------------ projects

    pub fn to_project_json(&self) -> Result<String, String> {
        let mut layers = Vec::new();
        for l in &self.layers {
            layers.push(match l {
                ComposeLayer::Text(t) => ProjectLayer::Text {
                    name: t.name.clone(),
                    text: t.text.clone(),
                    font_path: t.font_path.as_ref().map(|p| p.display().to_string()),
                    x: t.x,
                    y: t.y,
                    font_size: t.font_size,
                    color: t.color,
                    opacity: t.opacity,
                    stroke: t.stroke,
                    shadow: t.shadow,
                    visible: t.visible,
                },
                ComposeLayer::Image(i) => ProjectLayer::Image {
                    name: i.name.clone(),
                    image_png: png_b64(&i.image)?,
                    x: i.x,
                    y: i.y,
                    width: i.width,
                    height: i.height,
                    opacity: i.opacity,
                    visible: i.visible,
                    sticker_id: i.sticker.as_ref().map(|s| s.id.clone()),
                    sticker_fill: i.sticker.as_ref().map(|s| s.style.fill),
                    sticker_stroke: i.sticker.as_ref().map(|s| s.style.stroke),
                    sticker_stroke_width: i.sticker.as_ref().map(|s| s.style.stroke_width),
                    sticker_shadow: i.sticker.as_ref().map(|s| s.style.shadow),
                },
                ComposeLayer::Shape(s) => ProjectLayer::Shape {
                    name: s.name.clone(),
                    shape_kind: s.kind,
                    x1: s.x1,
                    y1: s.y1,
                    x2: s.x2,
                    y2: s.y2,
                    stroke_color: s.stroke_color,
                    stroke_width: s.stroke_width,
                    opacity: s.opacity,
                    filled: s.filled,
                    fill_color: s.fill_color,
                    visible: s.visible,
                },
            });
        }
        let p = ProjectFile {
            version: 2,
            background_png: png_b64(&DynamicImage::ImageRgba8(self.image.clone()))?,
            layers,
        };
        serde_json::to_string_pretty(&p).map_err(|e| e.to_string())
    }

    pub fn load_project_json(&mut self, json: &str) -> Result<(), String> {
        let p: ProjectFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let bg = png_decode(&p.background_png)?;
        let mut layers = Vec::new();
        for l in p.layers {
            layers.push(match l {
                ProjectLayer::Text {
                    name,
                    text,
                    font_path,
                    x,
                    y,
                    font_size,
                    color,
                    opacity,
                    stroke,
                    shadow,
                    visible,
                } => ComposeLayer::Text(TextLayer {
                    name,
                    text,
                    font_path: font_path.map(PathBuf::from),
                    x,
                    y,
                    font_size,
                    color,
                    opacity,
                    stroke,
                    shadow,
                    visible,
                }),
                ProjectLayer::Image {
                    name,
                    image_png,
                    x,
                    y,
                    width,
                    height,
                    opacity,
                    visible,
                    sticker_id,
                    sticker_fill,
                    sticker_stroke,
                    sticker_stroke_width,
                    sticker_shadow,
                } => ComposeLayer::Image(ImageLayer {
                    name,
                    image: png_decode(&image_png)?,
                    x,
                    y,
                    width,
                    height,
                    opacity,
                    visible,
                    sticker: sticker_id.map(|id| {
                        let d = StickerStyle::default();
                        StickerInstance {
                            id,
                            style: StickerStyle {
                                fill: sticker_fill.unwrap_or(d.fill),
                                stroke: sticker_stroke.unwrap_or(d.stroke),
                                stroke_width: sticker_stroke_width.unwrap_or(d.stroke_width),
                                shadow: sticker_shadow.unwrap_or(d.shadow),
                            },
                            rendered: None,
                        }
                    }),
                }),
                ProjectLayer::Shape {
                    name,
                    shape_kind,
                    x1,
                    y1,
                    x2,
                    y2,
                    stroke_color,
                    stroke_width,
                    opacity,
                    filled,
                    fill_color,
                    visible,
                } => ComposeLayer::Shape(ShapeLayer {
                    name,
                    kind: shape_kind,
                    x1,
                    y1,
                    x2,
                    y2,
                    stroke_color,
                    stroke_width,
                    opacity,
                    filled,
                    fill_color,
                    visible,
                }),
            });
        }
        self.load(bg, None);
        self.layers = layers;
        self.layers_changed();
        self.mark_saved();
        Ok(())
    }
}

pub fn move_layer(l: &mut ComposeLayer, dx: i32, dy: i32) {
    match l {
        ComposeLayer::Text(t) => {
            t.x += dx;
            t.y += dy;
        }
        ComposeLayer::Image(i) => {
            i.x += dx;
            i.y += dy;
        }
        ComposeLayer::Shape(s) => {
            s.x1 += dx;
            s.y1 += dy;
            s.x2 += dx;
            s.y2 += dy;
        }
    }
}

/// Source-over blend of `src` onto `dst` (straight alpha).
fn over(dst: [u8; 4], src: [u8; 4]) -> [u8; 4] {
    let sa = src[3] as u32;
    if sa == 0 {
        return dst;
    }
    if sa == 255 {
        return src;
    }
    let da = dst[3] as u32;
    let oa = sa + da * (255 - sa) / 255;
    if oa == 0 {
        return [0, 0, 0, 0];
    }
    let mut out = [0u8; 4];
    for c in 0..3 {
        out[c] = ((src[c] as u32 * sa + dst[c] as u32 * da * (255 - sa) / 255) / oa) as u8;
    }
    out[3] = oa as u8;
    out
}

fn png_b64(img: &DynamicImage) -> Result<String, String> {
    let mut bytes = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .map_err(|e| e.to_string())?;
    Ok(general_purpose::STANDARD.encode(bytes))
}

fn png_decode(s: &str) -> Result<DynamicImage, String> {
    let bytes = general_purpose::STANDARD
        .decode(s)
        .map_err(|e| e.to_string())?;
    image::load_from_memory(&bytes).map_err(|e| e.to_string())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ProjectFile {
    version: u32,
    background_png: String,
    layers: Vec<ProjectLayer>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
enum ProjectLayer {
    Text {
        name: String,
        text: String,
        font_path: Option<String>,
        x: i32,
        y: i32,
        font_size: f32,
        color: [u8; 4],
        opacity: f32,
        stroke: bool,
        shadow: bool,
        visible: bool,
    },
    Image {
        name: String,
        image_png: String,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        opacity: f32,
        visible: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sticker_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sticker_fill: Option<[u8; 4]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sticker_stroke: Option<[u8; 4]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sticker_stroke_width: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sticker_shadow: Option<bool>,
    },
    Shape {
        name: String,
        shape_kind: ShapeKind,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        stroke_color: [u8; 4],
        stroke_width: f32,
        opacity: f32,
        filled: bool,
        fill_color: [u8; 4],
        visible: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_revision_tracks_undo_redo_and_new_edit_branch() {
        let mut d = Doc::new(50, 50);
        assert!(!d.is_modified());
        d.add_text(5.0, 5.0, "title".into());
        d.mark_saved();
        let saved = d.revision();
        assert!(d.nudge_selected(4, 0));
        assert!(d.is_modified());
        assert!(d.undo());
        assert!(!d.is_modified());
        assert_eq!(d.layers.len(), 1);
        assert!(d.redo());
        assert!(d.is_modified());
        assert!(d.undo());
        d.nudge_selected(0, 3);
        assert!(!d.can_redo());
        assert_ne!(d.revision(), saved);
        d.load(DynamicImage::new_rgba8(10, 10), None);
        assert!(!d.is_modified());
        assert_ne!(d.revision(), saved);
    }

    #[test]
    fn cancelled_gesture_restores_pixels_layers_and_redo() {
        let mut d = Doc::new(50, 50);
        d.add_text(5.0, 5.0, "title".into());
        d.nudge_selected(4, 0);
        d.undo();
        let json = d.to_project_json().unwrap();
        let revision = d.revision();
        let before = d.begin_gesture();
        d.move_selected(12, 8);
        d.stroke(
            Tool::Pencil,
            (0.0, 0.0),
            (10.0, 0.0),
            false,
            &ToolSettings::default(),
        );
        d.cancel_gesture(before);
        assert_eq!(d.to_project_json().unwrap(), json);
        assert_eq!(d.revision(), revision);
        assert!(d.can_redo());
        d.redo();
        assert!(matches!(&d.layers[0], ComposeLayer::Text(t) if t.x == 9));
    }

    #[test]
    fn committed_gesture_is_one_undo_step() {
        let mut d = Doc::new(50, 50);
        d.add_text(5.0, 5.0, "title".into());
        let before = d.begin_gesture();
        d.move_selected(2, 0);
        d.move_selected(3, 0);
        d.commit_gesture(before);
        d.undo();
        assert!(matches!(&d.layers[0], ComposeLayer::Text(t) if t.x == 5));
        d.redo();
        assert!(matches!(&d.layers[0], ComposeLayer::Text(t) if t.x == 10));
    }

    #[test]
    fn redaction_box_gesture_can_be_undone_and_cancelled() {
        let mut d = Doc::new(100, 100);
        d.boxes.push(RedactBox::manual(10, 20, 30, 40));
        let before = d.begin_gesture();
        d.boxes[0].x = 25;
        d.commit_gesture(before);
        assert!(d.undo());
        assert_eq!(d.boxes[0].x, 10);
        assert!(d.redo());
        assert_eq!(d.boxes[0].x, 25);
        let before = d.begin_gesture();
        d.boxes[0].width = 80;
        d.cancel_gesture(before);
        assert_eq!(d.boxes[0].width, 30);
    }

    #[test]
    fn geometry_transforms_composition_and_undo_restores_editable_layers() {
        let mut d = Doc::new(100, 60);
        d.add_shape(
            ShapeKind::Rectangle,
            (70.0, 10.0),
            (90.0, 30.0),
            &ToolSettings {
                shape_filled: true,
                color2: [255, 0, 0, 255],
                ..Default::default()
            },
        );
        let json = d.to_project_json().unwrap();
        let expected = studio_core::transform::rotate_right(&d.flattened()).to_rgba8();
        d.apply_transform(studio_core::transform::rotate_right);
        assert_eq!(d.image, expected);
        assert!(d.layers.is_empty());
        d.undo();
        assert_eq!(d.to_project_json().unwrap(), json);
    }

    #[test]
    fn invalid_project_keeps_existing_document_and_history() {
        let mut d = Doc::new(50, 50);
        d.add_text(5.0, 5.0, "title".into());
        let json = d.to_project_json().unwrap();
        assert!(d.load_project_json("not a project").is_err());
        assert_eq!(d.to_project_json().unwrap(), json);
        assert!(d.can_undo());
        assert!(d.is_modified());
    }

    fn buf(d: &mut Doc) -> Vec<u8> {
        let mut b = vec![0u8; (d.width() * d.height() * 4) as usize];
        d.composite_into(&mut b, IRect::full(d.width(), d.height()));
        b
    }

    #[test]
    fn stroke_marks_only_a_small_dirty_region() {
        let mut d = Doc::new(400, 300);
        d.dirty = None;
        let s = ToolSettings::default();
        assert!(d.stroke(Tool::Brush, (10.0, 10.0), (20.0, 12.0), false, &s));
        let r = d.dirty.unwrap();
        assert!(r.x0 <= 8 && r.x1 >= 22 && r.x1 < 40 && r.y1 < 30, "{r:?}");
        assert_eq!(d.image.get_pixel(15, 11).0, [0, 0, 0, 255]);
    }

    #[test]
    fn undo_redo_roundtrip_including_layers() {
        let mut d = Doc::new(50, 50);
        d.checkpoint();
        d.stroke(
            Tool::Pencil,
            (0.0, 0.0),
            (10.0, 0.0),
            false,
            &ToolSettings::default(),
        );
        d.add_text(5.0, 5.0, "t".into());
        assert_eq!(d.layers.len(), 1);
        assert!(d.undo());
        assert_eq!(d.layers.len(), 0);
        assert!(d.undo());
        assert_eq!(d.image.get_pixel(5, 0).0, [255, 255, 255, 255]);
        assert!(d.redo() && d.redo());
        assert_eq!(d.layers.len(), 1);
        assert!(!d.redo());
    }

    #[test]
    fn composite_blends_layers_and_matches_flatten_for_opaque_layers() {
        let mut d = Doc::new(40, 40);
        d.add_shape(
            ShapeKind::Rectangle,
            (5.0, 5.0),
            (30.0, 30.0),
            &ToolSettings {
                shape_filled: true,
                color2: [0, 0, 255, 255],
                ..Default::default()
            },
        );
        let b = buf(&mut d);
        let i = ((20 * 40 + 20) * 4) as usize;
        assert_eq!(&b[i..i + 4], &[0, 0, 255, 255]);
        let flat = d.flattened().to_rgba8();
        assert_eq!(flat.get_pixel(20, 20).0, [0, 0, 255, 255]);
    }

    #[test]
    fn sticker_insert_move_resize_and_hit_test() {
        let mut d = Doc::new(500, 500);
        assert!(d.insert_sticker("heart", Some((250.0, 250.0)), StickerStyle::default()));
        let i = d.selected.unwrap();
        let (x, y, w, h) = d.layer_bounds(i).unwrap();
        assert_eq!(d.hit_test(x + w / 2.0, y + h / 2.0), Some(i));
        assert!(d.on_resize_handle(x + w, y + h, 6.0));
        d.resize_selected_to(x + w * 2.0, 1.0);
        let (_, _, w2, _) = d.layer_bounds(i).unwrap();
        assert!((w2 - w * 2.0).abs() < 2.0);
        d.move_selected(10, 0);
        assert_eq!(d.layer_bounds(i).unwrap().0, x + 10.0);
        let _ = buf(&mut d); // re-rasterizes at the new size
        if let ComposeLayer::Image(l) = &d.layers[i] {
            assert_eq!(l.image.width(), l.width);
        }
    }

    #[test]
    fn filter_respects_selection_and_crop_shifts_layers() {
        let mut d = Doc::new(20, 20);
        d.selection = Some(Selection {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 20.0,
            circle: false,
        });
        d.apply_filter(studio_core::transform::invert);
        assert_eq!(d.image.get_pixel(2, 2).0, [0, 0, 0, 255]);
        assert_eq!(d.image.get_pixel(15, 2).0, [255, 255, 255, 255]);
        d.add_text(12.0, 12.0, "t".into());
        d.selection = Some(Selection {
            x: 10.0,
            y: 10.0,
            w: 10.0,
            h: 10.0,
            circle: false,
        });
        assert!(d.crop_to_selection());
        assert_eq!(d.image.dimensions(), (10, 10));
        if let ComposeLayer::Text(t) = &d.layers[0] {
            assert_eq!((t.x, t.y), (2, 2));
        }
    }

    #[test]
    fn redaction_flattens_and_is_undoable() {
        let mut d = Doc::new(30, 30);
        d.add_text(0.0, 0.0, "x".into());
        d.boxes.push(RedactBox::manual(5, 5, 10, 10));
        assert_eq!(d.apply_redaction(RedactStyle::default(), 0), 1);
        assert!(d.layers.is_empty());
        assert_eq!(d.image.get_pixel(8, 8).0, [0, 0, 0, 255]);
        assert!(d.undo());
        assert_eq!(d.layers.len(), 1);
    }

    #[test]
    fn project_roundtrip_keeps_sticker_identity() {
        let mut d = Doc::new(100, 100);
        d.insert_sticker("star", None, StickerStyle::default());
        d.add_text(1.0, 1.0, "hello".into());
        let json = d.to_project_json().unwrap();
        let mut e = Doc::new(1, 1);
        e.load_project_json(&json).unwrap();
        assert_eq!(e.layers.len(), 2);
        assert!(
            matches!(&e.layers[0], ComposeLayer::Image(l) if l.sticker.as_ref().is_some_and(|s| s.id == "star"))
        );
        assert_eq!(e.image.dimensions(), (100, 100));
    }

    #[test]
    fn over_blend() {
        assert_eq!(
            over([255, 255, 255, 255], [0, 0, 0, 0]),
            [255, 255, 255, 255]
        );
        assert_eq!(over([255, 255, 255, 255], [0, 0, 0, 255]), [0, 0, 0, 255]);
        let m = over([255, 255, 255, 255], [0, 0, 0, 128]);
        assert!(m[0] > 120 && m[0] < 135 && m[3] == 255);
    }
}
