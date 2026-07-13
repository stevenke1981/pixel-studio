use std::borrow::Cow;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use base64::{Engine as _, engine::general_purpose};
use chrono::Local;
use eframe::egui;
use egui::{
    Color32, ColorImage, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Vec2,
};
use image::DynamicImage;
use studio_core::transform::SelectionMask;
use studio_core::{
    BatchOptions, BatchSummary, C2paAddOptions, C2paInfo, C2paManifestDraft, C2paRemovalOptions,
    ComposeDocument, ComposeLayer, ConvertOptions, ConvertResult, GifOptions, ImageInfo,
    ImageLayer, PanoramaMode, PanoramaOptions, ShapeKind, ShapeLayer, SupportedFormat, TextLayer,
    collect_supported_files, inspect_c2pa, inspect_image, load_dynamic_image,
    process_batch_with_progress, process_c2pa_add_batch_with_progress,
    process_c2pa_removal_batch_with_progress, remove_c2pa_manifest, render_composition,
    save_dynamic_image, text_layer_bounds, write_animated_gif, write_c2pa_manifest,
    write_panorama_dynamic_jpeg,
};

use crate::i18n::{Language, TextKey};

enum BatchMessage {
    Progress {
        completed: usize,
        total: usize,
    },
    Done(BatchSummary),
    C2paDone {
        summary: BatchSummary,
        mode: C2paBatchMode,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum C2paBatchMode {
    Add,
    Remove,
}

#[derive(Clone)]
struct FontChoice {
    name: String,
    path: PathBuf,
    preview_family: String,
}

#[derive(Clone, Copy)]
enum AlignTarget {
    Left,
    Center,
    Right,
    Top,
    Middle,
    Bottom,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SelectionShape {
    Rectangle,
    Circle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PreviewTool {
    Select,
    Crop,
    Text,
    Line,
    Arrow,
    Rectangle,
    Hand,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Workspace {
    Quick,
    Advanced,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BatchKind {
    Convert,
    C2pa(C2paBatchMode),
}

#[derive(Clone)]
struct CanvasSnapshot {
    image_path: Option<PathBuf>,
    image_info: Option<ImageInfo>,
    c2pa_info: C2paInfo,
    c2pa_draft: C2paManifestDraft,
    c2pa_write_on_save: bool,
    original_image: Option<DynamicImage>,
    working_image: Option<DynamicImage>,
    compose_layers: Vec<ComposeLayer>,
    selected_layer: Option<usize>,
    image_undo_stack: Vec<DynamicImage>,
    image_redo_stack: Vec<DynamicImage>,
    selection_shape: SelectionShape,
    selection_rect: Option<Rect>,
    folder_files: Vec<PathBuf>,
    selected_index: Option<usize>,
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

pub struct PixelStudioApp {
    language: Language,
    workspace: Workspace,
    pending_open: Option<PathBuf>,
    image_path: Option<PathBuf>,
    image_info: Option<ImageInfo>,
    c2pa_info: C2paInfo,
    c2pa_draft: C2paManifestDraft,
    c2pa_write_on_save: bool,
    original_image: Option<DynamicImage>,
    working_image: Option<DynamicImage>,
    texture: Option<TextureHandle>,
    texture_size: Vec2,
    cleared_canvas: Option<CanvasSnapshot>,
    compose_layers: Vec<ComposeLayer>,
    selected_layer: Option<usize>,
    undo_stack: Vec<Vec<ComposeLayer>>,
    redo_stack: Vec<Vec<ComposeLayer>>,
    image_undo_stack: Vec<DynamicImage>,
    image_redo_stack: Vec<DynamicImage>,
    drag_snapshot: Option<Vec<ComposeLayer>>,
    selection_shape: SelectionShape,
    selection_rect: Option<Rect>,
    selection_drag_start: Option<Pos2>,
    font_bytes: Option<Vec<u8>>,
    font_choices: Vec<FontChoice>,
    folder_files: Vec<PathBuf>,
    selected_index: Option<usize>,
    watermark_text: String,
    watermark_opacity: f32,
    watermark_font_size: f32,
    zoom: f32,
    fit_to_window: bool,
    preview_pan: Vec2,
    preview_tool: PreviewTool,
    output_format: SupportedFormat,
    quality: u8,
    resize_width: u32,
    resize_height: u32,
    preserve_aspect: bool,
    crop_x: u32,
    crop_y: u32,
    crop_width: u32,
    crop_height: u32,
    brightness_value: i32,
    contrast_value: f32,
    saturation_value: f32,
    exposure_value: f32,
    blur_sigma: f32,
    sharpen_sigma: f32,
    sharpen_threshold: i32,
    hue_degrees: f32,
    sepia_strength: f32,
    vignette_strength: f32,
    temperature_value: f32,
    tint_value: f32,
    posterize_levels: u8,
    panorama_mode: PanoramaMode,
    panorama_width: u32,
    gif_delay_ms: u32,
    gif_repeat: bool,
    gif_resize_enabled: bool,
    gif_max_width: u32,
    gif_max_height: u32,
    batch_input_dir: Option<PathBuf>,
    batch_output_dir: Option<PathBuf>,
    batch_resize_enabled: bool,
    batch_max_width: u32,
    batch_max_height: u32,
    batch_watermark_enabled: bool,
    batch_watermark_opacity: f32,
    batch_watermark_font_size: f32,
    c2pa_batch_input_dir: Option<PathBuf>,
    c2pa_batch_output_dir: Option<PathBuf>,
    batch_receiver: Option<Receiver<BatchMessage>>,
    batch_summary: Option<BatchSummary>,
    active_batch_kind: Option<BatchKind>,
    c2pa_batch_result: Option<(C2paBatchMode, BatchSummary)>,
    batch_completed: usize,
    batch_total: usize,
    status: String,
}

impl PixelStudioApp {
    pub fn new(initial_path: Option<PathBuf>) -> Self {
        Self {
            language: Language::ZhTw,
            workspace: Workspace::Quick,
            pending_open: initial_path,
            image_path: None,
            image_info: None,
            c2pa_info: C2paInfo::default(),
            c2pa_draft: C2paManifestDraft::default(),
            c2pa_write_on_save: false,
            original_image: None,
            working_image: None,
            texture: None,
            texture_size: Vec2::ZERO,
            cleared_canvas: None,
            compose_layers: Vec::new(),
            selected_layer: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            image_undo_stack: Vec::new(),
            image_redo_stack: Vec::new(),
            drag_snapshot: None,
            selection_shape: SelectionShape::Rectangle,
            selection_rect: None,
            selection_drag_start: None,
            font_bytes: crate::fonts::load_system_font_bytes(),
            font_choices: load_font_choices(),
            folder_files: Vec::new(),
            selected_index: None,
            watermark_text: "Pixel Studio".to_owned(),
            watermark_opacity: 0.45,
            watermark_font_size: 48.0,
            zoom: 1.0,
            fit_to_window: true,
            preview_pan: Vec2::ZERO,
            preview_tool: PreviewTool::Select,
            output_format: SupportedFormat::Png,
            quality: 92,
            resize_width: 1,
            resize_height: 1,
            preserve_aspect: true,
            crop_x: 0,
            crop_y: 0,
            crop_width: 1,
            crop_height: 1,
            brightness_value: 0,
            contrast_value: 0.0,
            saturation_value: 0.0,
            exposure_value: 0.0,
            blur_sigma: 2.0,
            sharpen_sigma: 1.0,
            sharpen_threshold: 8,
            hue_degrees: 0.0,
            sepia_strength: 0.5,
            vignette_strength: 0.5,
            temperature_value: 0.0,
            tint_value: 0.0,
            posterize_levels: 4,
            panorama_mode: PanoramaMode::Pad,
            panorama_width: 2048,
            gif_delay_ms: 120,
            gif_repeat: true,
            gif_resize_enabled: false,
            gif_max_width: 800,
            gif_max_height: 800,
            batch_input_dir: None,
            batch_output_dir: None,
            batch_resize_enabled: false,
            batch_max_width: 2048,
            batch_max_height: 2048,
            batch_watermark_enabled: false,
            batch_watermark_opacity: 0.45,
            batch_watermark_font_size: 48.0,
            c2pa_batch_input_dir: None,
            c2pa_batch_output_dir: None,
            batch_receiver: None,
            batch_summary: None,
            active_batch_kind: None,
            c2pa_batch_result: None,
            batch_completed: 0,
            batch_total: 0,
            status: "請開啟圖片或資料夾。".to_owned(),
        }
    }
}

impl Default for PixelStudioApp {
    fn default() -> Self {
        Self::new(None)
    }
}

impl eframe::App for PixelStudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_batch(ui.ctx());
        self.open_pending_path(ui.ctx());
        self.open_dropped_files(ui.ctx());
        self.handle_keyboard_shortcuts(ui.ctx());

        let ctx = ui.ctx().clone();
        egui::Panel::top("toolbar").show_inside(ui, |ui| self.toolbar(ui, &ctx));
        egui::Panel::left("metadata")
            .resizable(true)
            .default_size(250.0)
            .min_size(200.0)
            .show_inside(ui, |ui| self.metadata_panel(ui, &ctx));
        egui::Panel::right("actions")
            .resizable(true)
            .default_size(300.0)
            .min_size(240.0)
            .show_inside(ui, |ui| self.actions_panel(ui));
        egui::Panel::bottom("status").show_inside(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{}:", self.tr(TextKey::Status)));
                ui.label(&self.status);
                if let Some(image) = &self.working_image {
                    ui.separator();
                    ui.label(format!("{} × {}", image.width(), image.height()));
                }
                ui.separator();
                ui.label(format!("Zoom {:.0}%", self.zoom * 100.0));
                if let Some(selection) = self.selection_rect {
                    ui.separator();
                    ui.label(format!(
                        "Selection {:.0} × {:.0}",
                        selection.width(),
                        selection.height()
                    ));
                }
            });
        });
        egui::CentralPanel::default_margins().show_inside(ui, |ui| self.preview_panel(ui));
    }
}

impl PixelStudioApp {
    fn tr(&self, key: TextKey) -> &'static str {
        self.language.t(key)
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            if ui.button(self.tr(TextKey::CaptureScreenshot)).clicked() {
                self.capture_screenshot(ctx);
            }

            if ui.button(self.tr(TextKey::OpenImage)).clicked()
                && let Some(path) = rfd::FileDialog::new().pick_file()
            {
                self.open_path(ctx, path);
            }

            if ui.button(self.tr(TextKey::PasteImage)).clicked() {
                self.paste_clipboard_image(ctx);
            }

            if ui
                .add_enabled(
                    self.working_image.is_some(),
                    egui::Button::new(self.tr(TextKey::SaveAsNewFile)),
                )
                .clicked()
            {
                self.save_as_new_file();
            }

            if ui
                .add_enabled(
                    self.can_undo_image(),
                    egui::Button::new(self.tr(TextKey::ImageUndo)),
                )
                .clicked()
            {
                self.undo_image_edit(ctx);
            }
            if ui
                .add_enabled(
                    self.can_redo_image(),
                    egui::Button::new(self.tr(TextKey::ImageRedo)),
                )
                .clicked()
            {
                self.redo_image_edit(ctx);
            }

            ui.separator();

            let select_label = self.tr(TextKey::SelectTool);
            let pan_label = self.tr(TextKey::PanTool);
            let crop_label = self.tr(TextKey::CropTool);
            let text_label = self.tr(TextKey::TextTool);
            let line_label = self.tr(TextKey::LineTool);
            let arrow_label = self.tr(TextKey::ArrowTool);
            let rectangle_label = self.tr(TextKey::RectangleTool);
            ui.selectable_value(&mut self.preview_tool, PreviewTool::Select, select_label)
                .on_hover_text("Select and move layers");
            ui.selectable_value(&mut self.preview_tool, PreviewTool::Crop, crop_label)
                .on_hover_text("Drag a crop area, Enter applies, Esc cancels");
            ui.selectable_value(&mut self.preview_tool, PreviewTool::Text, text_label)
                .on_hover_text("Click the canvas to add text");
            ui.selectable_value(&mut self.preview_tool, PreviewTool::Line, line_label)
                .on_hover_text("Drag to draw a line");
            ui.selectable_value(&mut self.preview_tool, PreviewTool::Arrow, arrow_label)
                .on_hover_text("Drag to draw an arrow");
            ui.selectable_value(
                &mut self.preview_tool,
                PreviewTool::Rectangle,
                rectangle_label,
            )
            .on_hover_text("Drag to draw a rectangle");
            ui.selectable_value(&mut self.preview_tool, PreviewTool::Hand, pan_label)
                .on_hover_text("Pan the canvas");

            ui.separator();

            let quick_workspace_label = self.tr(TextKey::QuickAnnotate);
            let advanced_workspace_label = self.tr(TextKey::AdvancedTools);
            ui.selectable_value(&mut self.workspace, Workspace::Quick, quick_workspace_label);
            ui.selectable_value(
                &mut self.workspace,
                Workspace::Advanced,
                advanced_workspace_label,
            );

            if ui.button(self.tr(TextKey::Fit)).clicked() {
                self.fit_to_window = true;
                self.preview_pan = Vec2::ZERO;
            }
            if ui.button("100%").clicked() {
                self.fit_to_window = false;
                self.zoom = 1.0;
                self.preview_pan = Vec2::ZERO;
            }
            if ui.button(self.tr(TextKey::ZoomOut)).clicked() {
                self.fit_to_window = false;
                self.zoom = (self.zoom / 1.25).clamp(0.05, 8.0);
            }
            if ui.button(self.tr(TextKey::ZoomIn)).clicked() {
                self.fit_to_window = false;
                self.zoom = (self.zoom * 1.25).clamp(0.05, 8.0);
            }
        });

        egui::CollapsingHeader::new(self.tr(TextKey::AdvancedTools))
            .default_open(self.workspace == Workspace::Advanced)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(self.tr(TextKey::Project));
                    if ui.button(self.tr(TextKey::OpenFolder)).clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_folder()
                    {
                        self.open_folder(ctx, path);
                    }
                    if ui.button(self.tr(TextKey::OpenProject)).clicked() {
                        self.open_project(ctx);
                    }
                    if ui.button(self.tr(TextKey::SaveProject)).clicked()
                        && self.working_image.is_some()
                    {
                        self.save_project();
                    }
                    if ui.button(self.tr(TextKey::ClearCanvas)).clicked()
                        && self.working_image.is_some()
                    {
                        self.clear_canvas(ctx);
                    }
                    if ui.button(self.tr(TextKey::RestoreCanvas)).clicked()
                        && self.cleared_canvas.is_some()
                    {
                        self.restore_canvas(ctx);
                    }
                    if ui.button(self.tr(TextKey::Reload)).clicked()
                        && let Some(path) = self.image_path.clone()
                    {
                        self.open_image(ctx, path);
                    }
                    if ui.button(self.tr(TextKey::Previous)).clicked()
                        && let Some(index) = self.previous_index()
                    {
                        self.open_folder_index(ctx, index);
                    }
                    if ui.button("下一張 / Next").clicked()
                        && let Some(index) = self.next_index()
                    {
                        self.open_folder_index(ctx, index);
                    }
                    egui::ComboBox::from_label(self.tr(TextKey::Language))
                        .selected_text(self.language.name())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.language,
                                Language::ZhTw,
                                Language::ZhTw.name(),
                            );
                            ui.selectable_value(
                                &mut self.language,
                                Language::En,
                                Language::En.name(),
                            );
                        });
                });
            });
    }

    fn metadata_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.quick_tool_rail(ui);
        ui.separator();
        ui.heading(self.tr(TextKey::Image));
        ui.separator();

        match &self.image_info {
            Some(info) => {
                ui.label(format!(
                    "{}: {}",
                    self.tr(TextKey::File),
                    info.path.display()
                ));
                ui.label(format!("{}: {}", self.tr(TextKey::Format), info.format));
                ui.label(format!(
                    "{}: {} x {}",
                    self.tr(TextKey::Source),
                    info.width,
                    info.height
                ));
                if let Some(image) = &self.working_image {
                    ui.label(format!(
                        "{}: {} x {}",
                        self.tr(TextKey::Current),
                        image.width(),
                        image.height()
                    ));
                }
                ui.label(format!("Color: {}", info.color_type));
                ui.label(format!("Bytes: {}", info.file_size));
            }
            None => {
                if let Some(image) = &self.working_image {
                    ui.label(format!(
                        "{}: {}",
                        self.tr(TextKey::File),
                        self.tr(TextKey::ClipboardImage)
                    ));
                    ui.label(format!(
                        "{}: {} x {}",
                        self.tr(TextKey::Current),
                        image.width(),
                        image.height()
                    ));
                } else {
                    ui.label(self.tr(TextKey::NoImage));
                }
            }
        }

        ui.add_space(16.0);
        self.c2pa_panel(ui);

        ui.add_space(16.0);
        ui.heading(self.tr(TextKey::Folder));
        ui.separator();
        ui.label(format!(
            "{}: {}",
            self.tr(TextKey::Images),
            self.folder_files.len()
        ));

        egui::ScrollArea::vertical()
            .max_height(520.0)
            .show(ui, |ui| {
                for index in 0..self.folder_files.len() {
                    let file_name = self.folder_files[index]
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("(image)")
                        .to_owned();
                    let selected = self.selected_index == Some(index);
                    if ui.selectable_label(selected, file_name).clicked() {
                        self.open_folder_index(ctx, index);
                    }
                }
            });
    }

    fn quick_tool_rail(&mut self, ui: &mut egui::Ui) {
        if self.workspace != Workspace::Quick {
            return;
        }
        ui.label(self.tr(TextKey::QuickAnnotate));
        ui.horizontal_wrapped(|ui| {
            for (tool, label, hint) in [
                (
                    PreviewTool::Select,
                    self.tr(TextKey::SelectTool),
                    "Select or move a layer",
                ),
                (
                    PreviewTool::Crop,
                    self.tr(TextKey::CropTool),
                    "Drag crop area; Enter applies; Esc cancels",
                ),
                (
                    PreviewTool::Text,
                    self.tr(TextKey::TextTool),
                    "Click canvas to add text",
                ),
                (
                    PreviewTool::Line,
                    self.tr(TextKey::LineTool),
                    "Drag to draw a line",
                ),
                (
                    PreviewTool::Arrow,
                    self.tr(TextKey::ArrowTool),
                    "Drag to draw an arrow",
                ),
                (
                    PreviewTool::Rectangle,
                    self.tr(TextKey::RectangleTool),
                    "Drag to draw a rectangle",
                ),
                (PreviewTool::Hand, self.tr(TextKey::PanTool), "Pan canvas"),
            ] {
                ui.selectable_value(&mut self.preview_tool, tool, label)
                    .on_hover_text(hint);
            }
        });
        ui.label(self.tr(TextKey::QuickHint));
    }

    fn c2pa_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.tr(TextKey::C2pa));
        ui.separator();

        if self.c2pa_info.present {
            ui.label(format!(
                "{}: {}",
                self.tr(TextKey::Status),
                self.c2pa_info
                    .validation_state
                    .as_deref()
                    .unwrap_or("Unknown")
            ));
            ui.label(format!(
                "{}: {}",
                self.tr(TextKey::Manifests),
                self.c2pa_info.manifest_count
            ));
        } else {
            ui.label(self.tr(TextKey::C2paMissing));
        }

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(self.tr(TextKey::C2paSingleCopy));

            let write_on_save = self.tr(TextKey::C2paWriteOnSave);
            ui.checkbox(&mut self.c2pa_write_on_save, write_on_save);
            ui.add_enabled_ui(self.c2pa_write_on_save, |ui| {
                self.c2pa_draft_fields(ui, "single_c2pa_draft_fields");
            });

            ui.horizontal(|ui| {
                if ui.button(self.tr(TextKey::AddC2paCopy)).clicked() {
                    self.save_c2pa_copy();
                }

                let remove_label = self.tr(TextKey::RemoveC2paCopy);
                ui.add_enabled_ui(self.image_path.is_some(), |ui| {
                    if ui.button(remove_label).clicked() {
                        self.remove_c2pa_copy();
                    }
                });
            });
        });

        ui.add_space(10.0);
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(self.tr(TextKey::BatchC2pa));

            self.c2pa_batch_folder_fields(ui);

            let can_run_c2pa = self.c2pa_batch_input_dir.is_some()
                && self.c2pa_batch_output_dir.is_some()
                && self.batch_receiver.is_none();

            ui.separator();
            ui.label(self.tr(TextKey::BatchC2paRemove));
            if ui
                .add_enabled(
                    can_run_c2pa,
                    egui::Button::new(self.tr(TextKey::BatchC2paRunRemove)),
                )
                .clicked()
            {
                self.start_c2pa_batch(C2paBatchMode::Remove);
            }

            ui.separator();
            ui.label(self.tr(TextKey::BatchC2paAdd));
            self.c2pa_draft_fields(ui, "batch_c2pa_draft_fields");
            if ui
                .add_enabled(
                    can_run_c2pa,
                    egui::Button::new(self.tr(TextKey::BatchC2paRunAdd)),
                )
                .clicked()
            {
                self.start_c2pa_batch(C2paBatchMode::Add);
            }

            if matches!(self.active_batch_kind, Some(BatchKind::C2pa(_)))
                && self.batch_receiver.is_some()
            {
                let progress = if self.batch_total == 0 {
                    0.0
                } else {
                    self.batch_completed as f32 / self.batch_total as f32
                };
                ui.add(
                    egui::ProgressBar::new(progress)
                        .animate(true)
                        .text(format!("{} / {}", self.batch_completed, self.batch_total)),
                );
                ui.label(self.tr(TextKey::BatchRunning));
            }

            if let Some((mode, summary)) = &self.c2pa_batch_result {
                let heading = match mode {
                    C2paBatchMode::Remove => self.tr(TextKey::BatchC2paRemoveDone),
                    C2paBatchMode::Add => self.tr(TextKey::BatchC2paAddDone),
                };
                let completion = match self.language {
                    Language::ZhTw => format!(
                        "{heading}：共 {}，成功 {}，失敗 {}",
                        summary.total, summary.succeeded, summary.failed
                    ),
                    Language::En => format!(
                        "{heading}: {} total, {} succeeded, {} failed",
                        summary.total, summary.succeeded, summary.failed
                    ),
                };
                ui.colored_label(egui::Color32::from_rgb(40, 160, 90), completion);
                egui::ScrollArea::vertical()
                    .max_height(120.0)
                    .show(ui, |ui| {
                        for result in summary.results.iter().take(8) {
                            let label = match (&result.output, &result.error) {
                                (Some(output), None) => format!("OK {}", output.display()),
                                (_, Some(error)) => {
                                    format!("Failed {}: {error}", result.input.display())
                                }
                                _ => format!("Skipped {}", result.input.display()),
                            };
                            ui.label(label);
                        }
                    });
            }
        });

        if let Some(raw_json) = &self.c2pa_info.raw_json {
            egui::CollapsingHeader::new(self.tr(TextKey::C2paRawJson))
                .default_open(false)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(160.0)
                        .show(ui, |ui| {
                            ui.monospace(raw_json);
                        });
                });
        }
    }

    fn c2pa_batch_folder_fields(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("c2pa_batch_folders")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                let input_label = self.tr(TextKey::BatchC2paInputFolder);
                if ui.button(input_label).clicked()
                    && let Some(folder) = rfd::FileDialog::new().pick_folder()
                {
                    self.c2pa_batch_input_dir = Some(folder);
                }
                ui.label(
                    self.c2pa_batch_input_dir
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "-".to_owned()),
                );
                ui.end_row();

                let output_label = self.tr(TextKey::BatchC2paOutputFolder);
                if ui.button(output_label).clicked()
                    && let Some(folder) = rfd::FileDialog::new().pick_folder()
                {
                    self.c2pa_batch_output_dir = Some(folder);
                }
                ui.label(
                    self.c2pa_batch_output_dir
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "-".to_owned()),
                );
                ui.end_row();
            });
    }

    fn c2pa_draft_fields(&mut self, ui: &mut egui::Ui, id: &'static str) {
        egui::Grid::new(id)
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(self.tr(TextKey::Title));
                ui.text_edit_singleline(&mut self.c2pa_draft.title);
                ui.end_row();

                ui.label(self.tr(TextKey::Creator));
                ui.text_edit_singleline(&mut self.c2pa_draft.creator);
                ui.end_row();

                ui.label(self.tr(TextKey::ActionNote));
                ui.text_edit_multiline(&mut self.c2pa_draft.action);
                ui.end_row();
            });
    }

    fn actions_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.tr(TextKey::Actions));
        ui.separator();
        let ctx = ui.ctx().clone();
        if self.workspace == Workspace::Quick {
            self.quick_panel(ui, &ctx);
            ui.separator();
        }

        egui::CollapsingHeader::new(self.tr(TextKey::AdvancedTools))
            .default_open(self.workspace == Workspace::Advanced)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let ctx = ui.ctx().clone();
                    self.convert_panel(ui);
                    ui.add_space(20.0);
                    self.edit_panel(ui, &ctx);
                    ui.add_space(20.0);
                    self.compose_panel(ui, &ctx);
                    ui.add_space(20.0);
                    self.panorama_panel(ui);
                    ui.add_space(20.0);
                    self.gif_panel(ui, &ctx);
                    ui.add_space(20.0);
                    self.batch_panel(ui);
                });
            });
    }

    fn quick_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading(self.tr(TextKey::QuickAnnotate));
        ui.label(self.tr(TextKey::QuickHint));

        let has_image = self.working_image.is_some();
        let has_selection = has_image && self.selection_rect.is_some();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    has_image,
                    egui::Button::new(self.tr(TextKey::SaveAsNewFile)),
                )
                .clicked()
            {
                self.save_as_new_file();
            }
            if ui
                .add_enabled(
                    self.can_undo_image(),
                    egui::Button::new(self.tr(TextKey::ImageUndo)),
                )
                .clicked()
            {
                self.undo_image_edit(ctx);
            }
            if ui
                .add_enabled(
                    self.can_redo_image(),
                    egui::Button::new(self.tr(TextKey::ImageRedo)),
                )
                .clicked()
            {
                self.redo_image_edit(ctx);
            }
        });

        ui.group(|ui| {
            let privacy_title = self.tr(TextKey::QuickPrivacyTools);
            let blur_hint = self.tr(TextKey::QuickBlurHint);
            let c2pa_hint = self.tr(TextKey::QuickC2paHint);
            let webp_hint = self.tr(TextKey::QuickWebpHint);
            let select_label = self.tr(TextKey::SelectTool);
            ui.strong(privacy_title);
            ui.label(blur_hint);
            let blur_label = self.tr(TextKey::Blur);
            ui.add(egui::Slider::new(&mut self.blur_sigma, 2.0..=30.0).text(blur_label));
            ui.horizontal_wrapped(|ui| {
                if ui
                    .selectable_value(&mut self.preview_tool, PreviewTool::Select, select_label)
                    .on_hover_text(blur_hint)
                    .clicked()
                {
                    self.selection_shape = SelectionShape::Rectangle;
                    self.selected_layer = None;
                }
                if ui
                    .add_enabled(
                        has_selection,
                        egui::Button::new(self.tr(TextKey::BlurSelection)),
                    )
                    .on_hover_text(blur_hint)
                    .clicked()
                {
                    let sigma = self.blur_sigma;
                    self.map_scoped_working_image(ctx, |image| {
                        studio_core::transform::blur(image, sigma)
                    });
                    self.selection_rect = None;
                    self.selection_drag_start = None;
                    self.preview_tool = PreviewTool::Select;
                }

                if ui
                    .add_enabled(
                        self.image_path.is_some() && self.c2pa_info.present,
                        egui::Button::new(self.tr(TextKey::RemoveC2paQuick)),
                    )
                    .on_hover_text(c2pa_hint)
                    .clicked()
                {
                    self.remove_c2pa_copy();
                }

                if ui
                    .add_enabled(
                        has_image,
                        egui::Button::new(self.tr(TextKey::SaveWebpQuick)),
                    )
                    .on_hover_text(webp_hint)
                    .clicked()
                {
                    self.output_format = SupportedFormat::Webp;
                    self.save_as_new_file();
                }
            });
        });

        if let Some(selection) = self.selection_rect {
            ui.group(|ui| {
                ui.label(self.tr(TextKey::CropRectangle));
                ui.label(format!(
                    "X {:.0}  Y {:.0}  W {:.0}  H {:.0}",
                    selection.min.x,
                    selection.min.y,
                    selection.width(),
                    selection.height()
                ));
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(has_image, egui::Button::new(self.tr(TextKey::ApplyCrop)))
                        .clicked()
                    {
                        self.apply_selection_crop(ctx);
                    }
                    if ui.button(self.tr(TextKey::CancelCrop)).clicked() {
                        self.selection_rect = None;
                        self.selection_drag_start = None;
                        self.preview_tool = PreviewTool::Select;
                    }
                });
            });
        }

        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(has_image, egui::Button::new(self.tr(TextKey::AddText)))
                .clicked()
            {
                self.add_text_layer(ctx);
                self.preview_tool = PreviewTool::Select;
            }
            if ui
                .add_enabled(has_image, egui::Button::new(self.tr(TextKey::Compose)))
                .clicked()
            {
                self.workspace = Workspace::Advanced;
            }
        });

        if !self.compose_layers.is_empty() {
            egui::CollapsingHeader::new(self.tr(TextKey::Layers))
                .default_open(true)
                .show(ui, |ui| {
                    for index in 0..self.compose_layers.len() {
                        let label = format!(
                            "{} {}",
                            if self.compose_layers[index].is_visible() {
                                "●"
                            } else {
                                "○"
                            },
                            self.compose_layers[index].name()
                        );
                        if ui
                            .selectable_label(self.selected_layer == Some(index), label)
                            .clicked()
                        {
                            self.selected_layer = Some(index);
                        }
                    }
                    if self.selected_layer.is_some() {
                        let before_edit = self.compose_layers.clone();
                        if self.selected_layer_editor(ui) {
                            self.push_history_snapshot(before_edit);
                            self.update_texture(ctx);
                        }
                    }
                });
        }
    }

    fn convert_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.tr(TextKey::Convert));
        ui.separator();

        egui::ComboBox::from_label(self.tr(TextKey::Format))
            .selected_text(self.output_format.label())
            .show_ui(ui, |ui| {
                for format in SupportedFormat::ALL {
                    ui.selectable_value(&mut self.output_format, format, format.label());
                }
            });

        let quality_label = self.tr(TextKey::Quality);
        ui.add(egui::Slider::new(&mut self.quality, 1..=100).text(quality_label));

        let has_image = self.working_image.is_some();
        if ui
            .add_enabled(
                has_image,
                egui::Button::new(self.tr(TextKey::SaveConverted)),
            )
            .clicked()
        {
            self.save_converted();
        }
    }

    fn edit_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading(self.tr(TextKey::Edit));
        ui.separator();

        let has_image = self.working_image.is_some();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    self.can_undo_image(),
                    egui::Button::new(self.tr(TextKey::ImageUndo)),
                )
                .clicked()
            {
                self.undo_image_edit(ctx);
            }
            if ui
                .add_enabled(
                    self.can_redo_image(),
                    egui::Button::new(self.tr(TextKey::ImageRedo)),
                )
                .clicked()
            {
                self.redo_image_edit(ctx);
            }
        });

        ui.label(self.tr(TextKey::Selection));
        let selection_rectangle_label = self.tr(TextKey::SelectionRectangle);
        let selection_circle_label = self.tr(TextKey::SelectionCircle);
        let clear_selection_label = self.tr(TextKey::ClearSelection);
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(
                &mut self.selection_shape,
                SelectionShape::Rectangle,
                selection_rectangle_label,
            );
            ui.selectable_value(
                &mut self.selection_shape,
                SelectionShape::Circle,
                selection_circle_label,
            );
            if ui
                .add_enabled(
                    self.selection_rect.is_some(),
                    egui::Button::new(clear_selection_label),
                )
                .clicked()
            {
                self.selection_rect = None;
                self.selection_drag_start = None;
            }
        });

        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(has_image, egui::Button::new(self.tr(TextKey::RotateLeft)))
                .clicked()
            {
                self.map_working_image(ctx, studio_core::transform::rotate_left);
            }
            if ui
                .add_enabled(has_image, egui::Button::new(self.tr(TextKey::RotateRight)))
                .clicked()
            {
                self.map_working_image(ctx, studio_core::transform::rotate_right);
            }
            if ui
                .add_enabled(has_image, egui::Button::new(self.tr(TextKey::Rotate180)))
                .clicked()
            {
                self.map_working_image(ctx, studio_core::filters::rotate_180);
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(has_image, egui::Button::new(self.tr(TextKey::FlipH)))
                .clicked()
            {
                self.map_working_image(ctx, studio_core::transform::flip_horizontal);
            }
            if ui
                .add_enabled(has_image, egui::Button::new(self.tr(TextKey::FlipV)))
                .clicked()
            {
                self.map_working_image(ctx, studio_core::transform::flip_vertical);
            }
        });

        ui.checkbox(
            &mut self.preserve_aspect,
            self.language.t(TextKey::PreserveAspect),
        );
        ui.horizontal(|ui| {
            let width_changed = ui
                .add(
                    egui::DragValue::new(&mut self.resize_width)
                        .range(1..=65535)
                        .prefix("W "),
                )
                .changed();
            let height_changed = ui
                .add(
                    egui::DragValue::new(&mut self.resize_height)
                        .range(1..=65535)
                        .prefix("H "),
                )
                .changed();
            if self.preserve_aspect {
                self.sync_resize_aspect(width_changed, height_changed);
            }
        });
        if ui
            .add_enabled(has_image, egui::Button::new(self.tr(TextKey::ApplyResize)))
            .clicked()
        {
            self.apply_resize(ctx);
        }

        ui.label(self.tr(TextKey::CropRectangle));
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::DragValue::new(&mut self.crop_x)
                    .range(0..=65535)
                    .prefix("X "),
            );
            ui.add(
                egui::DragValue::new(&mut self.crop_y)
                    .range(0..=65535)
                    .prefix("Y "),
            );
            ui.add(
                egui::DragValue::new(&mut self.crop_width)
                    .range(1..=65535)
                    .prefix("W "),
            );
            ui.add(
                egui::DragValue::new(&mut self.crop_height)
                    .range(1..=65535)
                    .prefix("H "),
            );
        });
        if ui
            .add_enabled(has_image, egui::Button::new(self.tr(TextKey::ApplyCrop)))
            .clicked()
        {
            self.apply_crop(ctx);
        }

        ui.separator();
        ui.label(self.tr(TextKey::Adjustments));
        let brightness_label = self.tr(TextKey::Brightness);
        ui.add(egui::Slider::new(&mut self.brightness_value, -255..=255).text(brightness_label));
        if ui
            .add_enabled(
                has_image,
                egui::Button::new(self.tr(TextKey::ApplyBrightness)),
            )
            .clicked()
        {
            let value = self.brightness_value;
            self.map_scoped_working_image(ctx, |image| {
                studio_core::transform::adjust_brightness(image, value)
            });
        }

        let contrast_label = self.tr(TextKey::Contrast);
        ui.add(egui::Slider::new(&mut self.contrast_value, -100.0..=100.0).text(contrast_label));
        if ui
            .add_enabled(
                has_image,
                egui::Button::new(self.tr(TextKey::ApplyContrast)),
            )
            .clicked()
        {
            let value = self.contrast_value;
            self.map_scoped_working_image(ctx, |image| {
                studio_core::transform::adjust_contrast(image, value)
            });
        }

        let saturation_label = self.tr(TextKey::Saturation);
        ui.add(
            egui::Slider::new(&mut self.saturation_value, -100.0..=100.0).text(saturation_label),
        );
        if ui
            .add_enabled(
                has_image,
                egui::Button::new(self.tr(TextKey::ApplySaturation)),
            )
            .clicked()
        {
            let value = self.saturation_value;
            self.map_scoped_working_image(ctx, |image| {
                studio_core::transform::adjust_saturation(image, value)
            });
        }

        let exposure_label = self.tr(TextKey::Exposure);
        ui.add(egui::Slider::new(&mut self.exposure_value, -2.0..=2.0).text(exposure_label));
        if ui
            .add_enabled(
                has_image,
                egui::Button::new(self.tr(TextKey::ApplyExposure)),
            )
            .clicked()
        {
            let value = self.exposure_value;
            self.map_scoped_working_image(ctx, |image| {
                studio_core::transform::adjust_exposure(image, value)
            });
        }

        let blur_label = self.tr(TextKey::Blur);
        ui.add(egui::Slider::new(&mut self.blur_sigma, 0.1..=20.0).text(blur_label));
        if ui
            .add_enabled(has_image, egui::Button::new(self.tr(TextKey::ApplyBlur)))
            .clicked()
        {
            let sigma = self.blur_sigma;
            self.map_scoped_working_image(ctx, |image| studio_core::transform::blur(image, sigma));
        }

        let sharpen_label = self.tr(TextKey::Sharpen);
        let threshold_label = self.tr(TextKey::Threshold);
        ui.add(egui::Slider::new(&mut self.sharpen_sigma, 0.1..=10.0).text(sharpen_label));
        ui.add(egui::Slider::new(&mut self.sharpen_threshold, 0..=255).text(threshold_label));
        if ui
            .add_enabled(has_image, egui::Button::new(self.tr(TextKey::ApplySharpen)))
            .clicked()
        {
            let sigma = self.sharpen_sigma;
            let threshold = self.sharpen_threshold;
            self.map_scoped_working_image(ctx, |image| {
                studio_core::transform::sharpen(image, sigma, threshold)
            });
        }

        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(has_image, egui::Button::new(self.tr(TextKey::Grayscale)))
                .clicked()
            {
                self.map_scoped_working_image(ctx, studio_core::transform::grayscale);
            }
            if ui
                .add_enabled(has_image, egui::Button::new(self.tr(TextKey::Invert)))
                .clicked()
            {
                self.map_scoped_working_image(ctx, studio_core::transform::invert);
            }
            if ui
                .add_enabled(has_image, egui::Button::new(self.tr(TextKey::AutoContrast)))
                .clicked()
            {
                self.map_scoped_working_image(ctx, studio_core::filters::auto_contrast);
            }
        });

        ui.separator();
        ui.label(self.tr(TextKey::Filters));

        let hue_label = self.tr(TextKey::Hue);
        ui.add(egui::Slider::new(&mut self.hue_degrees, -180.0..=180.0).text(hue_label));
        if ui
            .add_enabled(has_image, egui::Button::new(self.tr(TextKey::ApplyHue)))
            .clicked()
        {
            let degrees = self.hue_degrees;
            self.map_scoped_working_image(ctx, |image| {
                studio_core::filters::adjust_hue(image, degrees)
            });
        }

        let sepia_label = self.tr(TextKey::Sepia);
        ui.add(egui::Slider::new(&mut self.sepia_strength, 0.0..=1.0).text(sepia_label));
        if ui
            .add_enabled(has_image, egui::Button::new(self.tr(TextKey::ApplySepia)))
            .clicked()
        {
            let strength = self.sepia_strength;
            self.map_scoped_working_image(ctx, |image| {
                studio_core::filters::sepia(image, strength)
            });
        }

        let vignette_label = self.tr(TextKey::Vignette);
        ui.add(egui::Slider::new(&mut self.vignette_strength, 0.0..=1.0).text(vignette_label));
        if ui
            .add_enabled(
                has_image,
                egui::Button::new(self.tr(TextKey::ApplyVignette)),
            )
            .clicked()
        {
            let strength = self.vignette_strength;
            self.map_scoped_working_image(ctx, |image| {
                studio_core::filters::vignette(image, strength)
            });
        }

        let temperature_label = self.tr(TextKey::Temperature);
        ui.add(
            egui::Slider::new(&mut self.temperature_value, -100.0..=100.0).text(temperature_label),
        );
        if ui
            .add_enabled(
                has_image,
                egui::Button::new(self.tr(TextKey::ApplyTemperature)),
            )
            .clicked()
        {
            let value = self.temperature_value;
            self.map_scoped_working_image(ctx, |image| {
                studio_core::filters::adjust_temperature(image, value)
            });
        }

        let tint_label = self.tr(TextKey::Tint);
        ui.add(egui::Slider::new(&mut self.tint_value, -100.0..=100.0).text(tint_label));
        if ui
            .add_enabled(has_image, egui::Button::new(self.tr(TextKey::ApplyTint)))
            .clicked()
        {
            let value = self.tint_value;
            self.map_scoped_working_image(ctx, |image| {
                studio_core::filters::adjust_tint(image, value)
            });
        }

        let posterize_label = self.tr(TextKey::Posterize);
        ui.add(egui::Slider::new(&mut self.posterize_levels, 2..=32).text(posterize_label));
        if ui
            .add_enabled(
                has_image,
                egui::Button::new(self.tr(TextKey::ApplyPosterize)),
            )
            .clicked()
        {
            let levels = self.posterize_levels;
            self.map_scoped_working_image(ctx, |image| {
                studio_core::filters::posterize(image, levels)
            });
        }

        if ui
            .add_enabled(
                self.original_image.is_some(),
                egui::Button::new(self.tr(TextKey::ResetEdits)),
            )
            .clicked()
        {
            self.reset_edits(ctx);
        }
    }

    fn panorama_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("360");
        ui.separator();

        egui::ComboBox::from_label("Mode")
            .selected_text(self.panorama_mode.to_string())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.panorama_mode, PanoramaMode::Pad, "pad");
                ui.selectable_value(&mut self.panorama_mode, PanoramaMode::Stretch, "stretch");
                ui.selectable_value(&mut self.panorama_mode, PanoramaMode::Crop, "crop");
            });

        ui.add(
            egui::DragValue::new(&mut self.panorama_width)
                .range(2..=32768)
                .prefix("Width "),
        );

        if ui
            .add_enabled(
                self.working_image.is_some(),
                egui::Button::new(self.tr(TextKey::Export360)),
            )
            .clicked()
        {
            self.export_panorama();
        }
    }

    fn gif_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading(self.tr(TextKey::GifMaker));
        ui.separator();

        if ui.button(self.tr(TextKey::InputFolder)).clicked()
            && let Some(path) = rfd::FileDialog::new().pick_folder()
        {
            self.open_folder(ctx, path);
        }

        ui.label(format!(
            "{}: {} {}",
            self.tr(TextKey::GifSource),
            self.folder_files.len(),
            self.tr(TextKey::Images)
        ));

        let delay_label = self.tr(TextKey::GifFrameDelay);
        ui.add(
            egui::DragValue::new(&mut self.gif_delay_ms)
                .range(10..=10_000)
                .suffix(" ms")
                .prefix(format!("{delay_label} ")),
        );
        let repeat_label = self.tr(TextKey::GifRepeat);
        let resize_label = self.tr(TextKey::GifResize);
        ui.checkbox(&mut self.gif_repeat, repeat_label);
        ui.checkbox(&mut self.gif_resize_enabled, resize_label);

        if self.gif_resize_enabled {
            let max_w_label = self.tr(TextKey::MaxW);
            let max_h_label = self.tr(TextKey::MaxH);
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut self.gif_max_width)
                        .range(1..=65535)
                        .prefix(max_w_label),
                );
                ui.add(
                    egui::DragValue::new(&mut self.gif_max_height)
                        .range(1..=65535)
                        .prefix(max_h_label),
                );
            });
        }

        if ui
            .add_enabled(
                !self.folder_files.is_empty(),
                egui::Button::new(self.tr(TextKey::RunGif)),
            )
            .clicked()
        {
            self.export_gif();
        }
    }

    fn compose_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading(self.tr(TextKey::Compose));
        ui.separator();

        let has_image = self.working_image.is_some();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(has_image, egui::Button::new(self.tr(TextKey::AddText)))
                .clicked()
            {
                self.add_text_layer(ctx);
            }
            if ui
                .add_enabled(has_image, egui::Button::new(self.tr(TextKey::AddImage)))
                .clicked()
            {
                self.add_image_layer_from_file(ctx);
            }
            if ui
                .add_enabled(has_image, egui::Button::new(self.tr(TextKey::PasteImage)))
                .clicked()
            {
                self.paste_image_layer(ctx);
            }
        });

        if ui
            .add_enabled(
                has_image,
                egui::Button::new(self.tr(TextKey::ExportComposite)),
            )
            .clicked()
        {
            self.export_composite();
        }

        ui.separator();
        ui.label(self.tr(TextKey::WatermarkText));
        ui.text_edit_singleline(&mut self.watermark_text);
        let watermark_opacity_label = self.tr(TextKey::WatermarkOpacity);
        ui.add(
            egui::Slider::new(&mut self.watermark_opacity, 0.05..=1.0)
                .text(watermark_opacity_label),
        );
        let watermark_font_size_label = self.tr(TextKey::WatermarkFontSize);
        ui.add(
            egui::Slider::new(&mut self.watermark_font_size, 12.0..=256.0)
                .text(watermark_font_size_label),
        );
        if ui
            .add_enabled(has_image, egui::Button::new(self.tr(TextKey::AddWatermark)))
            .clicked()
        {
            self.add_text_watermark(ctx);
        }

        ui.add_space(8.0);
        ui.label(self.tr(TextKey::Layers));
        let mut changed = false;
        egui::ScrollArea::vertical()
            .max_height(150.0)
            .show(ui, |ui| {
                for index in 0..self.compose_layers.len() {
                    let label = format!(
                        "{} {}",
                        if self.compose_layers[index].is_visible() {
                            "●"
                        } else {
                            "○"
                        },
                        self.compose_layers[index].name()
                    );
                    if ui
                        .selectable_label(self.selected_layer == Some(index), label)
                        .clicked()
                    {
                        self.selected_layer = Some(index);
                    }
                }
            });

        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(self.can_undo(), egui::Button::new(self.tr(TextKey::Undo)))
                .clicked()
            {
                self.undo(ctx);
            }
            if ui
                .add_enabled(self.can_redo(), egui::Button::new(self.tr(TextKey::Redo)))
                .clicked()
            {
                self.redo(ctx);
            }
            if ui
                .add_enabled(
                    self.can_move_layer_up(),
                    egui::Button::new(self.tr(TextKey::MoveUp)),
                )
                .clicked()
            {
                self.push_history();
                self.move_selected_layer(-1);
                changed = true;
            }
            if ui
                .add_enabled(
                    self.can_move_layer_down(),
                    egui::Button::new(self.tr(TextKey::MoveDown)),
                )
                .clicked()
            {
                self.push_history();
                self.move_selected_layer(1);
                changed = true;
            }
            if ui
                .add_enabled(
                    self.selected_layer.is_some(),
                    egui::Button::new(self.tr(TextKey::DuplicateLayer)),
                )
                .clicked()
            {
                self.push_history();
                self.duplicate_selected_layer();
                changed = true;
            }
            if ui
                .add_enabled(
                    self.selected_layer.is_some(),
                    egui::Button::new(self.tr(TextKey::DeleteLayer)),
                )
                .clicked()
            {
                self.push_history();
                self.delete_selected_layer();
                changed = true;
            }
        });

        ui.label(self.tr(TextKey::Align));
        ui.horizontal_wrapped(|ui| {
            for (target, label) in [
                (AlignTarget::Left, self.tr(TextKey::AlignLeft)),
                (AlignTarget::Center, self.tr(TextKey::AlignCenter)),
                (AlignTarget::Right, self.tr(TextKey::AlignRight)),
                (AlignTarget::Top, self.tr(TextKey::AlignTop)),
                (AlignTarget::Middle, self.tr(TextKey::AlignMiddle)),
                (AlignTarget::Bottom, self.tr(TextKey::AlignBottom)),
            ] {
                if ui
                    .add_enabled(self.can_align_selected_layer(), egui::Button::new(label))
                    .clicked()
                {
                    self.push_history();
                    self.align_selected_layer(target);
                    changed = true;
                }
            }
        });

        ui.separator();
        let before_edit = self.compose_layers.clone();
        let edited = self.selected_layer_editor(ui);
        if edited {
            self.push_history_snapshot(before_edit);
            changed = true;
        }
        ui.label(self.tr(TextKey::KeyboardHint));

        if changed {
            self.update_texture(ctx);
        }
    }

    fn selected_layer_editor(&mut self, ui: &mut egui::Ui) -> bool {
        let name_label = self.tr(TextKey::LayerName);
        let visible_label = self.tr(TextKey::Visible);
        let position_label = self.tr(TextKey::Position);
        let size_label = self.tr(TextKey::Size);
        let opacity_label = self.tr(TextKey::Opacity);
        let text_label = self.tr(TextKey::TextContent);
        let font_label = self.tr(TextKey::Font);
        let font_size_label = self.tr(TextKey::FontSize);
        let color_label = self.tr(TextKey::TextColor);
        let stroke_label = self.tr(TextKey::Stroke);
        let shadow_label = self.tr(TextKey::Shadow);
        let selected_label = self.tr(TextKey::SelectedLayer);
        let no_layer_label = self.tr(TextKey::NoLayer);
        let font_choices = self.font_choices.clone();

        let Some(index) = self.selected_layer else {
            ui.label(no_layer_label);
            return false;
        };

        let Some(layer) = self.compose_layers.get_mut(index) else {
            ui.label(no_layer_label);
            return false;
        };

        let mut changed = false;
        ui.label(selected_label);
        ui.horizontal(|ui| {
            ui.label(name_label);
            changed |= ui.text_edit_singleline(layer.name_mut()).changed();
        });

        let mut visible = layer.is_visible();
        if ui.checkbox(&mut visible, visible_label).changed() {
            layer.set_visible(visible);
            changed = true;
        }

        match layer {
            ComposeLayer::Text(layer) => {
                ui.label(text_label);
                changed |= ui.text_edit_multiline(&mut layer.text).changed();
                ui.label(position_label);
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(egui::DragValue::new(&mut layer.x).prefix("X "))
                        .changed();
                    changed |= ui
                        .add(egui::DragValue::new(&mut layer.y).prefix("Y "))
                        .changed();
                });
                changed |= ui
                    .add(egui::Slider::new(&mut layer.font_size, 8.0..=256.0).text(font_size_label))
                    .changed();
                let selected_font = layer
                    .font_path
                    .as_ref()
                    .and_then(|path| {
                        font_choices
                            .iter()
                            .find(|choice| &choice.path == path)
                            .map(|choice| choice.name.as_str())
                    })
                    .unwrap_or("系統預設 / Default");
                egui::ComboBox::from_label(font_label)
                    .selected_text(selected_font)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                layer.font_path.is_none(),
                                egui::RichText::new("Aa 字型 - 系統預設 / Default").size(18.0),
                            )
                            .clicked()
                        {
                            layer.font_path = None;
                            changed = true;
                        }

                        for choice in &font_choices {
                            let selected = layer.font_path.as_ref() == Some(&choice.path);
                            let preview = egui::RichText::new(format!("Aa 字型 - {}", choice.name))
                                .font(egui::FontId::new(
                                    18.0,
                                    egui::FontFamily::Name(choice.preview_family.clone().into()),
                                ));
                            if ui.selectable_label(selected, preview).clicked() {
                                layer.font_path = Some(choice.path.clone());
                                changed = true;
                            }
                        }
                    });
                changed |= ui
                    .add(egui::Slider::new(&mut layer.opacity, 0.0..=1.0).text(opacity_label))
                    .changed();
                ui.label(color_label);
                changed |= ui
                    .color_edit_button_srgba_unmultiplied(&mut layer.color)
                    .changed();
                changed |= ui.checkbox(&mut layer.stroke, stroke_label).changed();
                changed |= ui.checkbox(&mut layer.shadow, shadow_label).changed();
            }
            ComposeLayer::Image(layer) => {
                ui.label(position_label);
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(egui::DragValue::new(&mut layer.x).prefix("X "))
                        .changed();
                    changed |= ui
                        .add(egui::DragValue::new(&mut layer.y).prefix("Y "))
                        .changed();
                });
                ui.label(size_label);
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut layer.width)
                                .range(1..=65535)
                                .prefix("W "),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut layer.height)
                                .range(1..=65535)
                                .prefix("H "),
                        )
                        .changed();
                });
                changed |= ui
                    .add(egui::Slider::new(&mut layer.opacity, 0.0..=1.0).text(opacity_label))
                    .changed();
            }
            ComposeLayer::Shape(layer) => {
                ui.label(position_label);
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(egui::DragValue::new(&mut layer.x1).prefix("X1 "))
                        .changed();
                    changed |= ui
                        .add(egui::DragValue::new(&mut layer.y1).prefix("Y1 "))
                        .changed();
                });
                ui.horizontal(|ui| {
                    changed |= ui
                        .add(egui::DragValue::new(&mut layer.x2).prefix("X2 "))
                        .changed();
                    changed |= ui
                        .add(egui::DragValue::new(&mut layer.y2).prefix("Y2 "))
                        .changed();
                });
                changed |= ui
                    .add(egui::Slider::new(&mut layer.stroke_width, 1.0..=32.0).text("Width"))
                    .changed();
                changed |= ui
                    .add(egui::Slider::new(&mut layer.opacity, 0.0..=1.0).text(opacity_label))
                    .changed();
                ui.label("Stroke color");
                changed |= ui
                    .color_edit_button_srgba_unmultiplied(&mut layer.stroke_color)
                    .changed();
                changed |= ui.checkbox(&mut layer.filled, "Filled").changed();
                if layer.filled {
                    ui.label("Fill color");
                    changed |= ui
                        .color_edit_button_srgba_unmultiplied(&mut layer.fill_color)
                        .changed();
                }
            }
        }

        changed
    }

    fn batch_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.tr(TextKey::Batch));
        ui.separator();

        if ui.button(self.tr(TextKey::InputFolder)).clicked()
            && let Some(path) = rfd::FileDialog::new().pick_folder()
        {
            self.batch_input_dir = Some(path);
        }
        if let Some(path) = &self.batch_input_dir {
            ui.label(format!(
                "{}: {}",
                self.tr(TextKey::InputFolder),
                path.display()
            ));
        }

        if ui.button(self.tr(TextKey::OutputFolder)).clicked()
            && let Some(path) = rfd::FileDialog::new().pick_folder()
        {
            self.batch_output_dir = Some(path);
        }
        if let Some(path) = &self.batch_output_dir {
            ui.label(format!(
                "{}: {}",
                self.tr(TextKey::OutputFolder),
                path.display()
            ));
        }

        ui.checkbox(
            &mut self.batch_resize_enabled,
            self.language.t(TextKey::ResizeToFit),
        );
        let max_w_label = self.tr(TextKey::MaxW);
        let max_h_label = self.tr(TextKey::MaxH);
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut self.batch_max_width)
                    .range(1..=65535)
                    .prefix(max_w_label),
            );
            ui.add(
                egui::DragValue::new(&mut self.batch_max_height)
                    .range(1..=65535)
                    .prefix(max_h_label),
            );
        });

        ui.separator();
        ui.checkbox(
            &mut self.batch_watermark_enabled,
            self.language.t(TextKey::BatchWatermarkEnable),
        );
        if self.batch_watermark_enabled {
            let opacity_label = self.tr(TextKey::WatermarkOpacity);
            let size_label = self.tr(TextKey::WatermarkFontSize);
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut self.batch_watermark_opacity)
                        .range(0.05..=1.0)
                        .speed(0.05)
                        .prefix(opacity_label),
                );
                ui.add(
                    egui::DragValue::new(&mut self.batch_watermark_font_size)
                        .range(8.0..=256.0)
                        .speed(1.0)
                        .prefix(size_label),
                );
            });
        }

        let can_run = self.batch_input_dir.is_some()
            && self.batch_output_dir.is_some()
            && self.batch_receiver.is_none();
        if ui
            .add_enabled(can_run, egui::Button::new(self.tr(TextKey::RunBatch)))
            .clicked()
        {
            self.start_batch();
        }

        if self.batch_receiver.is_some() {
            let progress = if self.batch_total == 0 {
                0.0
            } else {
                self.batch_completed as f32 / self.batch_total as f32
            };
            ui.add(
                egui::ProgressBar::new(progress)
                    .animate(true)
                    .text(format!("{} / {}", self.batch_completed, self.batch_total)),
            );
            ui.label(self.tr(TextKey::BatchRunning));
        }

        if let Some(summary) = &self.batch_summary {
            ui.label(format!(
                "{}: {} total, {} succeeded, {} failed",
                self.tr(TextKey::LastBatch),
                summary.total,
                summary.succeeded,
                summary.failed
            ));
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .show(ui, |ui| {
                    for result in summary.results.iter().take(12) {
                        let label = match (&result.output, &result.error) {
                            (Some(output), None) => format!("OK {}", output.display()),
                            (_, Some(error)) => {
                                format!("Failed {}: {error}", result.input.display())
                            }
                            _ => format!("Skipped {}", result.input.display()),
                        };
                        ui.label(label);
                    }
                });
        }
    }

    fn preview_panel(&mut self, ui: &mut egui::Ui) {
        if let Some(texture) = &self.texture {
            let available = ui.available_size().max(Vec2::splat(1.0));
            let scale = if self.fit_to_window {
                (available.x / self.texture_size.x)
                    .min(available.y / self.texture_size.y)
                    .clamp(0.01, 1.0)
            } else {
                self.zoom
            };
            let display_size = self.texture_size * scale;
            let texture_id = texture.id();
            let (viewport_rect, viewport_response) =
                ui.allocate_exact_size(available, Sense::hover());
            let image_center = viewport_rect.center() + self.preview_pan;
            let image_rect = Rect::from_center_size(image_center, display_size);
            let response = if self.preview_tool == PreviewTool::Hand {
                let pan_response =
                    ui.interact(viewport_rect, ui.id().with("canvas_pan"), Sense::drag());
                if pan_response.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                } else if pan_response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }
                pan_response
            } else {
                ui.interact(
                    image_rect,
                    ui.id().with("canvas_image"),
                    Sense::click_and_drag(),
                )
            };
            ui.painter().image(
                texture_id,
                image_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            self.handle_middle_pan(ui.ctx(), viewport_rect, scale);
            self.handle_preview_zoom(ui.ctx(), &viewport_response, viewport_rect, scale);
            self.handle_canvas_interaction(ui.ctx(), &response, image_rect, scale);
            self.paint_selection(ui, image_rect, scale);
            self.paint_selected_layer(ui, image_rect, scale);
        } else {
            ui.vertical_centered(|ui| {
                ui.add_space(120.0);
                ui.heading("Pixel Studio");
                ui.label(self.tr(TextKey::QuickHint));
                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.button(self.tr(TextKey::CaptureScreenshot)).clicked() {
                        self.capture_screenshot(ui.ctx());
                    }
                    if ui.button(self.tr(TextKey::PasteImage)).clicked() {
                        self.paste_clipboard_image(ui.ctx());
                    }
                    if ui.button(self.tr(TextKey::OpenImage)).clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_file()
                    {
                        self.open_path(ui.ctx(), path);
                    }
                });
                ui.add_space(8.0);
                ui.label(self.tr(TextKey::BrowseHint));
            });
        }
    }

    fn handle_middle_pan(&mut self, ctx: &egui::Context, viewport_rect: Rect, current_scale: f32) {
        let (middle_down, pointer_pos, pointer_delta) = ctx.input(|input| {
            (
                input.pointer.button_down(egui::PointerButton::Middle),
                input.pointer.interact_pos(),
                input.pointer.delta(),
            )
        });
        let Some(pointer_pos) = pointer_pos else {
            return;
        };
        if !middle_down || !viewport_rect.contains(pointer_pos) {
            return;
        }

        ctx.set_cursor_icon(if pointer_delta == Vec2::ZERO {
            egui::CursorIcon::Grab
        } else {
            egui::CursorIcon::Grabbing
        });
        if pointer_delta != Vec2::ZERO {
            if self.fit_to_window {
                self.zoom = current_scale;
            }
            self.fit_to_window = false;
            self.preview_pan += pointer_delta;
            ctx.request_repaint();
        }
    }

    fn handle_preview_zoom(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        viewport_rect: Rect,
        current_scale: f32,
    ) {
        if !response.hovered() {
            return;
        }

        let scroll_y = ctx.input(|input| input.smooth_scroll_delta().y);
        if scroll_y.abs() <= f32::EPSILON {
            return;
        }

        let factor = (scroll_y / 240.0).exp().clamp(0.2, 5.0);
        let old_zoom = if self.fit_to_window {
            current_scale
        } else {
            self.zoom
        };
        let old_image_center = viewport_rect.center() + self.preview_pan;
        if self.fit_to_window {
            self.zoom = current_scale;
            self.fit_to_window = false;
        }
        self.zoom = (self.zoom * factor).clamp(0.05, 8.0);
        if let Some(pointer_pos) = response.hover_pos() {
            let image_offset = pointer_pos - old_image_center;
            let effective_factor = self.zoom / old_zoom;
            self.preview_pan += image_offset * (1.0 - effective_factor);
        }
        ctx.request_repaint();
    }

    fn handle_canvas_interaction(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        image_rect: Rect,
        scale: f32,
    ) {
        if ctx.input(|input| {
            input.pointer.button_down(egui::PointerButton::Middle)
                || input.pointer.button_released(egui::PointerButton::Middle)
        }) {
            return;
        }

        if self.preview_tool == PreviewTool::Hand {
            if response.dragged() {
                if self.fit_to_window {
                    self.zoom = scale;
                }
                self.fit_to_window = false;
                self.preview_pan += response.drag_delta();
                ctx.request_repaint();
            }
            return;
        }

        if self.preview_tool == PreviewTool::Text {
            if response.clicked()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let image_pos = screen_to_image(pointer, image_rect, scale);
                self.add_text_layer_at(ctx, image_pos);
                self.preview_tool = PreviewTool::Select;
            }
            return;
        }

        if let Some(shape_kind) = self.shape_tool_kind() {
            if response.drag_started()
                && let Some(pointer) = response.interact_pointer_pos()
            {
                let image_pos = screen_to_image(pointer, image_rect, scale);
                self.selected_layer = None;
                self.selection_drag_start = Some(image_pos);
                self.selection_rect = Some(selection_rect_from_points(
                    image_pos,
                    image_pos,
                    SelectionShape::Rectangle,
                    self.texture_size,
                ));
            }
            if response.dragged()
                && let (Some(start), Some(pointer)) =
                    (self.selection_drag_start, response.interact_pointer_pos())
            {
                let image_pos = screen_to_image(pointer, image_rect, scale);
                self.selection_rect = Some(selection_rect_from_points(
                    start,
                    image_pos,
                    SelectionShape::Rectangle,
                    self.texture_size,
                ));
            }
            if response.drag_stopped()
                && let (Some(start), Some(pointer)) = (
                    self.selection_drag_start.take(),
                    response.interact_pointer_pos(),
                )
            {
                let end = screen_to_image(pointer, image_rect, scale);
                if (end - start).length_sq() > 1.0 {
                    self.add_shape_layer(ctx, shape_kind, start, end);
                }
                self.selection_rect = None;
                self.preview_tool = PreviewTool::Select;
            }
            return;
        }

        let crop_mode = self.preview_tool == PreviewTool::Crop;

        if response.drag_started()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let image_pos = screen_to_image(pointer, image_rect, scale);
            self.selected_layer = if crop_mode {
                None
            } else {
                self.hit_test_layer(image_pos)
            };
            if self.selected_layer.is_some() {
                self.drag_snapshot = Some(self.compose_layers.clone());
                self.selection_drag_start = None;
            } else {
                self.selection_drag_start = Some(image_pos);
                self.selection_rect = Some(selection_rect_from_points(
                    image_pos,
                    image_pos,
                    self.selection_shape,
                    self.texture_size,
                ));
                self.drag_snapshot = None;
            }
        }

        if response.dragged()
            && !crop_mode
            && let Some(index) = self.selected_layer
        {
            let frame_delta = response.drag_delta() / scale.max(0.01);
            if frame_delta.length_sq() > 0.0 {
                self.move_layer_by(index, frame_delta);
                self.update_texture(ctx);
            }
        }

        if response.dragged()
            && (crop_mode || self.selected_layer.is_none())
            && let (Some(start), Some(pointer)) =
                (self.selection_drag_start, response.interact_pointer_pos())
        {
            let image_pos = screen_to_image(pointer, image_rect, scale);
            self.selection_rect = Some(selection_rect_from_points(
                start,
                image_pos,
                self.selection_shape,
                self.texture_size,
            ));
        }

        if response.drag_stopped()
            && let Some(snapshot) = self.drag_snapshot.take()
        {
            self.push_history_snapshot(snapshot);
        }
        if response.drag_stopped() {
            self.selection_drag_start = None;
        }

        if response.clicked()
            && !crop_mode
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let image_pos = screen_to_image(pointer, image_rect, scale);
            self.selected_layer = self.hit_test_layer(image_pos);
        }
    }

    fn paint_selected_layer(&self, ui: &mut egui::Ui, image_rect: Rect, scale: f32) {
        let Some(index) = self.selected_layer else {
            return;
        };
        let Some(layer) = self.compose_layers.get(index) else {
            return;
        };
        let bounds = self.layer_bounds(layer);
        let min = image_rect.min + bounds.min.to_vec2() * scale;
        let max = image_rect.min + bounds.max.to_vec2() * scale;
        ui.painter().rect_stroke(
            Rect::from_min_max(min, max),
            0.0,
            Stroke::new(2.0, Color32::from_rgb(80, 180, 255)),
            StrokeKind::Outside,
        );
    }

    fn paint_selection(&self, ui: &mut egui::Ui, image_rect: Rect, scale: f32) {
        let Some(bounds) = self.selection_rect else {
            return;
        };
        let min = image_rect.min + bounds.min.to_vec2() * scale;
        let max = image_rect.min + bounds.max.to_vec2() * scale;
        let rect = Rect::from_min_max(min, max);
        let stroke = Stroke::new(2.0, Color32::from_rgb(255, 210, 70));

        match self.selection_shape {
            SelectionShape::Rectangle => {
                ui.painter()
                    .rect_stroke(rect, 0.0, stroke, StrokeKind::Outside);
            }
            SelectionShape::Circle => {
                ui.painter().circle_stroke(
                    rect.center(),
                    rect.width().min(rect.height()) / 2.0,
                    stroke,
                );
            }
        }
    }

    fn open_pending_path(&mut self, ctx: &egui::Context) {
        if let Some(path) = self.pending_open.take() {
            self.open_path(ctx, path);
        }
    }

    fn open_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|input| input.raw.dropped_files.clone());
        let paths: Vec<PathBuf> = dropped.into_iter().filter_map(|file| file.path).collect();
        let Some(path) = paths
            .iter()
            .find(|path| is_supported_drop_path(path))
            .cloned()
            .or_else(|| paths.first().cloned())
        else {
            return;
        };

        self.open_path(ctx, path);
    }

    fn open_path(&mut self, ctx: &egui::Context, path: PathBuf) {
        if path.is_dir() {
            self.open_folder(ctx, path);
        } else if SupportedFormat::from_path(&path).is_some() {
            self.open_image(ctx, path);
        } else {
            self.status = match self.language {
                Language::ZhTw => format!("不支援拖曳的檔案格式：{}", path.display()),
                Language::En => format!("Unsupported dropped file format: {}", path.display()),
            };
        }
    }

    fn open_folder(&mut self, ctx: &egui::Context, path: PathBuf) {
        self.folder_files = collect_supported_files(&path);
        self.folder_files.sort();
        self.selected_index = None;

        if self.folder_files.is_empty() {
            self.status = match self.language {
                Language::ZhTw => format!("資料夾沒有可支援的圖片：{}", path.display()),
                Language::En => format!("No supported images found in {}", path.display()),
            };
            return;
        }

        self.open_folder_index(ctx, 0);
        self.status = match self.language {
            Language::ZhTw => format!(
                "已載入資料夾：{}，共 {} 張圖片",
                path.display(),
                self.folder_files.len()
            ),
            Language::En => format!(
                "Loaded folder {} with {} images",
                path.display(),
                self.folder_files.len()
            ),
        };
    }

    fn open_folder_index(&mut self, ctx: &egui::Context, index: usize) {
        if let Some(path) = self.folder_files.get(index).cloned() {
            self.selected_index = Some(index);
            self.open_image(ctx, path);
        }
    }

    fn open_image(&mut self, ctx: &egui::Context, path: PathBuf) {
        match load_dynamic_image(&path) {
            Ok(image) => {
                self.original_image = Some(image.clone());
                self.working_image = Some(image);
                self.compose_layers.clear();
                self.selected_layer = None;
                self.undo_stack.clear();
                self.redo_stack.clear();
                self.image_undo_stack.clear();
                self.image_redo_stack.clear();
                self.drag_snapshot = None;
                self.selection_rect = None;
                self.selection_drag_start = None;
                self.preview_pan = Vec2::ZERO;
                self.fit_to_window = true;
                self.image_info = inspect_image(&path).ok();
                self.load_c2pa_state(&path);
                self.image_path = Some(path.clone());
                self.sync_edit_dimensions();
                self.update_texture(ctx);
                self.status = match self.language {
                    Language::ZhTw => format!("已載入：{}", path.display()),
                    Language::En => format!("Loaded {}", path.display()),
                };
            }
            Err(error) => {
                self.status = match self.language {
                    Language::ZhTw => {
                        format!("無法開啟圖片：{}。錯誤：{error}", path.display())
                    }
                    Language::En => format!("Failed to open image {}: {error}", path.display()),
                };
            }
        }
    }

    fn update_texture(&mut self, ctx: &egui::Context) {
        if let Some(image) = self.render_current_image() {
            let rgba = image.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            self.texture = Some(ctx.load_texture("preview", color_image, TextureOptions::LINEAR));
            self.texture_size = Vec2::new(image.width() as f32, image.height() as f32);
        } else {
            self.texture = None;
            self.texture_size = Vec2::ZERO;
        }
    }

    fn clear_canvas(&mut self, ctx: &egui::Context) {
        if self.working_image.is_none() {
            return;
        }

        self.cleared_canvas = Some(self.capture_canvas_snapshot());
        self.image_path = None;
        self.image_info = None;
        self.original_image = None;
        self.working_image = None;
        self.compose_layers.clear();
        self.selected_layer = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.image_undo_stack.clear();
        self.image_redo_stack.clear();
        self.drag_snapshot = None;
        self.selection_rect = None;
        self.selection_drag_start = None;
        self.folder_files.clear();
        self.selected_index = None;
        self.reset_c2pa_state(None);
        self.update_texture(ctx);
        self.status = match self.language {
            Language::ZhTw => "已清除畫面，可按「恢復畫面」取回。".to_owned(),
            Language::En => "Canvas cleared. Use Restore Canvas to bring it back.".to_owned(),
        };
    }

    fn restore_canvas(&mut self, ctx: &egui::Context) {
        let Some(snapshot) = self.cleared_canvas.take() else {
            return;
        };

        self.image_path = snapshot.image_path;
        self.image_info = snapshot.image_info;
        self.c2pa_info = snapshot.c2pa_info;
        self.c2pa_draft = snapshot.c2pa_draft;
        self.c2pa_write_on_save = snapshot.c2pa_write_on_save;
        self.original_image = snapshot.original_image;
        self.working_image = snapshot.working_image;
        self.compose_layers = snapshot.compose_layers;
        self.selected_layer = snapshot.selected_layer;
        self.image_undo_stack = snapshot.image_undo_stack;
        self.image_redo_stack = snapshot.image_redo_stack;
        self.selection_shape = snapshot.selection_shape;
        self.selection_rect = snapshot.selection_rect;
        self.selection_drag_start = None;
        self.folder_files = snapshot.folder_files;
        self.selected_index = snapshot.selected_index;
        self.drag_snapshot = None;
        self.sync_edit_dimensions();
        self.update_texture(ctx);
        self.status = match self.language {
            Language::ZhTw => "已恢復清除前畫面。".to_owned(),
            Language::En => "Canvas restored.".to_owned(),
        };
    }

    fn capture_canvas_snapshot(&self) -> CanvasSnapshot {
        CanvasSnapshot {
            image_path: self.image_path.clone(),
            image_info: self.image_info.clone(),
            c2pa_info: self.c2pa_info.clone(),
            c2pa_draft: self.c2pa_draft.clone(),
            c2pa_write_on_save: self.c2pa_write_on_save,
            original_image: self.original_image.clone(),
            working_image: self.working_image.clone(),
            compose_layers: self.compose_layers.clone(),
            selected_layer: self.selected_layer,
            image_undo_stack: self.image_undo_stack.clone(),
            image_redo_stack: self.image_redo_stack.clone(),
            selection_shape: self.selection_shape,
            selection_rect: self.selection_rect,
            folder_files: self.folder_files.clone(),
            selected_index: self.selected_index,
        }
    }

    fn load_c2pa_state(&mut self, path: &PathBuf) {
        match inspect_c2pa(path) {
            Ok(info) => {
                let draft = C2paManifestDraft {
                    title: info
                        .title
                        .clone()
                        .unwrap_or_else(|| default_c2pa_title(path)),
                    creator: info.creator.clone().unwrap_or_default(),
                    action: info.action.clone().unwrap_or_default(),
                };
                self.c2pa_write_on_save = info.present;
                self.c2pa_info = info;
                self.c2pa_draft = draft;
            }
            Err(_) => self.reset_c2pa_state(Some(path)),
        }
    }

    fn reset_c2pa_state(&mut self, path: Option<&PathBuf>) {
        self.c2pa_info = C2paInfo::default();
        self.c2pa_draft = C2paManifestDraft {
            title: path
                .map(|path| default_c2pa_title(path.as_path()))
                .unwrap_or_default(),
            creator: String::new(),
            action: String::new(),
        };
        self.c2pa_write_on_save = false;
    }

    fn save_image_with_optional_c2pa(
        &self,
        image: &DynamicImage,
        output: &PathBuf,
        format: SupportedFormat,
    ) -> studio_core::Result<ConvertResult> {
        let options = ConvertOptions {
            format,
            quality: self.quality,
            background: [255, 255, 255, 255],
        };

        if !self.c2pa_write_on_save {
            return save_dynamic_image(image, output, options);
        }

        self.save_image_with_c2pa(image, output, options)
    }

    fn save_image_with_c2pa(
        &self,
        image: &DynamicImage,
        output: &PathBuf,
        options: ConvertOptions,
    ) -> studio_core::Result<ConvertResult> {
        let temp_output = temporary_c2pa_source_path(output);
        let result = save_dynamic_image(image, &temp_output, options)?;
        if output.exists() {
            fs::remove_file(output)?;
        }

        let write_result = write_c2pa_manifest(&temp_output, output, &self.c2pa_draft);
        let _ = fs::remove_file(&temp_output);
        write_result?;

        Ok(ConvertResult {
            input: PathBuf::new(),
            output: output.clone(),
            format: result.format,
            width: result.width,
            height: result.height,
        })
    }

    fn save_c2pa_copy(&mut self) {
        let Some(image) = self.render_current_image() else {
            self.status = match self.language {
                Language::ZhTw => "尚未載入可寫入 C2PA 的圖片。".to_owned(),
                Language::En => "No image loaded for C2PA export.".to_owned(),
            };
            return;
        };

        let Some(mut output) = rfd::FileDialog::new()
            .add_filter("Images", &["jpg", "jpeg", "png", "webp", "tif", "tiff"])
            .set_file_name(self.default_c2pa_copy_name())
            .save_file()
        else {
            return;
        };

        if output.extension().is_none() {
            output.set_extension(self.output_format.extension());
        }
        let format = SupportedFormat::from_path(&output).unwrap_or(self.output_format);
        match self.save_image_with_c2pa(
            &image,
            &output,
            ConvertOptions {
                format,
                quality: self.quality,
                background: [255, 255, 255, 255],
            },
        ) {
            Ok(result) => {
                self.status = match self.language {
                    Language::ZhTw => format!(
                        "已添加 C2PA 並另存：{}（{}x{}）",
                        result.output.display(),
                        result.width,
                        result.height
                    ),
                    Language::En => format!(
                        "Added C2PA and saved {} ({}x{})",
                        result.output.display(),
                        result.width,
                        result.height
                    ),
                };
            }
            Err(error) => {
                self.status = match self.language {
                    Language::ZhTw => format!("添加 C2PA 失敗：{error}"),
                    Language::En => format!("Adding C2PA failed: {error}"),
                };
            }
        }
    }

    fn remove_c2pa_copy(&mut self) {
        let Some(source) = self.image_path.clone() else {
            self.status = match self.language {
                Language::ZhTw => "尚未開啟可移除 C2PA 的來源檔。".to_owned(),
                Language::En => "No source file loaded for C2PA removal.".to_owned(),
            };
            return;
        };

        let Some(mut output) = rfd::FileDialog::new()
            .add_filter("Images", &["jpg", "jpeg", "png", "webp", "tif", "tiff"])
            .set_file_name(self.default_c2pa_removed_name())
            .save_file()
        else {
            return;
        };

        // C2PA removal copies the source image bytes and strips only the
        // manifest, so the output extension must continue to match the source.
        if let Some(extension) = source.extension() {
            output.set_extension(extension);
        }

        if same_existing_path(&source, &output) {
            self.status = match self.language {
                Language::ZhTw => "移除 C2PA 請另存為不同檔案。".to_owned(),
                Language::En => "Choose a different file when removing C2PA.".to_owned(),
            };
            return;
        }
        if output.exists()
            && let Err(error) = fs::remove_file(&output)
        {
            self.status = match self.language {
                Language::ZhTw => format!("無法覆寫輸出檔：{error}"),
                Language::En => format!("Could not overwrite output file: {error}"),
            };
            return;
        }

        match remove_c2pa_manifest(&source, &output) {
            Ok(()) => {
                self.status = match self.language {
                    Language::ZhTw => format!("已移除 C2PA 並另存：{}", output.display()),
                    Language::En => format!("Removed C2PA and saved {}", output.display()),
                };
            }
            Err(error) => {
                self.status = match self.language {
                    Language::ZhTw => format!("移除 C2PA 失敗：{error}"),
                    Language::En => format!("Removing C2PA failed: {error}"),
                };
            }
        }
    }

    fn default_c2pa_copy_name(&self) -> String {
        self.image_path
            .as_ref()
            .map(|path| file_name_with_suffix(path, "_c2pa"))
            .unwrap_or_else(|| format!("c2pa.{}", self.output_format.extension()))
    }

    fn default_c2pa_removed_name(&self) -> String {
        self.image_path
            .as_ref()
            .map(|path| file_name_with_suffix(path, "_no_c2pa"))
            .unwrap_or_else(|| format!("no_c2pa.{}", self.output_format.extension()))
    }

    fn save_converted(&mut self) {
        let Some(image) = self.render_current_image() else {
            return;
        };

        let Some(output) = rfd::FileDialog::new()
            .set_file_name(format!("converted.{}", self.output_format.extension()))
            .save_file()
        else {
            return;
        };

        match self.save_image_with_optional_c2pa(&image, &output, self.output_format) {
            Ok(result) => {
                self.status = match self.language {
                    Language::ZhTw => format!(
                        "已轉檔到 {}（{}x{}）",
                        result.output.display(),
                        result.width,
                        result.height
                    ),
                    Language::En => format!(
                        "Converted to {} ({}x{})",
                        result.output.display(),
                        result.width,
                        result.height
                    ),
                };
            }
            Err(error) => {
                self.status = match self.language {
                    Language::ZhTw => format!("轉檔失敗：{error}"),
                    Language::En => format!("Conversion failed: {error}"),
                };
            }
        }
    }

    fn save_as_new_file(&mut self) {
        let Some(image) = self.render_current_image() else {
            return;
        };

        let default_name = self.default_save_as_name();
        let Some(mut output) = rfd::FileDialog::new()
            .add_filter(
                "Images",
                &["jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff"],
            )
            .set_file_name(default_name)
            .save_file()
        else {
            return;
        };

        if output.extension().is_none() {
            output.set_extension(self.output_format.extension());
        }
        let format = SupportedFormat::from_path(&output).unwrap_or(self.output_format);

        match self.save_image_with_optional_c2pa(&image, &output, format) {
            Ok(result) => {
                self.status = match self.language {
                    Language::ZhTw => format!(
                        "已另存新檔：{}（{}x{}）",
                        result.output.display(),
                        result.width,
                        result.height
                    ),
                    Language::En => format!(
                        "Saved as {} ({}x{})",
                        result.output.display(),
                        result.width,
                        result.height
                    ),
                };
            }
            Err(error) => {
                self.status = match self.language {
                    Language::ZhTw => format!("另存新檔失敗：{error}"),
                    Language::En => format!("Save as failed: {error}"),
                };
            }
        }
    }

    fn default_save_as_name(&self) -> String {
        let stem = self
            .image_path
            .as_ref()
            .and_then(|path| path.file_stem())
            .and_then(|value| value.to_str())
            .unwrap_or("photo");
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        format!(
            "{stem}_edited_{timestamp}.{}",
            self.output_format.extension()
        )
    }

    fn default_gif_name(&self) -> String {
        let stem = self
            .folder_files
            .first()
            .and_then(|path| path.parent())
            .and_then(|path| path.file_name())
            .and_then(|value| value.to_str())
            .unwrap_or("animation");
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        format!("{stem}_gif_{timestamp}.gif")
    }

    fn export_panorama(&mut self) {
        let Some(image) = self.render_current_image() else {
            return;
        };

        let Some(output) = rfd::FileDialog::new()
            .set_file_name("photo_360.jpg")
            .save_file()
        else {
            return;
        };

        let input = self.image_path.clone().unwrap_or_default();
        match write_panorama_dynamic_jpeg(
            &image,
            &output,
            PanoramaOptions {
                mode: self.panorama_mode,
                target_width: Some(self.panorama_width.max(2)),
                quality: self.quality,
                background: [0, 0, 0, 255],
            },
            input,
        ) {
            Ok(result) => {
                self.status = match self.language {
                    Language::ZhTw => format!(
                        "已匯出 360 JPEG 到 {}（{}x{}，模式={}）",
                        result.output.display(),
                        result.width,
                        result.height,
                        result.mode
                    ),
                    Language::En => format!(
                        "Exported 360 JPEG to {} ({}x{}, mode={})",
                        result.output.display(),
                        result.width,
                        result.height,
                        result.mode
                    ),
                };
            }
            Err(error) => {
                self.status = match self.language {
                    Language::ZhTw => format!("360 匯出失敗：{error}"),
                    Language::En => format!("360 export failed: {error}"),
                };
            }
        }
    }

    fn export_gif(&mut self) {
        if self.folder_files.is_empty() {
            self.status = match self.language {
                Language::ZhTw => "請先開啟含有圖片的資料夾。".to_owned(),
                Language::En => "Open a folder with images first.".to_owned(),
            };
            return;
        }

        let Some(mut output) = rfd::FileDialog::new()
            .add_filter("GIF", &["gif"])
            .set_file_name(self.default_gif_name())
            .save_file()
        else {
            return;
        };
        if output.extension().is_none() {
            output.set_extension("gif");
        }

        match write_animated_gif(
            &self.folder_files,
            &output,
            GifOptions {
                delay_ms: self.gif_delay_ms,
                repeat: self.gif_repeat,
                max_width: self.gif_resize_enabled.then_some(self.gif_max_width),
                max_height: self.gif_resize_enabled.then_some(self.gif_max_height),
                background: [255, 255, 255, 255],
            },
        ) {
            Ok(result) => {
                self.status = match self.language {
                    Language::ZhTw => format!(
                        "已製作 GIF：{}（{} 張，{}x{}，{} ms/張）",
                        result.output.display(),
                        result.frame_count,
                        result.width,
                        result.height,
                        result.delay_ms
                    ),
                    Language::En => format!(
                        "Created GIF {} ({} frames, {}x{}, {} ms/frame)",
                        result.output.display(),
                        result.frame_count,
                        result.width,
                        result.height,
                        result.delay_ms
                    ),
                };
            }
            Err(error) => {
                self.status = match self.language {
                    Language::ZhTw => format!("GIF 製作失敗：{error}"),
                    Language::En => format!("GIF export failed: {error}"),
                };
            }
        }
    }

    fn map_working_image<F>(&mut self, ctx: &egui::Context, mapper: F)
    where
        F: FnOnce(&DynamicImage) -> DynamicImage,
    {
        if let Some(image) = self.working_image.clone() {
            self.push_image_history();
            self.working_image = Some(mapper(&image));
            self.selection_rect = None;
            self.selection_drag_start = None;
            self.sync_edit_dimensions();
            self.update_texture(ctx);
            self.status = match self.language {
                Language::ZhTw => "已套用編輯。".to_owned(),
                Language::En => "Edit applied.".to_owned(),
            };
        }
    }

    fn map_scoped_working_image<F>(&mut self, ctx: &egui::Context, mapper: F)
    where
        F: FnOnce(&DynamicImage) -> DynamicImage,
    {
        if let Some(image) = self.working_image.clone() {
            let mask = self.current_selection_mask();
            self.push_image_history();
            let edited = mapper(&image);
            self.working_image = Some(
                mask.map(|mask| {
                    studio_core::transform::apply_selection_mask(&image, &edited, mask)
                })
                .unwrap_or(edited),
            );
            self.sync_edit_dimensions();
            self.update_texture(ctx);
            self.status = match self.language {
                Language::ZhTw => "已套用編輯。".to_owned(),
                Language::En => "Edit applied.".to_owned(),
            };
        }
    }

    fn apply_resize(&mut self, ctx: &egui::Context) {
        if self.working_image.is_some() {
            self.push_image_history();
            self.working_image = Some(studio_core::resize::resize_exact(
                self.working_image.as_ref().unwrap(),
                self.resize_width,
                self.resize_height,
            ));
            self.selection_rect = None;
            self.selection_drag_start = None;
            self.sync_edit_dimensions();
            self.update_texture(ctx);
            self.status = match self.language {
                Language::ZhTw => "已套用尺寸調整。".to_owned(),
                Language::En => "Resize applied.".to_owned(),
            };
        }
    }

    fn apply_crop(&mut self, ctx: &egui::Context) {
        let Some(image) = self.working_image.clone() else {
            return;
        };

        let x = self.crop_x.min(image.width().saturating_sub(1));
        let y = self.crop_y.min(image.height().saturating_sub(1));
        let width = self.crop_width.min(image.width().saturating_sub(x)).max(1);
        let height = self
            .crop_height
            .min(image.height().saturating_sub(y))
            .max(1);
        self.push_image_history();
        self.working_image = Some(image.crop_imm(x, y, width, height));
        self.selection_rect = None;
        self.selection_drag_start = None;
        self.sync_edit_dimensions();
        self.update_texture(ctx);
        self.status = match self.language {
            Language::ZhTw => "已套用裁切。".to_owned(),
            Language::En => "Crop applied.".to_owned(),
        };
    }

    fn apply_selection_crop(&mut self, ctx: &egui::Context) {
        let Some(selection) = self.selection_rect else {
            return;
        };
        self.crop_x = selection.min.x.floor().max(0.0) as u32;
        self.crop_y = selection.min.y.floor().max(0.0) as u32;
        self.crop_width = selection.width().ceil().max(1.0) as u32;
        self.crop_height = selection.height().ceil().max(1.0) as u32;
        self.apply_crop(ctx);
        self.preview_tool = PreviewTool::Select;
    }

    fn reset_edits(&mut self, ctx: &egui::Context) {
        if let Some(image) = self.original_image.clone() {
            self.push_image_history();
            self.working_image = Some(image);
            self.selection_rect = None;
            self.selection_drag_start = None;
            self.sync_edit_dimensions();
            self.update_texture(ctx);
            self.status = match self.language {
                Language::ZhTw => "已重設編輯。".to_owned(),
                Language::En => "Edits reset.".to_owned(),
            };
        }
    }

    fn sync_resize_aspect(&mut self, width_changed: bool, height_changed: bool) {
        let Some(image) = &self.working_image else {
            return;
        };
        let ratio = image.width() as f32 / image.height().max(1) as f32;

        if width_changed {
            self.resize_height = ((self.resize_width as f32 / ratio).round() as u32).max(1);
        } else if height_changed {
            self.resize_width = ((self.resize_height as f32 * ratio).round() as u32).max(1);
        }
    }

    fn sync_edit_dimensions(&mut self) {
        if let Some(image) = &self.working_image {
            self.resize_width = image.width().max(1);
            self.resize_height = image.height().max(1);
            self.crop_x = 0;
            self.crop_y = 0;
            self.crop_width = image.width().max(1);
            self.crop_height = image.height().max(1);
        }
    }

    fn start_batch(&mut self) {
        let Some(input_dir) = self.batch_input_dir.clone() else {
            return;
        };
        let Some(output_dir) = self.batch_output_dir.clone() else {
            return;
        };

        let options = BatchOptions {
            output_dir: output_dir.clone(),
            convert: ConvertOptions {
                format: self.output_format,
                quality: self.quality,
                background: [255, 255, 255, 255],
            },
            max_width: self.batch_resize_enabled.then_some(self.batch_max_width),
            max_height: self.batch_resize_enabled.then_some(self.batch_max_height),
            watermark_text: self
                .batch_watermark_enabled
                .then(|| self.watermark_text.clone())
                .filter(|text| !text.trim().is_empty()),
            watermark_opacity: self.batch_watermark_opacity,
            watermark_font_size: self.batch_watermark_font_size,
        };

        let (sender, receiver) = mpsc::channel();
        self.batch_receiver = Some(receiver);
        self.batch_summary = None;
        self.active_batch_kind = Some(BatchKind::Convert);
        self.batch_completed = 0;
        self.batch_total = 0;
        self.status = match self.language {
            Language::ZhTw => "批次轉檔已開始。".to_owned(),
            Language::En => "Batch conversion started.".to_owned(),
        };

        thread::spawn(move || {
            let _ = std::fs::create_dir_all(&output_dir);
            let files = collect_supported_files(&input_dir);
            let total = files.len();
            let _ = sender.send(BatchMessage::Progress {
                completed: 0,
                total,
            });
            let progress_sender = sender.clone();
            let summary = process_batch_with_progress(&files, &options, move |completed, total| {
                let _ = progress_sender.send(BatchMessage::Progress { completed, total });
            });
            let _ = sender.send(BatchMessage::Done(summary));
        });
    }

    fn start_c2pa_batch(&mut self, mode: C2paBatchMode) {
        let Some(input_dir) = self.c2pa_batch_input_dir.clone() else {
            return;
        };
        let Some(output_dir) = self.c2pa_batch_output_dir.clone() else {
            return;
        };

        let removal_options = C2paRemovalOptions {
            output_dir: output_dir.clone(),
        };
        let add_options = C2paAddOptions {
            output_dir: output_dir.clone(),
            draft: self.c2pa_draft.clone(),
        };

        let (sender, receiver) = mpsc::channel();
        self.batch_receiver = Some(receiver);
        self.batch_summary = None;
        self.c2pa_batch_result = None;
        self.active_batch_kind = Some(BatchKind::C2pa(mode));
        self.batch_completed = 0;
        self.batch_total = 0;
        self.status = match self.language {
            Language::ZhTw => match mode {
                C2paBatchMode::Add => "C2PA 批次加入已開始。".to_owned(),
                C2paBatchMode::Remove => "C2PA 批次移除已開始。".to_owned(),
            },
            Language::En => match mode {
                C2paBatchMode::Add => "C2PA batch add started.".to_owned(),
                C2paBatchMode::Remove => "C2PA batch removal started.".to_owned(),
            },
        };

        thread::spawn(move || {
            let _ = std::fs::create_dir_all(&output_dir);
            let files = collect_supported_files(&input_dir);
            let total = files.len();
            let _ = sender.send(BatchMessage::Progress {
                completed: 0,
                total,
            });
            let progress_sender = sender.clone();
            let progress = move |completed, total| {
                let _ = progress_sender.send(BatchMessage::Progress { completed, total });
            };
            let summary = match mode {
                C2paBatchMode::Add => {
                    process_c2pa_add_batch_with_progress(&files, &add_options, progress)
                }
                C2paBatchMode::Remove => {
                    process_c2pa_removal_batch_with_progress(&files, &removal_options, progress)
                }
            };
            let _ = sender.send(BatchMessage::C2paDone { summary, mode });
        });
    }

    fn poll_batch(&mut self, ctx: &egui::Context) {
        let mut clear_receiver = false;

        if let Some(receiver) = &self.batch_receiver {
            loop {
                match receiver.try_recv() {
                    Ok(BatchMessage::Progress { completed, total }) => {
                        self.batch_completed = completed;
                        self.batch_total = total;
                    }
                    Ok(BatchMessage::Done(summary)) => {
                        self.batch_completed = summary.total;
                        self.batch_total = summary.total;
                        self.status = match self.language {
                            Language::ZhTw => format!(
                                "批次完成：共 {}，成功 {}，失敗 {}",
                                summary.total, summary.succeeded, summary.failed
                            ),
                            Language::En => format!(
                                "Batch complete: {} total, {} succeeded, {} failed",
                                summary.total, summary.succeeded, summary.failed
                            ),
                        };
                        self.batch_summary = Some(summary);
                        self.active_batch_kind = None;
                        clear_receiver = true;
                        break;
                    }
                    Ok(BatchMessage::C2paDone { summary, mode }) => {
                        self.batch_completed = summary.total;
                        self.batch_total = summary.total;
                        self.status = match self.language {
                            Language::ZhTw => match mode {
                                C2paBatchMode::Add => format!(
                                    "C2PA 批次加入完成：共 {}，成功 {}，失敗 {}",
                                    summary.total, summary.succeeded, summary.failed
                                ),
                                C2paBatchMode::Remove => format!(
                                    "C2PA 批次移除完成：共 {}，成功 {}，失敗 {}",
                                    summary.total, summary.succeeded, summary.failed
                                ),
                            },
                            Language::En => match mode {
                                C2paBatchMode::Add => format!(
                                    "C2PA batch add complete: {} total, {} succeeded, {} failed",
                                    summary.total, summary.succeeded, summary.failed
                                ),
                                C2paBatchMode::Remove => format!(
                                    "C2PA batch removal complete: {} total, {} succeeded, {} failed",
                                    summary.total, summary.succeeded, summary.failed
                                ),
                            },
                        };
                        self.c2pa_batch_result = Some((mode, summary.clone()));
                        self.batch_summary = Some(summary);
                        self.active_batch_kind = None;
                        clear_receiver = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        ctx.request_repaint();
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.status = match self.language {
                            Language::ZhTw => "批次背景工作非預期停止。".to_owned(),
                            Language::En => "Batch worker stopped unexpectedly.".to_owned(),
                        };
                        self.active_batch_kind = None;
                        clear_receiver = true;
                        break;
                    }
                }
            }
        }

        if clear_receiver {
            self.batch_receiver = None;
        }
    }

    fn render_current_image(&self) -> Option<DynamicImage> {
        let background = self.working_image.as_ref()?.clone();
        if self.compose_layers.is_empty() {
            return Some(background);
        }

        Some(render_composition(
            &ComposeDocument {
                background,
                layers: self.compose_layers.clone(),
            },
            self.font_bytes.as_deref(),
        ))
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }

        let (
            undo,
            redo,
            paste,
            delete,
            duplicate,
            save_copy,
            save_project_shortcut,
            apply_crop,
            cancel_crop,
            left,
            right,
            up,
            down,
        ) = ctx.input(|input| {
            (
                input.modifiers.ctrl && input.key_pressed(egui::Key::Z),
                input.modifiers.ctrl && input.key_pressed(egui::Key::Y),
                input.modifiers.command && input.key_pressed(egui::Key::V),
                input.key_pressed(egui::Key::Delete),
                input.modifiers.ctrl && input.key_pressed(egui::Key::D),
                input.modifiers.ctrl && input.modifiers.shift && input.key_pressed(egui::Key::S),
                input.modifiers.ctrl && !input.modifiers.shift && input.key_pressed(egui::Key::S),
                input.key_pressed(egui::Key::Enter),
                input.key_pressed(egui::Key::Escape),
                input.key_pressed(egui::Key::ArrowLeft),
                input.key_pressed(egui::Key::ArrowRight),
                input.key_pressed(egui::Key::ArrowUp),
                input.key_pressed(egui::Key::ArrowDown),
            )
        });

        if save_copy {
            self.save_as_new_file();
            return;
        }
        if save_project_shortcut && self.working_image.is_some() {
            self.save_project();
            return;
        }
        if self.preview_tool == PreviewTool::Crop && apply_crop && self.selection_rect.is_some() {
            self.apply_selection_crop(ctx);
            return;
        }
        if self.preview_tool == PreviewTool::Crop && cancel_crop {
            self.selection_rect = None;
            self.selection_drag_start = None;
            self.preview_tool = PreviewTool::Select;
            return;
        }

        if undo {
            if self.can_undo() {
                self.undo(ctx);
            } else {
                self.undo_image_edit(ctx);
            }
            return;
        }
        if redo {
            if self.can_redo() {
                self.redo(ctx);
            } else {
                self.redo_image_edit(ctx);
            }
            return;
        }
        if paste {
            self.paste_clipboard_image(ctx);
            return;
        }
        if delete && self.selected_layer.is_some() {
            self.push_history();
            self.delete_selected_layer();
            self.update_texture(ctx);
            return;
        }
        if duplicate && self.selected_layer.is_some() {
            self.push_history();
            self.duplicate_selected_layer();
            self.update_texture(ctx);
            return;
        }

        let mut delta = Vec2::ZERO;
        if left {
            delta.x -= 1.0;
        }
        if right {
            delta.x += 1.0;
        }
        if up {
            delta.y -= 1.0;
        }
        if down {
            delta.y += 1.0;
        }

        if delta != Vec2::ZERO
            && let Some(index) = self.selected_layer
        {
            self.push_history();
            self.move_layer_by(index, delta);
            self.update_texture(ctx);
        }
    }

    fn push_history(&mut self) {
        self.push_history_snapshot(self.compose_layers.clone());
    }

    fn push_history_snapshot(&mut self, snapshot: Vec<ComposeLayer>) {
        self.undo_stack.push(snapshot);
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    fn push_image_history(&mut self) {
        if let Some(image) = &self.working_image {
            self.image_undo_stack.push(image.clone());
            if self.image_undo_stack.len() > 50 {
                self.image_undo_stack.remove(0);
            }
            self.image_redo_stack.clear();
        }
    }

    fn can_undo_image(&self) -> bool {
        !self.image_undo_stack.is_empty()
    }

    fn can_redo_image(&self) -> bool {
        !self.image_redo_stack.is_empty()
    }

    fn undo_image_edit(&mut self, ctx: &egui::Context) {
        let Some(previous) = self.image_undo_stack.pop() else {
            return;
        };
        if let Some(current) = self.working_image.replace(previous) {
            self.image_redo_stack.push(current);
        }
        self.selection_rect = None;
        self.selection_drag_start = None;
        self.sync_edit_dimensions();
        self.update_texture(ctx);
    }

    fn redo_image_edit(&mut self, ctx: &egui::Context) {
        let Some(next) = self.image_redo_stack.pop() else {
            return;
        };
        if let Some(current) = self.working_image.replace(next) {
            self.image_undo_stack.push(current);
        }
        self.selection_rect = None;
        self.selection_drag_start = None;
        self.sync_edit_dimensions();
        self.update_texture(ctx);
    }

    fn undo(&mut self, ctx: &egui::Context) {
        let Some(previous) = self.undo_stack.pop() else {
            return;
        };
        self.redo_stack.push(self.compose_layers.clone());
        self.compose_layers = previous;
        self.selected_layer =
            normalize_selected_layer(self.selected_layer, self.compose_layers.len());
        self.update_texture(ctx);
    }

    fn redo(&mut self, ctx: &egui::Context) {
        let Some(next) = self.redo_stack.pop() else {
            return;
        };
        self.undo_stack.push(self.compose_layers.clone());
        self.compose_layers = next;
        self.selected_layer =
            normalize_selected_layer(self.selected_layer, self.compose_layers.len());
        self.update_texture(ctx);
    }

    fn add_text_layer(&mut self, ctx: &egui::Context) {
        self.push_history();
        let mut layer = TextLayer {
            name: match self.language {
                Language::ZhTw => format!("文字 {}", self.compose_layers.len() + 1),
                Language::En => format!("Text {}", self.compose_layers.len() + 1),
            },
            ..Default::default()
        };
        if let Some(image) = &self.working_image {
            layer.x = (image.width() / 10) as i32;
            layer.y = (image.height() / 10) as i32;
            layer.font_size = (image.height() as f32 / 12.0).clamp(24.0, 96.0);
        }
        self.compose_layers.push(ComposeLayer::Text(layer));
        self.selected_layer = Some(self.compose_layers.len() - 1);
        self.update_texture(ctx);
        self.status = match self.language {
            Language::ZhTw => "已新增文字圖層。".to_owned(),
            Language::En => "Text layer added.".to_owned(),
        };
    }

    fn add_text_layer_at(&mut self, ctx: &egui::Context, image_pos: Pos2) {
        self.push_history();
        let mut layer = TextLayer {
            name: match self.language {
                Language::ZhTw => format!("文字 {}", self.compose_layers.len() + 1),
                Language::En => format!("Text {}", self.compose_layers.len() + 1),
            },
            ..Default::default()
        };
        layer.x = image_pos.x.round().max(0.0) as i32;
        layer.y = image_pos.y.round().max(0.0) as i32;
        if let Some(image) = &self.working_image {
            layer.font_size = (image.height() as f32 / 12.0).clamp(24.0, 96.0);
        }
        self.compose_layers.push(ComposeLayer::Text(layer));
        self.selected_layer = Some(self.compose_layers.len() - 1);
        self.update_texture(ctx);
        self.status = match self.language {
            Language::ZhTw => "已在畫布位置新增文字，請在右側編輯內容。".to_owned(),
            Language::En => {
                "Text added at the canvas position; edit its content on the right.".to_owned()
            }
        };
    }

    fn shape_tool_kind(&self) -> Option<ShapeKind> {
        match self.preview_tool {
            PreviewTool::Line => Some(ShapeKind::Line),
            PreviewTool::Arrow => Some(ShapeKind::Arrow),
            PreviewTool::Rectangle => Some(ShapeKind::Rectangle),
            _ => None,
        }
    }

    fn add_shape_layer(&mut self, ctx: &egui::Context, kind: ShapeKind, start: Pos2, end: Pos2) {
        self.push_history();
        let mut layer = ShapeLayer::new(
            kind,
            start.x.round() as i32,
            start.y.round() as i32,
            end.x.round() as i32,
            end.y.round() as i32,
        );
        layer.name = match (self.language, kind) {
            (Language::ZhTw, ShapeKind::Line) => format!("線條 {}", self.compose_layers.len() + 1),
            (Language::ZhTw, ShapeKind::Arrow) => format!("箭頭 {}", self.compose_layers.len() + 1),
            (Language::ZhTw, ShapeKind::Rectangle) => {
                format!("框線 {}", self.compose_layers.len() + 1)
            }
            (Language::En, ShapeKind::Line) => format!("Line {}", self.compose_layers.len() + 1),
            (Language::En, ShapeKind::Arrow) => format!("Arrow {}", self.compose_layers.len() + 1),
            (Language::En, ShapeKind::Rectangle) => {
                format!("Rectangle {}", self.compose_layers.len() + 1)
            }
        };
        self.compose_layers.push(ComposeLayer::Shape(layer));
        self.selected_layer = Some(self.compose_layers.len() - 1);
        self.update_texture(ctx);
    }

    fn add_text_watermark(&mut self, ctx: &egui::Context) {
        let Some(image) = &self.working_image else {
            return;
        };
        let image_width = image.width();
        let image_height = image.height();

        self.push_history();
        let text = if self.watermark_text.trim().is_empty() {
            "Pixel Studio".to_owned()
        } else {
            self.watermark_text.trim().to_owned()
        };
        let mut layer = TextLayer {
            name: match self.language {
                Language::ZhTw => format!("浮水印 {}", self.compose_layers.len() + 1),
                Language::En => format!("Watermark {}", self.compose_layers.len() + 1),
            },
            text,
            font_size: self.watermark_font_size.clamp(12.0, 256.0),
            color: [255, 255, 255, 255],
            opacity: self.watermark_opacity.clamp(0.05, 1.0),
            stroke: true,
            shadow: true,
            ..Default::default()
        };

        let bounds = text_layer_bounds(&layer, self.font_bytes.as_deref());
        let margin = ((image_width.min(image_height) as f32) * 0.03).clamp(16.0, 64.0) as i32;
        layer.x = (image_width as i32 - bounds.width as i32 - margin).max(margin);
        layer.y = (image_height as i32 - bounds.height as i32 - margin).max(margin);

        self.compose_layers.push(ComposeLayer::Text(layer));
        self.selected_layer = Some(self.compose_layers.len() - 1);
        self.update_texture(ctx);
        self.status = match self.language {
            Language::ZhTw => "已新增文字浮水印。".to_owned(),
            Language::En => "Text watermark added.".to_owned(),
        };
    }

    fn add_image_layer_from_file(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new().pick_file() else {
            return;
        };

        match load_dynamic_image(&path) {
            Ok(image) => {
                let name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("image")
                    .to_owned();
                self.push_image_layer(ctx, name, image);
                self.status = match self.language {
                    Language::ZhTw => format!("已加入圖片圖層：{}", path.display()),
                    Language::En => format!("Image layer added: {}", path.display()),
                };
            }
            Err(error) => {
                self.status = match self.language {
                    Language::ZhTw => format!("無法加入圖片：{error}"),
                    Language::En => format!("Failed to add image: {error}"),
                };
            }
        }
    }

    fn paste_clipboard_image(&mut self, ctx: &egui::Context) {
        match self.read_clipboard_image() {
            Ok(image) => {
                if self.working_image.is_some() {
                    self.push_pasted_image_layer(ctx, image);
                    self.status = match self.language {
                        Language::ZhTw => "已從剪貼簿貼上圖片圖層。".to_owned(),
                        Language::En => "Pasted clipboard image as a layer.".to_owned(),
                    };
                } else {
                    self.open_clipboard_image(ctx, image);
                }
            }
            Err(error) => {
                self.status = match self.language {
                    Language::ZhTw => format!("剪貼簿沒有可貼上的圖片：{error}"),
                    Language::En => format!("No image could be pasted from clipboard: {error}"),
                };
            }
        }
    }

    fn capture_screenshot(&mut self, ctx: &egui::Context) {
        // Hide the editor before capturing so a full-screen shot does not
        // contain Pixel Studio itself. The provider remains platform-neutral;
        // non-Windows builds return an actionable fallback error.
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        std::thread::sleep(std::time::Duration::from_millis(150));
        let result = crate::capture::capture_fullscreen();
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));

        match result {
            Ok(image) => {
                let clipboard_result = arboard::Clipboard::new().and_then(|mut clipboard| {
                    let rgba = image.to_rgba8();
                    clipboard.set_image(arboard::ImageData {
                        width: rgba.width() as usize,
                        height: rgba.height() as usize,
                        bytes: Cow::Owned(rgba.into_raw()),
                    })
                });
                self.open_clipboard_image(ctx, image);
                if let Err(error) = clipboard_result {
                    self.status = match self.language {
                        Language::ZhTw => format!("截圖已載入，但無法保留到剪貼簿：{error}"),
                        Language::En => format!(
                            "Screenshot loaded, but it could not be copied to the clipboard: {error}"
                        ),
                    };
                }
            }
            Err(error) => {
                self.status = match self.language {
                    Language::ZhTw => format!("截圖失敗：{error}"),
                    Language::En => format!("Screenshot failed: {error}"),
                };
            }
        }
    }

    fn open_clipboard_image(&mut self, ctx: &egui::Context, image: DynamicImage) {
        let width = image.width();
        let height = image.height();
        self.original_image = Some(image.clone());
        self.working_image = Some(image);
        self.compose_layers.clear();
        self.selected_layer = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.image_undo_stack.clear();
        self.image_redo_stack.clear();
        self.drag_snapshot = None;
        self.selection_rect = None;
        self.selection_drag_start = None;
        self.image_info = None;
        self.image_path = None;
        self.reset_c2pa_state(None);
        self.folder_files.clear();
        self.selected_index = None;
        self.sync_edit_dimensions();
        self.update_texture(ctx);
        self.status = match self.language {
            Language::ZhTw => format!("已從剪貼簿貼上圖片（{width}x{height}）。"),
            Language::En => format!("Pasted image from clipboard ({width}x{height})."),
        };
    }

    fn paste_image_layer(&mut self, ctx: &egui::Context) {
        match self.read_clipboard_image() {
            Ok(image) => {
                self.push_pasted_image_layer(ctx, image);
                self.status = match self.language {
                    Language::ZhTw => "已從剪貼簿貼上圖片。".to_owned(),
                    Language::En => "Image pasted from clipboard.".to_owned(),
                };
            }
            Err(error) => {
                self.status = match self.language {
                    Language::ZhTw => format!("剪貼簿沒有可貼上的圖片：{error}"),
                    Language::En => format!("No image could be pasted from clipboard: {error}"),
                };
            }
        }
    }

    fn read_clipboard_image(&self) -> Result<DynamicImage, String> {
        match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_image()) {
            Ok(image) => {
                let width = image.width as u32;
                let height = image.height as u32;
                let bytes = image.bytes.into_owned();
                if let Some(rgba) = image::RgbaImage::from_raw(width, height, bytes) {
                    Ok(DynamicImage::ImageRgba8(rgba))
                } else {
                    Err(match self.language {
                        Language::ZhTw => "剪貼簿圖片格式無法讀取。".to_owned(),
                        Language::En => "Clipboard image format could not be read.".to_owned(),
                    })
                }
            }
            Err(error) => Err(error.to_string()),
        }
    }

    fn push_pasted_image_layer(&mut self, ctx: &egui::Context, image: DynamicImage) {
        let name = match self.language {
            Language::ZhTw => format!("貼上圖片 {}", self.compose_layers.len() + 1),
            Language::En => format!("Pasted Image {}", self.compose_layers.len() + 1),
        };
        self.push_image_layer(ctx, name, image);
    }

    fn push_image_layer(&mut self, ctx: &egui::Context, name: String, image: DynamicImage) {
        self.push_history();
        let mut layer = ImageLayer::new(name, image);
        if let Some(background) = &self.working_image {
            let max_width = (background.width() / 2).max(1);
            let max_height = (background.height() / 2).max(1);
            let scale = (max_width as f32 / layer.width as f32)
                .min(max_height as f32 / layer.height as f32)
                .min(1.0);
            layer.width = ((layer.width as f32 * scale).round() as u32).max(1);
            layer.height = ((layer.height as f32 * scale).round() as u32).max(1);
            layer.x = ((background.width().saturating_sub(layer.width)) / 2) as i32;
            layer.y = ((background.height().saturating_sub(layer.height)) / 2) as i32;
        }
        self.compose_layers.push(ComposeLayer::Image(layer));
        self.selected_layer = Some(self.compose_layers.len() - 1);
        self.update_texture(ctx);
    }

    fn export_composite(&mut self) {
        let Some(image) = self.render_current_image() else {
            return;
        };

        let Some(output) = rfd::FileDialog::new()
            .set_file_name(format!("composite.{}", self.output_format.extension()))
            .save_file()
        else {
            return;
        };

        match save_dynamic_image(
            &image,
            &output,
            ConvertOptions {
                format: self.output_format,
                quality: self.quality,
                background: [255, 255, 255, 255],
            },
        ) {
            Ok(result) => {
                self.status = match self.language {
                    Language::ZhTw => format!(
                        "已匯出合成圖：{}（{}x{}）",
                        result.output.display(),
                        result.width,
                        result.height
                    ),
                    Language::En => format!(
                        "Composite exported to {} ({}x{})",
                        result.output.display(),
                        result.width,
                        result.height
                    ),
                };
            }
            Err(error) => {
                self.status = match self.language {
                    Language::ZhTw => format!("合成圖匯出失敗：{error}"),
                    Language::En => format!("Composite export failed: {error}"),
                };
            }
        }
    }

    fn save_project(&mut self) {
        let Some(output) = rfd::FileDialog::new()
            .add_filter("Pixel Studio Project", &["pixel-project"])
            .set_file_name("photo.pixel-project")
            .save_file()
        else {
            return;
        };

        let result = self
            .project_from_state()
            .and_then(|project| {
                serde_json::to_string_pretty(&project).map_err(|error| error.to_string())
            })
            .and_then(|json| std::fs::write(&output, json).map_err(|error| error.to_string()));

        match result {
            Ok(()) => {
                self.status = match self.language {
                    Language::ZhTw => format!("已儲存專案：{}", output.display()),
                    Language::En => format!("Project saved: {}", output.display()),
                };
            }
            Err(error) => {
                self.status = match self.language {
                    Language::ZhTw => format!("專案儲存失敗：{error}"),
                    Language::En => format!("Project save failed: {error}"),
                };
            }
        }
    }

    fn open_project(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Pixel Studio Project", &["pixel-project"])
            .pick_file()
        else {
            return;
        };

        let result = std::fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|json| {
                serde_json::from_str::<ProjectFile>(&json).map_err(|error| error.to_string())
            })
            .and_then(|project| self.apply_project(ctx, project));

        match result {
            Ok(()) => {
                self.status = match self.language {
                    Language::ZhTw => format!("已開啟專案：{}", path.display()),
                    Language::En => format!("Project opened: {}", path.display()),
                };
            }
            Err(error) => {
                self.status = match self.language {
                    Language::ZhTw => format!("專案開啟失敗：{error}"),
                    Language::En => format!("Project open failed: {error}"),
                };
            }
        }
    }

    fn project_from_state(&self) -> Result<ProjectFile, String> {
        let background = self
            .working_image
            .as_ref()
            .ok_or_else(|| "No image loaded".to_owned())?;
        let mut layers = Vec::with_capacity(self.compose_layers.len());

        for layer in &self.compose_layers {
            match layer {
                ComposeLayer::Text(layer) => layers.push(ProjectLayer::Text {
                    name: layer.name.clone(),
                    text: layer.text.clone(),
                    font_path: layer
                        .font_path
                        .as_ref()
                        .map(|path| path.to_string_lossy().to_string()),
                    x: layer.x,
                    y: layer.y,
                    font_size: layer.font_size,
                    color: layer.color,
                    opacity: layer.opacity,
                    stroke: layer.stroke,
                    shadow: layer.shadow,
                    visible: layer.visible,
                }),
                ComposeLayer::Image(layer) => layers.push(ProjectLayer::Image {
                    name: layer.name.clone(),
                    image_png: encode_png_base64(&layer.image)?,
                    x: layer.x,
                    y: layer.y,
                    width: layer.width,
                    height: layer.height,
                    opacity: layer.opacity,
                    visible: layer.visible,
                }),
                ComposeLayer::Shape(layer) => layers.push(ProjectLayer::Shape {
                    name: layer.name.clone(),
                    shape_kind: layer.kind,
                    x1: layer.x1,
                    y1: layer.y1,
                    x2: layer.x2,
                    y2: layer.y2,
                    stroke_color: layer.stroke_color,
                    stroke_width: layer.stroke_width,
                    opacity: layer.opacity,
                    filled: layer.filled,
                    fill_color: layer.fill_color,
                    visible: layer.visible,
                }),
            }
        }

        Ok(ProjectFile {
            version: 1,
            background_png: encode_png_base64(background)?,
            layers,
        })
    }

    fn apply_project(&mut self, ctx: &egui::Context, project: ProjectFile) -> Result<(), String> {
        if project.version != 1 {
            return Err(format!("Unsupported project version {}", project.version));
        }

        let background = decode_png_base64(&project.background_png)?;
        let mut layers = Vec::with_capacity(project.layers.len());

        for layer in project.layers {
            match layer {
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
                } => layers.push(ComposeLayer::Text(TextLayer {
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
                })),
                ProjectLayer::Image {
                    name,
                    image_png,
                    x,
                    y,
                    width,
                    height,
                    opacity,
                    visible,
                } => layers.push(ComposeLayer::Image(ImageLayer {
                    name,
                    image: decode_png_base64(&image_png)?,
                    x,
                    y,
                    width,
                    height,
                    opacity,
                    visible,
                })),
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
                } => layers.push(ComposeLayer::Shape(ShapeLayer {
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
                })),
            }
        }

        self.original_image = Some(background.clone());
        self.working_image = Some(background);
        self.image_path = None;
        self.image_info = None;
        self.compose_layers = layers;
        self.selected_layer = normalize_selected_layer(None, self.compose_layers.len());
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.image_undo_stack.clear();
        self.image_redo_stack.clear();
        self.drag_snapshot = None;
        self.selection_rect = None;
        self.selection_drag_start = None;
        self.sync_edit_dimensions();
        self.update_texture(ctx);
        Ok(())
    }

    fn can_move_layer_up(&self) -> bool {
        self.selected_layer
            .is_some_and(|index| index + 1 < self.compose_layers.len())
    }

    fn can_move_layer_down(&self) -> bool {
        self.selected_layer.is_some_and(|index| index > 0)
    }

    fn move_selected_layer(&mut self, direction: i32) {
        let Some(index) = self.selected_layer else {
            return;
        };
        let new_index = if direction < 0 {
            index + 1
        } else {
            index.saturating_sub(1)
        };
        if new_index < self.compose_layers.len() {
            self.compose_layers.swap(index, new_index);
            self.selected_layer = Some(new_index);
        }
    }

    fn duplicate_selected_layer(&mut self) {
        let Some(index) = self.selected_layer else {
            return;
        };
        let Some(layer) = self.compose_layers.get(index).cloned() else {
            return;
        };
        let mut layer = layer;
        match &mut layer {
            ComposeLayer::Text(layer) => {
                layer.name = format!("{} copy", layer.name);
                layer.x += 16;
                layer.y += 16;
            }
            ComposeLayer::Image(layer) => {
                layer.name = format!("{} copy", layer.name);
                layer.x += 16;
                layer.y += 16;
            }
            ComposeLayer::Shape(layer) => {
                layer.name = format!("{} copy", layer.name);
                layer.x1 += 16;
                layer.y1 += 16;
                layer.x2 += 16;
                layer.y2 += 16;
            }
        }
        let insert_at = index + 1;
        self.compose_layers.insert(insert_at, layer);
        self.selected_layer = Some(insert_at);
    }

    fn move_layer_by(&mut self, index: usize, delta: Vec2) {
        let Some(layer) = self.compose_layers.get_mut(index) else {
            return;
        };
        let dx = delta.x.round() as i32;
        let dy = delta.y.round() as i32;
        match layer {
            ComposeLayer::Text(layer) => {
                layer.x += dx;
                layer.y += dy;
            }
            ComposeLayer::Image(layer) => {
                layer.x += dx;
                layer.y += dy;
            }
            ComposeLayer::Shape(layer) => {
                layer.x1 += dx;
                layer.y1 += dy;
                layer.x2 += dx;
                layer.y2 += dy;
            }
        }
    }

    fn hit_test_layer(&self, image_pos: Pos2) -> Option<usize> {
        self.compose_layers
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, layer)| {
                (layer.is_visible() && self.layer_bounds(layer).contains(image_pos))
                    .then_some(index)
            })
    }

    fn can_align_selected_layer(&self) -> bool {
        self.selected_layer.is_some() && self.working_image.is_some()
    }

    fn align_selected_layer(&mut self, target: AlignTarget) {
        let Some(index) = self.selected_layer else {
            return;
        };
        let Some(image) = &self.working_image else {
            return;
        };
        let Some(layer) = self.compose_layers.get(index) else {
            return;
        };

        let bounds = self.layer_bounds(layer);
        let image_width = image.width() as f32;
        let image_height = image.height() as f32;
        let dx = match target {
            AlignTarget::Left => -bounds.min.x,
            AlignTarget::Center => (image_width - bounds.width()) / 2.0 - bounds.min.x,
            AlignTarget::Right => image_width - bounds.width() - bounds.min.x,
            _ => 0.0,
        };
        let dy = match target {
            AlignTarget::Top => -bounds.min.y,
            AlignTarget::Middle => (image_height - bounds.height()) / 2.0 - bounds.min.y,
            AlignTarget::Bottom => image_height - bounds.height() - bounds.min.y,
            _ => 0.0,
        };

        self.move_layer_by(index, Vec2::new(dx, dy));
    }

    fn layer_bounds(&self, layer: &ComposeLayer) -> Rect {
        match layer {
            ComposeLayer::Text(layer) => {
                let bounds = text_layer_bounds(layer, self.font_bytes.as_deref());
                Rect::from_min_size(
                    Pos2::new(bounds.x as f32, bounds.y as f32),
                    Vec2::new(bounds.width as f32, bounds.height as f32),
                )
            }
            ComposeLayer::Image(layer) => Rect::from_min_size(
                Pos2::new(layer.x as f32, layer.y as f32),
                Vec2::new(layer.width as f32, layer.height as f32),
            ),
            ComposeLayer::Shape(layer) => {
                let bounds = layer.bounds();
                Rect::from_min_size(
                    Pos2::new(bounds.x as f32, bounds.y as f32),
                    Vec2::new(bounds.width as f32, bounds.height as f32),
                )
            }
        }
    }

    fn current_selection_mask(&self) -> Option<SelectionMask> {
        let image = self.working_image.as_ref()?;
        let rect = self.selection_rect?;
        let x = rect.min.x.floor().max(0.0) as u32;
        let y = rect.min.y.floor().max(0.0) as u32;
        let width = rect.width().ceil().max(1.0) as u32;
        let height = rect.height().ceil().max(1.0) as u32;
        let x = x.min(image.width().saturating_sub(1));
        let y = y.min(image.height().saturating_sub(1));

        match self.selection_shape {
            SelectionShape::Rectangle => Some(SelectionMask::Rectangle {
                x,
                y,
                width: width.min(image.width().saturating_sub(x)).max(1),
                height: height.min(image.height().saturating_sub(y)).max(1),
            }),
            SelectionShape::Circle => {
                let diameter = width
                    .min(height)
                    .min(image.width().saturating_sub(x))
                    .min(image.height().saturating_sub(y))
                    .max(1);
                Some(SelectionMask::Circle { x, y, diameter })
            }
        }
    }

    fn delete_selected_layer(&mut self) {
        let Some(index) = self.selected_layer else {
            return;
        };
        if index < self.compose_layers.len() {
            self.compose_layers.remove(index);
        }
        self.selected_layer = if self.compose_layers.is_empty() {
            None
        } else {
            Some(index.min(self.compose_layers.len() - 1))
        };
    }

    fn previous_index(&self) -> Option<usize> {
        self.selected_index
            .filter(|index| *index > 0)
            .map(|index| index - 1)
    }

    fn next_index(&self) -> Option<usize> {
        self.selected_index
            .and_then(|index| (index + 1 < self.folder_files.len()).then_some(index + 1))
    }
}

fn screen_to_image(pointer: Pos2, image_rect: Rect, scale: f32) -> Pos2 {
    let offset = (pointer - image_rect.min) / scale.max(0.01);
    Pos2::new(offset.x, offset.y)
}

fn selection_rect_from_points(
    start: Pos2,
    current: Pos2,
    shape: SelectionShape,
    image_size: Vec2,
) -> Rect {
    let start = clamp_image_pos(start, image_size);
    let mut current = clamp_image_pos(current, image_size);

    if shape == SelectionShape::Circle {
        let delta = current - start;
        let side = delta.x.abs().max(delta.y.abs());
        current = Pos2::new(
            start.x + side * delta.x.signum(),
            start.y + side * delta.y.signum(),
        );
        current = clamp_image_pos(current, image_size);
    }

    let min = Pos2::new(start.x.min(current.x), start.y.min(current.y));
    let max = Pos2::new(start.x.max(current.x), start.y.max(current.y));
    Rect::from_min_max(min, max).expand(0.0)
}

fn clamp_image_pos(pos: Pos2, image_size: Vec2) -> Pos2 {
    Pos2::new(
        pos.x.clamp(0.0, image_size.x.max(1.0)),
        pos.y.clamp(0.0, image_size.y.max(1.0)),
    )
}

fn is_supported_drop_path(path: &Path) -> bool {
    path.is_dir() || SupportedFormat::from_path(path).is_some()
}

fn normalize_selected_layer(selected: Option<usize>, len: usize) -> Option<usize> {
    if len == 0 {
        None
    } else {
        Some(selected.unwrap_or(0).min(len - 1))
    }
}

fn default_c2pa_title(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("image")
        .to_owned()
}

fn file_name_with_suffix(path: &Path, suffix: &str) -> String {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("jpg");
    format!("{stem}{suffix}.{extension}")
}

fn same_existing_path(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn temporary_c2pa_source_path(output: &Path) -> PathBuf {
    let extension = output
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("tmp");
    let stamp = Local::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| Local::now().timestamp_micros() * 1_000);
    let file_name = format!(".studio-app-c2pa-{stamp}.{extension}");

    output
        .parent()
        .map(|parent| parent.join(&file_name))
        .unwrap_or_else(|| std::env::temp_dir().join(file_name))
}

fn encode_png_base64(image: &DynamicImage) -> Result<String, String> {
    let mut cursor = Cursor::new(Vec::new());
    image
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(general_purpose::STANDARD.encode(cursor.into_inner()))
}

fn decode_png_base64(encoded: &str) -> Result<DynamicImage, String> {
    let bytes = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| error.to_string())?;
    image::load_from_memory(&bytes).map_err(|error| error.to_string())
}

fn load_font_choices() -> Vec<FontChoice> {
    crate::fonts::list_system_font_paths()
        .into_iter()
        .map(|path| {
            let name = crate::fonts::display_font_name(&path);
            let preview_family = crate::fonts::preview_family_name(&path);
            FontChoice {
                name,
                path,
                preview_family,
            }
        })
        .collect()
}
