use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use base64::{Engine as _, engine::general_purpose};
use chrono::Local;
use eframe::egui;
use egui::{
    Color32, ColorImage, Pos2, Rect, Stroke, StrokeKind, TextureHandle, TextureOptions, Vec2,
};
use image::DynamicImage;
use studio_core::transform::SelectionMask;
use studio_core::{
    BatchOptions, BatchSummary, C2paAddOptions, C2paInfo, C2paManifestDraft, C2paRemovalOptions,
    ComposeDocument, ComposeLayer, ConvertOptions, ConvertResult, GifOptions, ImageInfo,
    ImageLayer, PanoramaMode, PanoramaOptions, SupportedFormat, TextLayer, collect_supported_files,
    inspect_c2pa, inspect_image, load_dynamic_image, process_batch_with_progress,
    process_c2pa_add_batch_with_progress, process_c2pa_removal_batch_with_progress,
    remove_c2pa_manifest, render_composition, save_dynamic_image, text_layer_bounds,
    write_animated_gif, write_c2pa_manifest, write_panorama_dynamic_jpeg,
};

use crate::i18n::{Language, TextKey};
use crate::ui::canvas_view::{self, CanvasInput};
use crate::ui::state::{AdjustUiValues, LeftToolCategory, ToolId, UiState};
use crate::ui::theme::{UiTokens, apply_visuals};
use crate::ui::{
    CommandAvailability, HistoryView, LayerKindUi, LayerView, UiAction, contextual_toolbar,
    left_tools, right_panel, status_bar, top_menu_bar,
};

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
    Hand,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DrawTool {
    None,
    Brush,
    Line,
}

impl DrawTool {
    fn is_active(self) -> bool {
        !matches!(self, Self::None)
    }
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
}

pub struct PixelStudioApp {
    language: Language,
    ui_state: UiState,
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
    draw_tool: DrawTool,
    draw_color: Color32,
    draw_size: f32,
    draw_opacity: f32,
    drawing_active: bool,
    drawing_start: Option<Pos2>,
    drawing_last: Option<Pos2>,
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
            ui_state: UiState::default(),
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
            draw_tool: DrawTool::None,
            draw_color: Color32::BLACK,
            draw_size: 8.0,
            draw_opacity: 1.0,
            drawing_active: false,
            drawing_start: None,
            drawing_last: None,
            output_format: SupportedFormat::Jpeg,
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
        let ctx = ui.ctx().clone();
        let tokens = UiTokens::from_ctx(&ctx);
        apply_visuals(&ctx, &tokens);
        self.poll_batch(&ctx);
        self.open_pending_path(&ctx);
        self.open_dropped_files(&ctx);
        self.handle_keyboard_shortcuts(&ctx);

        let availability = self.ui_command_availability();
        let language = self.language;
        let mut actions = Vec::new();

        egui::Panel::top("top_menu_bar")
            .exact_size(40.0)
            .frame(panel_frame(&tokens, tokens.bg_primary, PanelEdge::Bottom))
            .show_inside(ui, |ui| {
                top_menu_bar::show_top_menu_bar(ui, &tokens, language, availability, &mut actions);
            });
        egui::Panel::top("contextual_toolbar")
            .exact_size(34.0)
            .frame(panel_frame(&tokens, tokens.bg_primary, PanelEdge::Bottom))
            .show_inside(ui, |ui| {
                contextual_toolbar::show_contextual_toolbar(
                    ui,
                    &tokens,
                    language,
                    &mut self.ui_state,
                    availability,
                    &mut actions,
                );
            });
        egui::Panel::bottom("status_bar")
            .exact_size(26.0)
            .frame(panel_frame(&tokens, tokens.bg_secondary, PanelEdge::Top))
            .show_inside(ui, |ui| {
                status_bar::show_status_bar(
                    ui,
                    &tokens,
                    language,
                    self.working_image
                        .as_ref()
                        .map(|image| (image.width(), image.height())),
                    &mut self.ui_state,
                    &mut actions,
                );
            });
        egui::Panel::left("tool_rail")
            .exact_size(50.0)
            .resizable(false)
            .frame(panel_frame(&tokens, tokens.bg_secondary, PanelEdge::Right))
            .show_inside(ui, |ui| {
                left_tools::show_category_rail(
                    &ctx,
                    ui,
                    &tokens,
                    language,
                    &mut self.ui_state,
                    &mut actions,
                );
            });

        if self.ui_state.left_tools_open {
            let category = self.ui_state.left_category;
            egui::Panel::left("left_tools_panel")
                .exact_size(300.0)
                .resizable(false)
                .frame(panel_frame(&tokens, tokens.bg_primary, PanelEdge::Right))
                .show_inside(ui, |ui| {
                    self.show_left_category_panel(ui, &ctx, &tokens, category, &mut actions);
                });
        }

        let layer_views = self.layer_views();
        let history = HistoryView {
            image_undo_count: self.image_undo_stack.len(),
            layer_undo_count: self.undo_stack.len(),
        };
        egui::Panel::right("right_panel")
            .exact_size(260.0)
            .resizable(false)
            .frame(panel_frame(&tokens, tokens.bg_primary, PanelEdge::Left))
            .show_inside(ui, |ui| {
                right_panel::show_right_panel(
                    ui,
                    &mut self.ui_state,
                    right_panel::RightPanelInput {
                        tokens: &tokens,
                        language,
                        availability,
                        layers: &layer_views,
                        history,
                    },
                    &mut actions,
                );
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(tokens.bg_tertiary))
            .show_inside(ui, |ui| self.preview_panel(ui, &tokens));

        self.dispatch_ui_actions(&ctx, actions);
        self.sync_ui_to_editor_tool();
    }
}

#[derive(Clone, Copy)]
enum PanelEdge {
    Top,
    Right,
    Bottom,
    Left,
}

fn panel_frame(tokens: &UiTokens, fill: Color32, edge: PanelEdge) -> egui::Frame {
    let margin = match edge {
        PanelEdge::Top => egui::Margin::symmetric(10, 3),
        PanelEdge::Bottom => egui::Margin::symmetric(10, 3),
        PanelEdge::Left | PanelEdge::Right => egui::Margin::symmetric(6, 8),
    };
    egui::Frame::new()
        .fill(fill)
        .inner_margin(margin)
        .stroke(Stroke::new(1.0, tokens.border_tertiary))
}

impl PixelStudioApp {
    fn tr(&self, key: TextKey) -> &'static str {
        self.language.t(key)
    }

    fn ui_command_availability(&self) -> CommandAvailability {
        CommandAvailability {
            has_image: self.working_image.is_some(),
            can_undo: self.can_undo(),
            can_redo: self.can_redo(),
            can_undo_image: self.can_undo_image(),
            can_redo_image: self.can_redo_image(),
            has_selection: self.selection_rect.is_some(),
            has_selected_layer: self.selected_layer.is_some(),
            can_restore_canvas: self.cleared_canvas.is_some(),
        }
    }

    fn layer_views(&self) -> Vec<LayerView> {
        self.compose_layers
            .iter()
            .enumerate()
            .map(|(index, layer)| {
                let (kind, opacity) = match layer {
                    ComposeLayer::Text(layer) => (LayerKindUi::Text, layer.opacity),
                    ComposeLayer::Image(layer) => (LayerKindUi::Image, layer.opacity),
                };
                LayerView {
                    index,
                    name: layer.name().to_owned(),
                    visible: layer.is_visible(),
                    selected: self.selected_layer == Some(index),
                    opacity_percent: (opacity.clamp(0.0, 1.0) * 100.0).round() as u32,
                    kind,
                }
            })
            .collect()
    }

    fn dispatch_ui_actions(&mut self, ctx: &egui::Context, actions: Vec<UiAction>) {
        for action in actions {
            match action {
                UiAction::OpenImage => {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.open_path(ctx, path);
                    }
                }
                UiAction::OpenFolder => {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.open_folder(ctx, path);
                    }
                }
                UiAction::PasteImage => self.paste_clipboard_image(ctx),
                UiAction::OpenProject => self.open_project(ctx),
                UiAction::SaveProject => self.save_project(),
                UiAction::SaveAs => self.save_as_new_file(),
                UiAction::ExportComposite => self.export_composite(),
                UiAction::Reload => {
                    if let Some(path) = self.image_path.clone() {
                        self.open_image(ctx, path);
                    }
                }
                UiAction::ClearCanvas => self.clear_canvas(ctx),
                UiAction::RestoreCanvas => self.restore_canvas(ctx),
                UiAction::Undo => self.undo(ctx),
                UiAction::Redo => self.redo(ctx),
                UiAction::ImageUndo => self.undo_image_edit(ctx),
                UiAction::ImageRedo => self.redo_image_edit(ctx),
                UiAction::RotateLeft => {
                    self.map_working_image(ctx, studio_core::transform::rotate_left)
                }
                UiAction::RotateRight => {
                    self.map_working_image(ctx, studio_core::transform::rotate_right)
                }
                UiAction::Rotate180 => {
                    self.map_working_image(ctx, studio_core::filters::rotate_180)
                }
                UiAction::FlipHorizontal => {
                    self.map_working_image(ctx, studio_core::transform::flip_horizontal)
                }
                UiAction::FlipVertical => {
                    self.map_working_image(ctx, studio_core::transform::flip_vertical)
                }
                UiAction::Grayscale => {
                    self.map_scoped_working_image(ctx, studio_core::transform::grayscale)
                }
                UiAction::Invert => {
                    self.map_scoped_working_image(ctx, studio_core::transform::invert)
                }
                UiAction::AutoContrast => {
                    self.map_scoped_working_image(ctx, studio_core::filters::auto_contrast)
                }
                UiAction::ResetEdits => self.reset_edits(ctx),
                UiAction::AddTextLayer => self.add_text_layer(ctx),
                UiAction::AddImageLayer => self.add_image_layer_from_file(ctx),
                UiAction::DuplicateLayer => {
                    if self.selected_layer.is_some() {
                        self.push_history();
                        self.duplicate_selected_layer();
                        self.update_texture(ctx);
                    }
                }
                UiAction::DeleteLayer => {
                    if self.selected_layer.is_some() {
                        self.push_history();
                        self.delete_selected_layer();
                        self.update_texture(ctx);
                    }
                }
                UiAction::ApplyCrop => {
                    self.sync_crop_fields_from_selection();
                    self.apply_crop(ctx);
                }
                UiAction::ClearSelection => {
                    self.selection_rect = None;
                    self.selection_drag_start = None;
                }
                UiAction::ApplyAdjustments(values) => self.apply_ui_adjustments(ctx, values),
                UiAction::SelectTool(tool) => self.ui_state.active_tool = tool,
                UiAction::SelectLayer(index) => self.selected_layer = Some(index),
                UiAction::ToggleLayerVisibility(index, visible) => {
                    self.set_layer_visibility(ctx, index, visible)
                }
                UiAction::SetLayerOpacity(index, opacity) => {
                    self.set_layer_opacity(ctx, index, opacity)
                }
                UiAction::SetLanguage(language) => self.language = language,
                UiAction::FitToWindow => {
                    self.fit_to_window = true;
                    self.preview_pan = Vec2::ZERO;
                }
                UiAction::ActualSize => {
                    self.fit_to_window = false;
                    self.zoom = 1.0;
                    self.ui_state.zoom_percent = 100;
                    self.preview_pan = Vec2::ZERO;
                }
                UiAction::SetZoom(percent) => {
                    self.fit_to_window = false;
                    self.zoom = percent as f32 / 100.0;
                    self.ui_state.zoom_percent = percent.clamp(10, 400);
                }
                UiAction::ToggleLeftTools => {
                    self.ui_state.left_tools_open = !self.ui_state.left_tools_open;
                }
            }
        }
    }

    fn sync_ui_to_editor_tool(&mut self) {
        self.draw_size = self.ui_state.brush_ui.size_px.clamp(1, 200) as f32;
        self.draw_opacity = self.ui_state.brush_ui.opacity.min(100) as f32 / 100.0;
        self.draw_color = self.ui_state.brush_ui.color;

        match self.ui_state.active_tool {
            ToolId::Brush => {
                self.preview_tool = PreviewTool::Select;
                self.draw_tool = DrawTool::Brush;
            }
            ToolId::Line => {
                self.preview_tool = PreviewTool::Select;
                self.draw_tool = DrawTool::Line;
            }
            ToolId::Zoom => {
                self.preview_tool = PreviewTool::Hand;
                self.draw_tool = DrawTool::None;
            }
            _ => {
                self.preview_tool = PreviewTool::Select;
                self.draw_tool = DrawTool::None;
            }
        }
    }

    fn show_left_category_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        tokens: &UiTokens,
        category: LeftToolCategory,
        actions: &mut Vec<UiAction>,
    ) {
        ui.horizontal(|ui| {
            ui.heading(left_tools::category_label(self.language, category));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("×")
                    .on_hover_text(self.tr(TextKey::ClosePanel))
                    .clicked()
                {
                    self.ui_state.left_tools_open = false;
                }
            });
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("left_category_content")
            .auto_shrink([false, false])
            .show(ui, |ui| match category {
                LeftToolCategory::File => self.file_tools_panel(ui, ctx),
                LeftToolCategory::Browse => self.metadata_panel(ui, ctx),
                LeftToolCategory::Drawing => {
                    left_tools::show_tool_picker(
                        ui,
                        tokens,
                        self.language,
                        &mut self.ui_state,
                        actions,
                    );
                    ui.add_space(8.0);
                    ui.label(self.tr(TextKey::DrawingSettingsHint));
                }
                LeftToolCategory::Edit => self.edit_panel(ui, ctx),
                LeftToolCategory::Layers => self.compose_panel(ui, ctx),
                LeftToolCategory::Export => {
                    self.convert_panel(ui);
                    ui.add_space(18.0);
                    self.panorama_panel(ui);
                }
                LeftToolCategory::Gif => self.gif_panel(ui, ctx),
                LeftToolCategory::Batch => self.batch_panel(ui),
                LeftToolCategory::C2pa => self.c2pa_panel(ui),
            });
    }

    fn sync_crop_fields_from_selection(&mut self) {
        let Some(rect) = self.selection_rect else {
            return;
        };
        self.crop_x = rect.min.x.max(0.0).round() as u32;
        self.crop_y = rect.min.y.max(0.0).round() as u32;
        self.crop_width = rect.width().max(1.0).round() as u32;
        self.crop_height = rect.height().max(1.0).round() as u32;
    }

    fn apply_ui_adjustments(&mut self, ctx: &egui::Context, values: AdjustUiValues) {
        let brightness = (values.brightness - 50) * 2;
        let contrast = ((values.contrast - 50) * 2) as f32;
        let saturation = ((values.saturation - 50) * 2) as f32;
        let temperature = ((values.temperature - 50) * 2) as f32;
        if brightness == 0 && contrast == 0.0 && saturation == 0.0 && temperature == 0.0 {
            return;
        }
        self.map_scoped_working_image(ctx, |image| {
            let image = studio_core::transform::adjust_brightness(image, brightness);
            let image = studio_core::transform::adjust_contrast(&image, contrast);
            let image = studio_core::transform::adjust_saturation(&image, saturation);
            studio_core::filters::adjust_temperature(&image, temperature)
        });
    }

    fn set_layer_visibility(&mut self, ctx: &egui::Context, index: usize, visible: bool) {
        if self.compose_layers.get(index).is_none() {
            return;
        }
        self.push_history();
        self.compose_layers[index].set_visible(visible);
        self.update_texture(ctx);
    }

    fn set_layer_opacity(&mut self, ctx: &egui::Context, index: usize, opacity: u32) {
        if self.compose_layers.get(index).is_none() {
            return;
        }
        self.push_history();
        let opacity = opacity.min(100) as f32 / 100.0;
        match &mut self.compose_layers[index] {
            ComposeLayer::Text(layer) => layer.opacity = opacity,
            ComposeLayer::Image(layer) => layer.opacity = opacity,
        }
        self.update_texture(ctx);
    }

    fn file_tools_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            if ui.button(self.tr(TextKey::OpenImage)).clicked()
                && let Some(path) = rfd::FileDialog::new().pick_file()
            {
                self.open_path(ctx, path);
            }

            if ui.button(self.tr(TextKey::PasteImage)).clicked() {
                self.paste_clipboard_image(ctx);
            }
            if ui.button(self.tr(TextKey::OpenFolder)).clicked()
                && let Some(path) = rfd::FileDialog::new().pick_folder()
            {
                self.open_folder(ctx, path);
            }
        });

        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    self.working_image.is_some(),
                    egui::Button::new(self.tr(TextKey::ClearCanvas)),
                )
                .clicked()
            {
                self.clear_canvas(ctx);
            }

            if ui
                .add_enabled(
                    self.cleared_canvas.is_some(),
                    egui::Button::new(self.tr(TextKey::RestoreCanvas)),
                )
                .clicked()
            {
                self.restore_canvas(ctx);
            }
            if ui.button(self.tr(TextKey::Reload)).clicked()
                && let Some(path) = self.image_path.clone()
            {
                self.open_image(ctx, path);
            }
        });

        ui.add_space(12.0);
        ui.label(self.tr(TextKey::Project));
        ui.horizontal_wrapped(|ui| {
            if ui.button(self.tr(TextKey::OpenProject)).clicked() {
                self.open_project(ctx);
            }
            if ui
                .add_enabled(
                    self.working_image.is_some(),
                    egui::Button::new(self.tr(TextKey::SaveProject)),
                )
                .clicked()
            {
                self.save_project();
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
        });

        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    self.previous_index().is_some(),
                    egui::Button::new(self.tr(TextKey::Previous)),
                )
                .clicked()
            {
                self.open_folder_index(ctx, self.previous_index().unwrap_or(0));
            }

            if ui
                .add_enabled(
                    self.next_index().is_some(),
                    egui::Button::new(self.tr(TextKey::Next)),
                )
                .clicked()
            {
                self.open_folder_index(ctx, self.next_index().unwrap_or(0));
            }
        });

        ui.add_space(12.0);
        egui::ComboBox::from_label(self.tr(TextKey::Language))
            .selected_text(self.language.name())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.language, Language::ZhTw, Language::ZhTw.name());
                ui.selectable_value(&mut self.language, Language::En, Language::En.name());
            });

        ui.add_space(12.0);
        ui.label(format!("{}: {}", self.tr(TextKey::Status), self.status));
    }

    fn metadata_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
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
                                .font(egui::FontId::proportional(18.0));
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

    fn preview_panel(&mut self, ui: &mut egui::Ui, tokens: &UiTokens) {
        let canvas = canvas_view::show_canvas_view(
            ui,
            tokens,
            CanvasInput {
                texture: self.texture.as_ref(),
                texture_size: self.texture_size,
                fit_to_window: self.fit_to_window,
                zoom_percent: self.ui_state.zoom_percent,
                pan: self.preview_pan,
                pan_mode: self.preview_tool == PreviewTool::Hand,
                empty_hint: self.tr(TextKey::BrowseHint),
            },
        );
        let Some(canvas) = canvas else {
            return;
        };

        if self.fit_to_window {
            self.ui_state.zoom_percent = ((canvas.scale * 100.0).round() as u32).clamp(10, 400);
        }
        if self.preview_tool == PreviewTool::Hand {
            if canvas.image_response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if canvas.image_response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
        }
        self.handle_preview_zoom(ui.ctx(), &canvas.viewport_response, canvas.scale);
        self.handle_canvas_interaction(
            ui.ctx(),
            &canvas.image_response,
            canvas.image_rect,
            canvas.scale,
        );
        self.paint_drawing_preview(ui, canvas.image_rect, canvas.scale);
        self.paint_selection(ui, canvas.image_rect, canvas.scale, tokens.border_info);
        self.paint_selected_layer(ui, canvas.image_rect, canvas.scale, tokens.border_info);
    }

    fn handle_preview_zoom(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
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
        if self.fit_to_window {
            self.zoom = current_scale;
            self.fit_to_window = false;
        }
        self.zoom = (self.zoom * factor).clamp(0.1, 4.0);
        self.ui_state.zoom_percent = ((self.zoom * 100.0).round() as u32).clamp(10, 400);
        ctx.request_repaint();
    }

    fn handle_canvas_interaction(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        image_rect: Rect,
        scale: f32,
    ) {
        if self.preview_tool == PreviewTool::Hand {
            if response.dragged() {
                self.fit_to_window = false;
                self.preview_pan += response.drag_delta();
                ctx.request_repaint();
            }
            return;
        }

        if self.draw_tool.is_active() {
            if response.hovered() {
                ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
            }
            self.handle_drawing_interaction(ctx, response, image_rect, scale);
            return;
        }

        if response.drag_started()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let image_pos = screen_to_image(pointer, image_rect, scale);
            self.selected_layer = self.hit_test_layer(image_pos);
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
            && let Some(index) = self.selected_layer
        {
            let frame_delta = response.drag_delta() / scale.max(0.01);
            if frame_delta.length_sq() > 0.0 {
                self.move_layer_by(index, frame_delta);
                self.update_texture(ctx);
            }
        }

        if response.dragged()
            && self.selected_layer.is_none()
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
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let image_pos = screen_to_image(pointer, image_rect, scale);
            self.selected_layer = self.hit_test_layer(image_pos);
        }
    }

    fn handle_drawing_interaction(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        image_rect: Rect,
        scale: f32,
    ) {
        if self.working_image.is_none() {
            return;
        }

        if response.drag_started()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let image_pos = clamp_image_pos(
                screen_to_image(pointer, image_rect, scale),
                self.texture_size,
            );
            self.begin_drawing_stroke(ctx, image_pos);
        }

        if response.dragged()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let image_pos = clamp_image_pos(
                screen_to_image(pointer, image_rect, scale),
                self.texture_size,
            );
            self.continue_drawing_stroke(ctx, image_pos);
        }

        if response.drag_stopped() {
            let image_pos = response.interact_pointer_pos().map(|pointer| {
                clamp_image_pos(
                    screen_to_image(pointer, image_rect, scale),
                    self.texture_size,
                )
            });
            self.finish_drawing_stroke(ctx, image_pos);
        }

        if response.clicked()
            && !self.drawing_active
            && let Some(pointer) = response.interact_pointer_pos()
        {
            let image_pos = clamp_image_pos(
                screen_to_image(pointer, image_rect, scale),
                self.texture_size,
            );
            self.begin_drawing_stroke(ctx, image_pos);
            self.finish_drawing_stroke(ctx, Some(image_pos));
        }
    }

    fn begin_drawing_stroke(&mut self, ctx: &egui::Context, image_pos: Pos2) {
        if self.drawing_active || self.working_image.is_none() {
            return;
        }

        self.push_image_history();
        self.selected_layer = None;
        self.selection_drag_start = None;
        self.drawing_active = true;
        self.drawing_start = Some(image_pos);
        self.drawing_last = Some(image_pos);

        if self.draw_tool == DrawTool::Brush {
            self.draw_current_segment(ctx, image_pos, image_pos);
        } else {
            ctx.request_repaint();
        }
    }

    fn continue_drawing_stroke(&mut self, ctx: &egui::Context, image_pos: Pos2) {
        if !self.drawing_active {
            return;
        }

        match self.draw_tool {
            DrawTool::Brush => {
                if let Some(previous) = self.drawing_last {
                    self.draw_current_segment(ctx, previous, image_pos);
                }
                self.drawing_last = Some(image_pos);
            }
            DrawTool::Line => {
                self.drawing_last = Some(image_pos);
                ctx.request_repaint();
            }
            DrawTool::None => {}
        }
    }

    fn finish_drawing_stroke(&mut self, ctx: &egui::Context, image_pos: Option<Pos2>) {
        if !self.drawing_active {
            return;
        }

        let end = image_pos.or(self.drawing_last);
        if self.draw_tool == DrawTool::Line
            && let (Some(start), Some(end)) = (self.drawing_start, end)
        {
            self.draw_current_segment(ctx, start, end);
        }

        self.drawing_active = false;
        self.drawing_start = None;
        self.drawing_last = None;
        self.status = match self.language {
            Language::ZhTw => "已完成繪畫筆畫。".to_owned(),
            Language::En => "Drawing stroke applied.".to_owned(),
        };
        ctx.request_repaint();
    }

    fn draw_current_segment(&mut self, ctx: &egui::Context, start: Pos2, end: Pos2) {
        if let Some(image) = self.working_image.as_mut() {
            draw_stroke_segment(
                image,
                start,
                end,
                self.draw_color,
                self.draw_size,
                self.draw_opacity,
            );
            self.update_texture(ctx);
        }
    }

    fn paint_drawing_preview(&self, ui: &mut egui::Ui, image_rect: Rect, scale: f32) {
        if self.draw_tool != DrawTool::Line || !self.drawing_active {
            return;
        }
        let (Some(start), Some(end)) = (self.drawing_start, self.drawing_last) else {
            return;
        };

        let start = image_rect.min + start.to_vec2() * scale;
        let end = image_rect.min + end.to_vec2() * scale;
        let color = Color32::from_rgba_unmultiplied(
            self.draw_color.r(),
            self.draw_color.g(),
            self.draw_color.b(),
            ((self.draw_color.a() as f32) * self.draw_opacity.clamp(0.0, 1.0)) as u8,
        );
        ui.painter().line_segment(
            [start, end],
            Stroke::new((self.draw_size * scale).max(1.0), color),
        );
    }

    fn paint_selected_layer(
        &self,
        ui: &mut egui::Ui,
        image_rect: Rect,
        scale: f32,
        color: Color32,
    ) {
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
            Stroke::new(1.0, color),
            StrokeKind::Outside,
        );
    }

    fn paint_selection(&self, ui: &mut egui::Ui, image_rect: Rect, scale: f32, color: Color32) {
        let Some(bounds) = self.selection_rect else {
            return;
        };
        let min = image_rect.min + bounds.min.to_vec2() * scale;
        let max = image_rect.min + bounds.max.to_vec2() * scale;
        let rect = Rect::from_min_max(min, max);
        let stroke = Stroke::new(1.0, color);

        match self.selection_shape {
            SelectionShape::Rectangle => {
                canvas_view::paint_dashed_rect(ui, rect, color);
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
                self.reset_drawing_state();
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
        self.reset_drawing_state();
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
        self.reset_drawing_state();
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

        if output.extension().is_none()
            && let Some(extension) = source.extension()
        {
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

    fn reset_drawing_state(&mut self) {
        self.drawing_active = false;
        self.drawing_start = None;
        self.drawing_last = None;
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

        let (undo, redo, paste, delete, duplicate, left, right, up, down) = ctx.input(|input| {
            (
                input.modifiers.ctrl && input.key_pressed(egui::Key::Z),
                input.modifiers.ctrl && input.key_pressed(egui::Key::Y),
                input.modifiers.command && input.key_pressed(egui::Key::V),
                input.key_pressed(egui::Key::Delete),
                input.modifiers.ctrl && input.key_pressed(egui::Key::D),
                input.key_pressed(egui::Key::ArrowLeft),
                input.key_pressed(egui::Key::ArrowRight),
                input.key_pressed(egui::Key::ArrowUp),
                input.key_pressed(egui::Key::ArrowDown),
            )
        });

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
        self.reset_drawing_state();
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

fn draw_stroke_segment(
    image: &mut DynamicImage,
    start: Pos2,
    end: Pos2,
    color: Color32,
    brush_size: f32,
    opacity: f32,
) {
    let mut rgba = image.to_rgba8();
    let radius = (brush_size / 2.0).max(0.5);
    let delta = end - start;
    let length = delta.length();

    if length <= f32::EPSILON {
        draw_brush_stamp(&mut rgba, start, radius, color, opacity);
        *image = DynamicImage::ImageRgba8(rgba);
        return;
    }

    let steps = length.ceil().max(1.0) as u32;

    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let center = start + delta * t;
        draw_brush_stamp(&mut rgba, center, radius, color, opacity);
    }

    *image = DynamicImage::ImageRgba8(rgba);
}

fn draw_brush_stamp(
    image: &mut image::RgbaImage,
    center: Pos2,
    radius: f32,
    color: Color32,
    opacity: f32,
) {
    let width = image.width() as i32;
    let height = image.height() as i32;
    if width <= 0 || height <= 0 {
        return;
    }

    let min_x = ((center.x - radius - 1.0).floor() as i32).clamp(0, width - 1);
    let max_x = ((center.x + radius + 1.0).ceil() as i32).clamp(0, width - 1);
    let min_y = ((center.y - radius - 1.0).floor() as i32).clamp(0, height - 1);
    let max_y = ((center.y + radius + 1.0).ceil() as i32).clamp(0, height - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let pixel_center = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
            let distance = (pixel_center - center).length();
            let coverage = (radius + 0.5 - distance).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }

            let pixel = image.get_pixel_mut(x as u32, y as u32);
            blend_brush_pixel(pixel, color, opacity * coverage);
        }
    }
}

fn blend_brush_pixel(pixel: &mut image::Rgba<u8>, color: Color32, opacity: f32) {
    let source_alpha = (color.a() as f32 / 255.0) * opacity.clamp(0.0, 1.0);
    if source_alpha <= 0.0 {
        return;
    }

    let dest_alpha = pixel[3] as f32 / 255.0;
    let output_alpha = source_alpha + dest_alpha * (1.0 - source_alpha);
    if output_alpha <= f32::EPSILON {
        return;
    }

    let source_rgb = [color.r() as f32, color.g() as f32, color.b() as f32];
    for channel in 0..3 {
        let dest = pixel[channel] as f32;
        let mixed = (source_rgb[channel] * source_alpha + dest * dest_alpha * (1.0 - source_alpha))
            / output_alpha;
        pixel[channel] = mixed.round().clamp(0.0, 255.0) as u8;
    }
    pixel[3] = (output_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
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
            FontChoice { name, path }
        })
        .collect()
}

#[cfg(test)]
mod drawing_tests {
    use super::*;

    #[test]
    fn draw_stroke_segment_blends_a_brush_dot() {
        let mut image =
            DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(5, 5, image::Rgba([255; 4])));

        draw_stroke_segment(
            &mut image,
            Pos2::new(2.0, 2.0),
            Pos2::new(2.0, 2.0),
            Color32::BLACK,
            4.0,
            0.5,
        );

        let rgba = image.to_rgba8();
        let center = rgba.get_pixel(2, 2);
        assert!(center[0] < 255);
        assert!(center[0] > 100);
        assert_eq!(center[3], 255);
    }

    #[test]
    fn draw_stroke_segment_interpolates_a_continuous_line() {
        let mut image = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            8,
            8,
            image::Rgba([0, 0, 0, 0]),
        ));

        draw_stroke_segment(
            &mut image,
            Pos2::new(1.0, 1.0),
            Pos2::new(6.0, 1.0),
            Color32::from_rgb(255, 0, 0),
            2.0,
            1.0,
        );

        let rgba = image.to_rgba8();
        let touched = (1..=6).filter(|x| rgba.get_pixel(*x, 1)[3] > 0).count();
        assert!(touched >= 5);
    }
}
