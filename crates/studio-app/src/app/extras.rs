//! v0.2 additions: sticker library + social canvas presets + quick text styles,
//! offline auto-redaction, pixel grid, export metadata policy, and redraw helpers.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};

use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Vec2};
use image::DynamicImage;
use studio_core::redact::{
    RedactBox, RedactStyle, apply_redactions, preselect_sensitive, redact_brush_dab,
    select_by_keyword,
};
use studio_core::stickers::{self, CANVAS_PRESETS, Sticker, StickerCategory, StickerStyle};
use studio_core::{ComposeLayer, ImageLayer, TextLayer};

use super::PixelStudioApp;
use crate::i18n::Language;
use crate::paint::InspectorTab;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RedactMode {
    #[default]
    Off,
    /// Drag to add a box, click a box to toggle it, right-click to delete it.
    Boxes,
    /// Paint solid colour directly into the pixels.
    Brush,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RedactStyleKind {
    #[default]
    Solid,
    Mosaic,
    Blur,
}

pub struct ExtrasState {
    pub library: Vec<Sticker>,
    pub thumbs: HashMap<String, TextureHandle>,
    pub sticker_query: String,
    pub sticker_category: Option<StickerCategory>,
    pub sticker_style: StickerStyle,
    pub show_grid: bool,
    pub keep_metadata: bool,
    // redaction
    pub boxes: Vec<RedactBox>,
    pub redact_mode: RedactMode,
    pub redact_style: RedactStyleKind,
    pub redact_color: [u8; 4],
    pub redact_padding: u32,
    pub mosaic_block: u32,
    pub blur_sigma: f32,
    pub redact_keyword: String,
    pub redact_brush_radius: f32,
    pub box_drag_start: Option<Pos2>,
    pub box_drag_now: Option<Pos2>,
    pub detect_rx: Option<Receiver<Result<Vec<RedactBox>, String>>>,
    pub resize_drag: Option<(usize, Vec2)>,
}

impl Default for ExtrasState {
    fn default() -> Self {
        Self {
            library: stickers::library(),
            thumbs: HashMap::new(),
            sticker_query: String::new(),
            sticker_category: None,
            sticker_style: StickerStyle::default(),
            show_grid: true,
            keep_metadata: false,
            boxes: Vec::new(),
            redact_mode: RedactMode::Off,
            redact_style: RedactStyleKind::Solid,
            redact_color: [0, 0, 0, 255],
            redact_padding: 4,
            mosaic_block: 12,
            blur_sigma: 10.0,
            redact_keyword: String::new(),
            redact_brush_radius: 12.0,
            box_drag_start: None,
            box_drag_now: None,
            detect_rx: None,
            resize_drag: None,
        }
    }
}

fn tl<'a>(lang: Language, zh: &'a str, en: &'a str) -> &'a str {
    if lang == Language::ZhTw { zh } else { en }
}

impl PixelStudioApp {
    /// Per-frame housekeeping: OCR results and sticker re-rasterization.
    pub(super) fn extras_poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.ex.detect_rx {
            if let Ok(result) = rx.try_recv() {
                self.ex.detect_rx = None;
                match result {
                    Ok(boxes) => {
                        let n = boxes.iter().filter(|b| b.selected).count();
                        self.status = if self.language == Language::ZhTw {
                            format!("偵測到 {} 行文字，已預選 {n} 個敏感項目", boxes.len())
                        } else {
                            format!(
                                "Detected {} text lines, pre-selected {n} sensitive items",
                                boxes.len()
                            )
                        };
                        self.ex.boxes = boxes;
                    }
                    Err(e) => self.status = format!("OCR: {e}"),
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        let mut changed = false;
        for layer in &mut self.compose_layers {
            if let ComposeLayer::Image(img) = layer
                && let Some(inst) = &mut img.sticker
            {
                let key = (img.width.max(1), img.height.max(1), inst.style);
                if inst.rendered != Some(key)
                    && let Some(s) = stickers::find(&self.ex.library, &inst.id)
                {
                    img.image = stickers::render_sticker(s, &inst.style, key.0, key.1);
                    inst.rendered = Some(key);
                    changed = true;
                }
            }
        }
        if changed {
            self.update_texture(ctx);
        }
    }

    fn thumb(&mut self, ctx: &egui::Context, s: &Sticker) -> TextureHandle {
        if let Some(t) = self.ex.thumbs.get(&s.id) {
            return t.clone();
        }
        let style = StickerStyle {
            fill: s.default_fill,
            stroke: [70, 70, 70, 255],
            stroke_width: 3.0,
            shadow: false,
        };
        let img = stickers::render_sticker(s, &style, 56, 56).to_rgba8();
        let ci = egui::ColorImage::from_rgba_unmultiplied([56, 56], img.as_raw());
        let t = ctx.load_texture(
            format!("sticker-{}", s.id),
            ci,
            egui::TextureOptions::LINEAR,
        );
        self.ex.thumbs.insert(s.id.clone(), t.clone());
        t
    }

    pub(super) fn insert_sticker(&mut self, ctx: &egui::Context, id: &str, at: Option<Pos2>) {
        let Some(s) = stickers::find(&self.ex.library, id).cloned() else {
            return;
        };
        let canvas = self.texture_size;
        let side = (canvas.x.min(canvas.y) * 0.22).clamp(48.0, 480.0) as u32;
        let mut style = self.ex.sticker_style;
        style.fill = s.default_fill;
        if s.default_fill[..3] == [255, 255, 255] {
            style.stroke = [40, 40, 40, 255];
        }
        let center = at.unwrap_or(Pos2::new(canvas.x / 2.0, canvas.y / 2.0));
        self.push_history();
        let mut layer = ImageLayer::new(
            s.id.clone(),
            stickers::render_sticker(&s, &style, side, side),
        );
        layer.x = (center.x - side as f32 / 2.0) as i32;
        layer.y = (center.y - side as f32 / 2.0) as i32;
        layer.sticker = Some(studio_core::compose::StickerInstance {
            id: s.id.clone(),
            style,
            rendered: Some((side, side, style)),
        });
        self.compose_layers.push(ComposeLayer::Image(layer));
        self.selected_layer = Some(self.compose_layers.len() - 1);
        self.paint_state.active_tool = crate::paint::PaintTool::Select;
        self.update_texture(ctx);
    }

    pub(super) fn new_canvas(&mut self, ctx: &egui::Context, w: u32, h: u32) {
        self.init_default_paint_canvas(ctx);
        let blank = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));
        let d = DynamicImage::ImageRgba8(blank);
        self.original_image = Some(d.clone());
        self.working_image = Some(d);
        self.ex.boxes.clear();
        self.fit_to_window = true;
        self.preview_pan = Vec2::ZERO;
        self.sync_edit_dimensions();
        self.update_texture(ctx);
        self.status = format!("{} {w}×{h}", tl(self.language, "新畫布", "New canvas"));
    }

    fn add_styled_text(&mut self, ctx: &egui::Context, preset: usize) {
        let c = self.paint_state.color_1_foreground.to_srgba_unmultiplied();
        let size = (self.texture_size.y * 0.09).clamp(28.0, 200.0);
        let (color, stroke, shadow, name) = match preset {
            0 => ([255, 255, 255, 255], true, false, "Outlined title"),
            1 => (c, false, true, "Shadow title"),
            2 => ([255, 214, 0, 255], true, true, "Outline + shadow"),
            _ => (c, false, false, "Plain text"),
        };
        self.push_history();
        self.compose_layers.push(ComposeLayer::Text(TextLayer {
            name: name.to_owned(),
            text: tl(self.language, "標題文字", "Title text").to_owned(),
            x: (self.texture_size.x * 0.08) as i32,
            y: (self.texture_size.y * 0.08) as i32,
            font_size: size,
            color,
            stroke,
            shadow,
            ..Default::default()
        }));
        self.selected_layer = Some(self.compose_layers.len() - 1);
        self.paint_state.active_tool = crate::paint::PaintTool::Select;
        self.update_texture(ctx);
    }

    fn canvas_presets_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.label(
            egui::RichText::new(tl(self.language, "社群貼文畫布", "Social post canvas")).strong(),
        );
        let mut chosen = None;
        egui::ComboBox::from_id_salt("canvas_presets")
            .selected_text(tl(
                self.language,
                "選擇尺寸建立新畫布…",
                "Pick a size for a new canvas…",
            ))
            .width(ui.available_width().min(300.0))
            .show_ui(ui, |ui| {
                for (name, w, h) in CANVAS_PRESETS {
                    if ui
                        .selectable_label(false, format!("{name}  {w}×{h}"))
                        .clicked()
                    {
                        chosen = Some((w, h));
                    }
                }
            });
        if let Some((w, h)) = chosen {
            self.new_canvas(ctx, w, h);
        }
    }

    fn text_styles_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.label(
            egui::RichText::new(tl(self.language, "快速文字樣式", "Quick text styles")).strong(),
        );
        ui.horizontal_wrapped(|ui| {
            let names = [
                tl(self.language, "外框標題", "Outlined title"),
                tl(self.language, "陰影標題", "Shadow title"),
                tl(self.language, "外框＋陰影", "Outline + shadow"),
                tl(self.language, "純文字", "Plain"),
            ];
            for (i, n) in names.iter().enumerate() {
                if ui.button(*n).clicked() {
                    self.add_styled_text(ctx, i);
                }
            }
        });
    }

    /// Extra controls appended to the Properties tab.
    pub(super) fn extras_properties(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(8.0);
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            use studio_core::ShapeKind as K;
            ui.label(tl(self.language, "形狀", "Shape"));
            for (k, zh, en) in [
                (K::Line, "／ 直線", "／ Line"),
                (K::Arrow, "→ 箭頭", "→ Arrow"),
                (K::Rectangle, "▭ 矩形", "▭ Rectangle"),
                (K::Ellipse, "◯ 橢圓", "◯ Ellipse"),
                (K::Triangle, "△ 三角形", "△ Triangle"),
            ] {
                if ui
                    .selectable_label(self.paint_state.shape_kind == k, tl(self.language, zh, en))
                    .clicked()
                {
                    self.paint_state.shape_kind = k;
                    self.paint_state.active_tool = crate::paint::PaintTool::Shapes;
                }
            }
            ui.checkbox(
                &mut self.paint_state.shape_filled,
                tl(self.language, "以色彩 2 填滿", "Fill with colour 2"),
            );
        });
        ui.checkbox(
            &mut self.ex.show_grid,
            tl(
                self.language,
                "放大時顯示像素格線 (Ctrl+G)",
                "Pixel grid when zoomed in (Ctrl+G)",
            ),
        );
        ui.checkbox(
            &mut self.ex.keep_metadata,
            tl(
                self.language,
                "匯出時保留 EXIF／色彩描述檔（預設移除）",
                "Keep EXIF / ICC on export (stripped by default)",
            ),
        );
        ui.horizontal(|ui| {
            ui.label(tl(self.language, "JPEG／WebP 品質", "JPEG / WebP quality"));
            ui.add(egui::Slider::new(&mut self.quality, 1..=100));
        });
        ui.small(tl(
            self.language,
            "WebP：品質 100 = 無損，其餘為有損",
            "WebP: quality 100 = lossless, below = lossy",
        ));
        ui.add_space(6.0);
        self.canvas_presets_ui(ui, ctx);
        ui.add_space(6.0);
        self.text_styles_ui(ui, ctx);
    }

    pub(super) fn stickers_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading(tl(self.language, "貼圖與圖示庫", "Stickers & icons"));
        ui.small(tl(self.language,
            "點一下插入，或拖曳到畫布上。插入後可移動、縮放、換色。",
            "Click to insert or drag onto the canvas. Inserted stickers are movable, resizable and recolorable.",
        ));
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.add(
                egui::TextEdit::singleline(&mut self.ex.sticker_query).hint_text(tl(
                    self.language,
                    "搜尋（例：愛心、arrow、SALE）",
                    "Search (e.g. heart, arrow, SALE)",
                )),
            );
        });
        let zh = self.language == Language::ZhTw;
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(
                    self.ex.sticker_category.is_none(),
                    tl(self.language, "全部", "All"),
                )
                .clicked()
            {
                self.ex.sticker_category = None;
            }
            for c in StickerCategory::ALL {
                if ui
                    .selectable_label(self.ex.sticker_category == Some(c), c.label(zh))
                    .clicked()
                {
                    self.ex.sticker_category = Some(c);
                }
            }
        });
        ui.separator();
        let items: Vec<Sticker> = stickers::search(
            &self.ex.library,
            &self.ex.sticker_query,
            self.ex.sticker_category,
        )
        .into_iter()
        .cloned()
        .collect();
        let mut insert = None;
        let cols = ((ui.available_width() / 50.0).floor() as usize).max(3);
        egui::Grid::new("sticker_grid")
            .spacing(Vec2::splat(4.0))
            .show(ui, |ui| {
                for (n, s) in items.iter().enumerate() {
                    let tex = self.thumb(ctx, s);
                    let resp = ui.add(
                        egui::Image::new((tex.id(), Vec2::splat(44.0)))
                            .sense(Sense::click_and_drag()),
                    );
                    if resp.dragged() {
                        resp.dnd_set_drag_payload(s.id.clone());
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    }
                    if resp.hovered() {
                        ui.painter().rect_stroke(
                            resp.rect,
                            4.0,
                            Stroke::new(1.5_f32, Color32::from_rgb(80, 180, 255)),
                            StrokeKind::Outside,
                        );
                    }
                    if resp.clicked() {
                        insert = Some(s.id.clone());
                    }
                    resp.on_hover_text(&s.keywords);
                    if (n + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
        if items.is_empty() {
            ui.label(tl(self.language, "找不到符合的貼圖", "No stickers match"));
        }
        if let Some(id) = insert {
            self.insert_sticker(ctx, &id, None);
        }
        ui.separator();
        self.sticker_style_ui(ui, ctx);
        ui.separator();
        self.canvas_presets_ui(ui, ctx);
        ui.add_space(6.0);
        self.text_styles_ui(ui, ctx);
        ui.add_space(6.0);
        ui.small(tl(self.language,
            "所有圖示皆為 Pixel Studio 原創（MIT）。不含任何平台商標；社群類為通用圖示。",
            "All icons are original Pixel Studio artwork (MIT). No platform trademarks; social icons are generic.",
        ));
    }

    fn sticker_style_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let selected = self
            .selected_layer
            .and_then(|i| match self.compose_layers.get(i) {
                Some(ComposeLayer::Image(l)) if l.sticker.is_some() => Some(i),
                _ => None,
            });
        let Some(i) = selected else {
            ui.small(tl(
                self.language,
                "選取一個貼圖圖層以調整顏色、外框和陰影。",
                "Select a sticker layer to change colour, outline and shadow.",
            ));
            return;
        };
        ui.label(egui::RichText::new(tl(self.language, "貼圖樣式", "Sticker style")).strong());
        let fill_l = tl(self.language, "填色", "Fill");
        let stroke_l = tl(self.language, "外框", "Outline");
        let width_l = tl(self.language, "外框寬度", "Outline width");
        let shadow_l = tl(self.language, "陰影", "Shadow");
        let size_l = tl(self.language, "大小", "Size");
        let opacity_l = tl(self.language, "不透明度", "Opacity");
        let mut changed = false;
        let mut snapshot = None;
        if let Some(ComposeLayer::Image(l)) = self.compose_layers.get_mut(i)
            && let Some(inst) = &mut l.sticker
        {
            let before = (inst.style, l.width, l.opacity);
            ui.horizontal(|ui| {
                ui.label(fill_l);
                ui.color_edit_button_srgba_unmultiplied(&mut inst.style.fill);
                ui.label(stroke_l);
                ui.color_edit_button_srgba_unmultiplied(&mut inst.style.stroke);
            });
            ui.add(egui::Slider::new(&mut inst.style.stroke_width, 0.0..=12.0).text(width_l));
            ui.checkbox(&mut inst.style.shadow, shadow_l);
            let mut side = l.width;
            if ui
                .add(egui::Slider::new(&mut side, 16..=2048).text(size_l))
                .changed()
            {
                let ratio = l.height as f32 / l.width.max(1) as f32;
                l.width = side;
                l.height = ((side as f32) * ratio).round().max(1.0) as u32;
            }
            ui.add(egui::Slider::new(&mut l.opacity, 0.05..=1.0).text(opacity_l));
            if before != (inst.style, l.width, l.opacity) {
                changed = true;
                snapshot = Some(());
            }
        }
        if changed {
            let _ = snapshot;
            self.update_texture(ctx);
        }
    }

    // ------------------------------------------------------------------ redaction

    pub(super) fn redact_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading(tl(self.language, "自動打碼", "Auto Redact"));
        ui.small(tl(self.language,
            "完全離線：在本機偵測文字（中文、英文），自動預選電話、Email、身分證、信用卡、網址、地址。",
            "Fully offline: detects text locally (Chinese & English) and pre-selects phones, emails, TW IDs, cards, URLs and addresses.",
        ));
        let busy = self.ex.detect_rx.is_some();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !busy,
                    egui::Button::new(tl(self.language, "🔎 偵測文字", "🔎 Detect text")),
                )
                .clicked()
            {
                self.start_detection(ctx);
            }
            if busy {
                ui.spinner();
                ui.label(tl(
                    self.language,
                    "偵測中…（第一次會下載約 12 MB 模型）",
                    "Detecting… (first run downloads a ~12 MB model)",
                ));
            }
        });
        ui.horizontal(|ui| {
            ui.label(tl(self.language, "工具", "Tool"));
            ui.selectable_value(
                &mut self.ex.redact_mode,
                RedactMode::Off,
                tl(self.language, "關閉", "Off"),
            );
            ui.selectable_value(
                &mut self.ex.redact_mode,
                RedactMode::Boxes,
                tl(self.language, "▭ 框選", "▭ Boxes"),
            );
            ui.selectable_value(
                &mut self.ex.redact_mode,
                RedactMode::Brush,
                tl(self.language, "🖌 打碼筆刷", "🖌 Redact brush"),
            );
        });
        match self.ex.redact_mode {
            RedactMode::Boxes => {
                ui.small(tl(
                    self.language,
                    "拖曳新增框；點框切換選取；右鍵刪除框。",
                    "Drag to add a box; click a box to toggle; right-click to delete.",
                ));
            }
            RedactMode::Brush => {
                ui.add(
                    egui::Slider::new(&mut self.ex.redact_brush_radius, 2.0..=80.0).text(tl(
                        self.language,
                        "筆刷半徑",
                        "Brush radius",
                    )),
                );
                ui.small(tl(
                    self.language,
                    "筆刷直接以實心顏色寫入像素（不可還原）。",
                    "The brush writes solid colour straight into the pixels (irreversible).",
                ));
            }
            RedactMode::Off => {}
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.ex.redact_keyword)
                    .hint_text(tl(self.language, "關鍵字選取…", "Select by keyword…"))
                    .desired_width(150.0),
            );
            if ui.button(tl(self.language, "選取", "Select")).clicked() {
                let n = select_by_keyword(&mut self.ex.boxes, &self.ex.redact_keyword);
                self.status = format!("{} {n}", tl(self.language, "符合：", "Matched:"));
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(tl(self.language, "預選敏感資料", "Pre-select sensitive"))
                .clicked()
            {
                preselect_sensitive(&mut self.ex.boxes);
            }
            if ui.button(tl(self.language, "全選", "All")).clicked() {
                self.ex.boxes.iter_mut().for_each(|b| b.selected = true);
            }
            if ui.button(tl(self.language, "全不選", "None")).clicked() {
                self.ex.boxes.iter_mut().for_each(|b| b.selected = false);
            }
            if ui
                .button(tl(self.language, "清除框", "Clear boxes"))
                .clicked()
            {
                self.ex.boxes.clear();
            }
        });
        let sel = self.ex.boxes.iter().filter(|b| b.selected).count();
        ui.label(format!(
            "{} {} / {}",
            tl(self.language, "已選取", "Selected"),
            sel,
            self.ex.boxes.len()
        ));
        let mut remove = None;
        egui::ScrollArea::vertical()
            .id_salt("redact_boxes")
            .max_height(220.0)
            .show(ui, |ui| {
                for (i, b) in self.ex.boxes.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut b.selected, "");
                        ui.add(egui::DragValue::new(&mut b.x).prefix("x").speed(1));
                        ui.add(egui::DragValue::new(&mut b.y).prefix("y").speed(1));
                        ui.add(
                            egui::DragValue::new(&mut b.width)
                                .prefix("w")
                                .speed(1)
                                .range(1..=100000),
                        );
                        ui.add(
                            egui::DragValue::new(&mut b.height)
                                .prefix("h")
                                .speed(1)
                                .range(1..=100000),
                        );
                        if ui.small_button("✕").clicked() {
                            remove = Some(i);
                        }
                    });
                    let kind = b
                        .kind
                        .map(|k| format!("[{}] ", k.label()))
                        .unwrap_or_default();
                    if !b.text.is_empty() || !kind.is_empty() {
                        ui.small(format!("{kind}{}", b.text));
                    }
                }
            });
        if let Some(i) = remove {
            self.ex.boxes.remove(i);
        }
        ui.separator();
        ui.label(egui::RichText::new(tl(self.language, "打碼方式", "Redaction style")).strong());
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.ex.redact_style,
                RedactStyleKind::Solid,
                tl(self.language, "實心色塊（建議）", "Solid box (recommended)"),
            );
            ui.selectable_value(
                &mut self.ex.redact_style,
                RedactStyleKind::Mosaic,
                tl(self.language, "馬賽克", "Mosaic"),
            );
            ui.selectable_value(
                &mut self.ex.redact_style,
                RedactStyleKind::Blur,
                tl(self.language, "模糊", "Blur"),
            );
        });
        match self.ex.redact_style {
            RedactStyleKind::Solid => {
                ui.horizontal(|ui| {
                    ui.label(tl(self.language, "顏色", "Colour"));
                    ui.color_edit_button_srgba_unmultiplied(&mut self.ex.redact_color);
                });
            }
            RedactStyleKind::Mosaic => {
                ui.add(
                    egui::Slider::new(&mut self.ex.mosaic_block, 4..=48).text(tl(
                        self.language,
                        "格大小",
                        "Block size",
                    )),
                );
            }
            RedactStyleKind::Blur => {
                ui.add(egui::Slider::new(&mut self.ex.blur_sigma, 2.0..=40.0).text("sigma"));
            }
        }
        if self.ex.redact_style != RedactStyleKind::Solid {
            ui.colored_label(
                Color32::from_rgb(200, 90, 0),
                tl(self.language,
                    "⚠ 馬賽克和模糊有時可被部分還原（尤其是短文字、數字）。機密資料請用實心色塊。",
                    "⚠ Mosaic and blur can sometimes be partially reversed (especially short text and digits). Use a solid box for secrets.",
                ),
            );
        }
        ui.add(
            egui::Slider::new(&mut self.ex.redact_padding, 0..=32).text(tl(
                self.language,
                "外擴邊距 px",
                "Padding px",
            )),
        );
        let apply = ui.add_enabled(
            sel > 0,
            egui::Button::new(
                egui::RichText::new(tl(
                    self.language,
                    "✔ 套用打碼（合併到像素）",
                    "✔ Apply redaction (flatten into pixels)",
                ))
                .strong(),
            ),
        );
        if apply.clicked() {
            self.apply_redaction(ctx);
        }
        ui.small(tl(self.language,
            "套用後會把圖層合併進像素，並在匯出時移除 EXIF／XMP（可在「屬性」中選擇保留）。",
            "Applying flattens layers into the pixels; EXIF/XMP is stripped on export (opt-in keep in Properties).",
        ));
    }

    fn start_detection(&mut self, ctx: &egui::Context) {
        let Some(img) = self.render_current_image() else {
            return;
        };
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let r = (|| -> anyhow::Result<Vec<RedactBox>> {
                let dir = studio_core::ocr::ensure_models()?;
                let det = studio_core::ocr::TextDetector::load(&dir)?;
                det.detect(&img)
            })()
            .map_err(|e| e.to_string());
            let _ = tx.send(r);
            ctx.request_repaint();
        });
        self.ex.detect_rx = Some(rx);
    }

    fn current_redact_style(&self) -> RedactStyle {
        match self.ex.redact_style {
            RedactStyleKind::Solid => RedactStyle::Solid(self.ex.redact_color),
            RedactStyleKind::Mosaic => RedactStyle::Mosaic(self.ex.mosaic_block),
            RedactStyleKind::Blur => RedactStyle::Blur(self.ex.blur_sigma),
        }
    }

    /// Flatten layers + redactions into the working image (undoable within the session).
    fn flatten_layers(&mut self) -> Option<DynamicImage> {
        let flat = self.render_current_image()?;
        self.push_history();
        self.push_image_history();
        self.compose_layers.clear();
        self.selected_layer = None;
        Some(flat)
    }

    fn apply_redaction(&mut self, ctx: &egui::Context) {
        let style = self.current_redact_style();
        let Some(flat) = self.flatten_layers() else {
            return;
        };
        let (out, n) = apply_redactions(&flat, &self.ex.boxes, style, self.ex.redact_padding);
        self.working_image = Some(out);
        self.ex.boxes.retain(|b| !b.selected);
        self.update_texture(ctx);
        self.status = format!(
            "{} {n}",
            tl(self.language, "已打碼區域：", "Redacted regions:")
        );
    }

    /// Canvas input for redaction tools / sticker drops / sticker resize. Returns true when consumed.
    pub(super) fn extras_canvas(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        image_rect: Rect,
        scale: f32,
    ) -> bool {
        let to_img = |p: Pos2| ((p - image_rect.min) / scale).to_pos2();
        if let Some(id) = response.dnd_release_payload::<String>() {
            let at = response.hover_pos().map(to_img);
            self.insert_sticker(ctx, &id, at);
            return true;
        }
        // Sticker / image layer corner resize handle.
        if let Some((i, start)) = self.ex.resize_drag {
            if let Some(p) = ctx.input(|inp| inp.pointer.interact_pos())
                && let Some(ComposeLayer::Image(l)) = self.compose_layers.get_mut(i)
            {
                let cur = to_img(p);
                let w = (cur.x - l.x as f32).max(8.0);
                let ratio = start.y / start.x.max(1.0);
                l.width = w.round() as u32;
                l.height = (w * ratio).round().max(1.0) as u32;
                self.update_texture(ctx);
            }
            if !ctx.input(|inp| inp.pointer.primary_down()) {
                self.ex.resize_drag = None;
            }
            return true;
        }
        if response.drag_started()
            && let (Some(i), Some(p)) = (
                self.selected_layer,
                ctx.input(|inp| inp.pointer.press_origin()),
            )
            && let Some(ComposeLayer::Image(l)) = self.compose_layers.get(i)
        {
            let corner = image_rect.min
                + Vec2::new(
                    (l.x + l.width as i32) as f32,
                    (l.y + l.height as i32) as f32,
                ) * scale;
            if (p - corner).length() < 12.0 {
                let size = Vec2::new(l.width as f32, l.height as f32);
                self.push_history();
                self.ex.resize_drag = Some((i, size));
                return true;
            }
        }
        if self.paint_state.active_inspector_tab != InspectorTab::Redact {
            return false;
        }
        match self.ex.redact_mode {
            RedactMode::Off => false,
            RedactMode::Boxes => {
                if response.secondary_clicked()
                    && let Some(p) = response.interact_pointer_pos()
                {
                    let ip = to_img(p);
                    if let Some(i) = self.ex.boxes.iter().rposition(|b| b.contains(ip.x, ip.y)) {
                        self.ex.boxes.remove(i);
                    }
                } else if response.clicked()
                    && let Some(p) = response.interact_pointer_pos()
                {
                    let ip = to_img(p);
                    if let Some(b) = self
                        .ex
                        .boxes
                        .iter_mut()
                        .rev()
                        .find(|b| b.contains(ip.x, ip.y))
                    {
                        b.selected = !b.selected;
                    }
                }
                if response.drag_started() {
                    self.ex.box_drag_start =
                        ctx.input(|inp| inp.pointer.press_origin()).map(to_img);
                }
                if response.dragged() {
                    self.ex.box_drag_now = response.interact_pointer_pos().map(to_img);
                }
                if response.drag_stopped() {
                    if let (Some(a), Some(b)) =
                        (self.ex.box_drag_start.take(), self.ex.box_drag_now.take())
                    {
                        let r = Rect::from_two_pos(a, b);
                        if r.width() >= 3.0 && r.height() >= 3.0 {
                            self.ex.boxes.push(RedactBox::manual(
                                r.min.x as i32,
                                r.min.y as i32,
                                r.width() as u32,
                                r.height() as u32,
                            ));
                        }
                    }
                }
                true
            }
            RedactMode::Brush => {
                if response.drag_started() || response.clicked() {
                    self.push_image_history();
                }
                if (response.dragged() || response.clicked() || response.drag_started())
                    && let Some(p) = response.interact_pointer_pos()
                {
                    let ip = to_img(p);
                    let from = self.paint_state.last_mouse_image_pos.unwrap_or(ip);
                    let color = self.ex.redact_color;
                    let r = self.ex.redact_brush_radius;
                    if let Some(img) = self.working_image.as_mut() {
                        if !matches!(img, DynamicImage::ImageRgba8(_)) {
                            *img = DynamicImage::ImageRgba8(img.to_rgba8());
                        }
                        if let DynamicImage::ImageRgba8(buf) = img {
                            let steps = ((ip - from).length() / (r * 0.4).max(1.0)).ceil().max(1.0)
                                as usize;
                            for s in 0..=steps {
                                let t = s as f32 / steps as f32;
                                let q = from + (ip - from) * t;
                                redact_brush_dab(buf, q.x, q.y, r, color);
                            }
                        }
                    }
                    self.paint_state.last_mouse_image_pos = Some(ip);
                    self.update_texture(ctx);
                }
                if response.drag_stopped() || response.clicked() {
                    self.paint_state.last_mouse_image_pos = None;
                }
                true
            }
        }
    }

    /// Overlays drawn on top of the canvas: pixel grid, redaction boxes, resize handle.
    pub(super) fn extras_overlay(&self, ui: &egui::Ui, image_rect: Rect, scale: f32) {
        let painter = ui.painter_at(ui.clip_rect());
        if self.ex.show_grid && scale >= 8.0 {
            let clip = image_rect.intersect(ui.clip_rect());
            let stroke = Stroke::new(1.0_f32, Color32::from_black_alpha(40));
            let x0 = ((clip.min.x - image_rect.min.x) / scale).floor() as i32;
            let x1 = ((clip.max.x - image_rect.min.x) / scale).ceil() as i32;
            let y0 = ((clip.min.y - image_rect.min.y) / scale).floor() as i32;
            let y1 = ((clip.max.y - image_rect.min.y) / scale).ceil() as i32;
            for x in x0..=x1 {
                let sx = image_rect.min.x + x as f32 * scale;
                painter.line_segment(
                    [Pos2::new(sx, clip.min.y), Pos2::new(sx, clip.max.y)],
                    stroke,
                );
            }
            for y in y0..=y1 {
                let sy = image_rect.min.y + y as f32 * scale;
                painter.line_segment(
                    [Pos2::new(clip.min.x, sy), Pos2::new(clip.max.x, sy)],
                    stroke,
                );
            }
        }
        let to_screen = |x: f32, y: f32| image_rect.min + Vec2::new(x, y) * scale;
        if self.paint_state.active_inspector_tab == InspectorTab::Redact {
            for b in &self.ex.boxes {
                let r = Rect::from_min_max(
                    to_screen(b.x as f32, b.y as f32),
                    to_screen(
                        (b.x + b.width as i32) as f32,
                        (b.y + b.height as i32) as f32,
                    ),
                );
                let (fill, stroke) = if b.selected {
                    (
                        Color32::from_rgba_unmultiplied(229, 57, 53, 70),
                        Stroke::new(2.0_f32, Color32::from_rgb(229, 57, 53)),
                    )
                } else {
                    (
                        Color32::TRANSPARENT,
                        Stroke::new(1.5_f32, Color32::from_rgb(30, 136, 229)),
                    )
                };
                painter.rect_filled(r, 0.0, fill);
                painter.rect_stroke(r, 0.0, stroke, StrokeKind::Outside);
            }
            if let (Some(a), Some(b)) = (self.ex.box_drag_start, self.ex.box_drag_now) {
                let r = Rect::from_two_pos(to_screen(a.x, a.y), to_screen(b.x, b.y));
                painter.rect_stroke(
                    r,
                    0.0,
                    Stroke::new(2.0_f32, Color32::from_rgb(255, 160, 0)),
                    StrokeKind::Outside,
                );
            }
        }
        if let Some(i) = self.selected_layer
            && let Some(ComposeLayer::Image(l)) = self.compose_layers.get(i)
        {
            let c = to_screen(
                (l.x + l.width as i32) as f32,
                (l.y + l.height as i32) as f32,
            );
            let r = Rect::from_center_size(c, Vec2::splat(10.0));
            painter.rect_filled(r, 2.0, Color32::WHITE);
            painter.rect_stroke(
                r,
                2.0,
                Stroke::new(1.5_f32, Color32::from_rgb(80, 180, 255)),
                StrokeKind::Outside,
            );
        }
    }
}
