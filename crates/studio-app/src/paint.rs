//! Paint drawing state, tools, color palette, and inspector tab definitions
//! for "Pixel Paint & Viewer 像素畫家與圖片檢視器".

use egui::{Color32, Pos2};
use studio_core::SupportedFormat;

use crate::i18n::{Language, TextKey};

/// Main application modes for Pixel Paint & Viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    /// Image browsing, folder gallery, non-destructive viewing/panning/zooming.
    Viewer,
    /// Interactive pixel canvas drawing, brush tools, 30-preset color palette.
    #[default]
    Paint,
    /// Batch image conversion, resizing, watermark, and C2PA processing.
    Batch,
}

impl AppMode {
    pub fn icon(self) -> &'static str {
        match self {
            Self::Viewer => "🖼️",
            Self::Paint => "🎨",
            Self::Batch => "⚡",
        }
    }

    pub fn text_key(self) -> TextKey {
        match self {
            Self::Viewer => TextKey::ModeViewer,
            Self::Paint => TextKey::ModePaint,
            Self::Batch => TextKey::ModeBatch,
        }
    }

    pub fn tag_text_key(self) -> TextKey {
        match self {
            Self::Viewer => TextKey::ModeTagViewer,
            Self::Paint => TextKey::ModeTagPaint,
            Self::Batch => TextKey::ModeTagBatch,
        }
    }
}

/// 14 interactive tools for the MS Paint-style drawing toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaintTool {
    /// Rectangular marquee selection.
    Select,
    /// 1px aliased hard line.
    Pencil,
    /// Anti-aliased round circular brush with adjustable radius.
    #[default]
    Brush,
    /// Semi-transparent highlighter stroke (50% alpha).
    Highlighter,
    /// Random dispersed particle airbrush spray.
    Airbrush,
    /// 45-degree angled chisel nib calligraphy stroke.
    Calligraphy,
    /// Erases pixels to Color 2 (background color).
    Eraser,
    /// 4-connected BFS flood fill with tolerance.
    FloodFill,
    /// Samples color under cursor (Left click: Color 1, Right click: Color 2).
    ColorPicker,
    /// Text placement tool.
    Text,
    /// Pixelation mosaic tool.
    Mosaic,
    /// Draggable crop marquee tool.
    Crop,
    /// Viewport panning hand tool.
    Hand,
    /// Vector and raster shapes flyout.
    Shapes,
}

impl PaintTool {
    pub fn icon(self) -> &'static str {
        match self {
            Self::Select => "🔲",
            Self::Pencil => "✏️",
            Self::Brush => "🖌️",
            Self::Highlighter => "🖊️",
            Self::Airbrush => "💨",
            Self::Calligraphy => "✒️",
            Self::Eraser => "🧽",
            Self::FloodFill => "🪣",
            Self::ColorPicker => "💧",
            Self::Text => "🔤",
            Self::Mosaic => "▦",
            Self::Crop => "✂️",
            Self::Hand => "✋",
            Self::Shapes => "📐",
        }
    }

    pub fn text_key(self) -> TextKey {
        match self {
            Self::Select => TextKey::ToolSelect,
            Self::Pencil => TextKey::ToolPencil,
            Self::Brush => TextKey::ToolBrush,
            Self::Highlighter => TextKey::ToolHighlighter,
            Self::Airbrush => TextKey::ToolAirbrush,
            Self::Calligraphy => TextKey::ToolCalligraphy,
            Self::Eraser => TextKey::ToolEraser,
            Self::FloodFill => TextKey::ToolFloodFill,
            Self::ColorPicker => TextKey::ToolColorPicker,
            Self::Text => TextKey::ToolText,
            Self::Mosaic => TextKey::ToolMosaic,
            Self::Crop => TextKey::ToolCrop,
            Self::Hand => TextKey::ToolHand,
            Self::Shapes => TextKey::ToolShapes,
        }
    }

    pub fn label(self, lang: Language) -> &'static str {
        lang.t(self.text_key())
    }

    pub const ALL_TOOLS: [PaintTool; 14] = [
        PaintTool::Select,
        PaintTool::Pencil,
        PaintTool::Brush,
        PaintTool::Highlighter,
        PaintTool::Airbrush,
        PaintTool::Calligraphy,
        PaintTool::Eraser,
        PaintTool::FloodFill,
        PaintTool::ColorPicker,
        PaintTool::Text,
        PaintTool::Mosaic,
        PaintTool::Crop,
        PaintTool::Hand,
        PaintTool::Shapes,
    ];
}

/// Six right inspector tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InspectorTab {
    #[default]
    Properties,
    Layers,
    History,
    Filters,
    Info,
    C2pa,
}

impl InspectorTab {
    pub fn icon(self) -> &'static str {
        match self {
            Self::Properties => "⚙️",
            Self::Layers => "📚",
            Self::History => "⏱️",
            Self::Filters => "✨",
            Self::Info => "📊",
            Self::C2pa => "🛡️",
        }
    }

    pub fn text_key(self) -> TextKey {
        match self {
            Self::Properties => TextKey::TabProperties,
            Self::Layers => TextKey::TabLayers,
            Self::History => TextKey::TabHistory,
            Self::Filters => TextKey::TabFilters,
            Self::Info => TextKey::TabInfo,
            Self::C2pa => TextKey::TabC2pa,
        }
    }

    pub const ALL_TABS: [InspectorTab; 6] = [
        InspectorTab::Properties,
        InspectorTab::Layers,
        InspectorTab::History,
        InspectorTab::Filters,
        InspectorTab::Info,
        InspectorTab::C2pa,
    ];
}

/// Canvas boundary drag-resize handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragHandle {
    /// Horizontal resize (right center).
    Right,
    /// Vertical resize (bottom center).
    Bottom,
    /// Diagonal resize (bottom right corner).
    Corner,
}

/// 30-preset color palette swatches from AcmeUI / MS Paint specification.
pub const PRESET_PALETTE_30: [Color32; 30] = [
    Color32::from_rgb(0x00, 0x00, 0x00), // 1. Black
    Color32::from_rgb(0x40, 0x40, 0x40), // 2. Dark Gray
    Color32::from_rgb(0x80, 0x80, 0x80), // 3. Medium Gray
    Color32::from_rgb(0xC0, 0xC0, 0xC0), // 4. Light Gray
    Color32::from_rgb(0xFF, 0xFF, 0xFF), // 5. White
    Color32::from_rgb(0x80, 0x00, 0x00), // 6. Maroon
    Color32::from_rgb(0xE5, 0x39, 0x35), // 7. Bright Red
    Color32::from_rgb(0xFF, 0x57, 0x22), // 8. Coral Orange
    Color32::from_rgb(0xFF, 0xA5, 0x00), // 9. Orange
    Color32::from_rgb(0xFF, 0xB7, 0x4D), // 10. Peach
    Color32::from_rgb(0xFF, 0xD7, 0x00), // 11. Gold
    Color32::from_rgb(0xFF, 0xFF, 0x00), // 12. Yellow
    Color32::from_rgb(0xAE, 0xEA, 0x00), // 13. Lime Green
    Color32::from_rgb(0x4C, 0xAF, 0x50), // 14. Grass Green
    Color32::from_rgb(0x00, 0x80, 0x00), // 15. Dark Green
    Color32::from_rgb(0x00, 0x96, 0x88), // 16. Teal
    Color32::from_rgb(0x00, 0xBC, 0xD4), // 17. Cyan
    Color32::from_rgb(0x03, 0xA9, 0xF4), // 18. Sky Blue
    Color32::from_rgb(0x21, 0x96, 0xF3), // 19. Dodger Blue
    Color32::from_rgb(0x19, 0x76, 0xD2), // 20. Royal Blue
    Color32::from_rgb(0x3F, 0x51, 0xB5), // 21. Indigo
    Color32::from_rgb(0x67, 0x3A, 0xB7), // 22. Deep Purple
    Color32::from_rgb(0x9C, 0x27, 0xB0), // 23. Purple
    Color32::from_rgb(0xE9, 0x1E, 0x63), // 24. Magenta / Pink
    Color32::from_rgb(0x79, 0x55, 0x48), // 25. Brown
    Color32::from_rgb(0xA1, 0x88, 0x7F), // 26. Tan
    Color32::from_rgb(0x80, 0xCB, 0xC4), // 27. Mint
    Color32::from_rgb(0xB3, 0x9D, 0xDB), // 28. Lavender
    Color32::from_rgb(0xF4, 0x8F, 0xB1), // 29. Rose Pink
    Color32::from_rgb(0x37, 0x47, 0x4F), // 30. Slate Gray
];

/// Helper to convert `egui::Color32` to `image::Rgba<u8>`.
#[inline]
pub fn color32_to_rgba(c: Color32) -> image::Rgba<u8> {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    image::Rgba([r, g, b, a])
}

/// Helper to convert `image::Rgba<u8>` to `egui::Color32`.
#[inline]
pub fn rgba_to_color32(r: image::Rgba<u8>) -> Color32 {
    Color32::from_rgba_unmultiplied(r[0], r[1], r[2], r[3])
}

/// Paint-mode state container embedded in `PixelStudioApp`.
#[derive(Debug, Clone)]
pub struct PaintState {
    /// Active mode: Viewer, Paint, or Batch.
    pub active_mode: AppMode,
    /// Active paint tool (e.g. Brush, Pencil, Eraser).
    pub active_tool: PaintTool,
    /// Active tab in the right inspector panel.
    pub active_inspector_tab: InspectorTab,
    /// Brush stroke width in pixels (1, 2, 4, 8, 16, etc.; default 4.0).
    pub brush_stroke_width: f32,
    /// Primary/Foreground color (Color 1).
    pub color_1_foreground: Color32,
    /// Secondary/Background color (Color 2).
    pub color_2_background: Color32,
    /// Whether Color 1 is currently the active target for palette clicks.
    pub is_color_1_selected: bool,
    /// Currently dragged canvas handle, if any.
    pub canvas_drag_handle: Option<DragHandle>,
    /// Last mouse position in image coordinates for continuous stroke interpolation.
    pub last_mouse_image_pos: Option<Pos2>,
    /// Hovered pixel coordinate `(X, Y)` for real-time status bar display.
    pub hover_pixel_coord: Option<(i32, i32)>,
    /// Hovered pixel color under the cursor.
    pub hover_pixel_color: Option<image::Rgba<u8>>,
    /// Quick export format selected in Properties tab.
    pub quick_export_format: SupportedFormat,
    /// Custom color picker popup toggle.
    pub custom_color_picker_open: bool,
    /// Custom color picker temporary value.
    pub custom_color_temp: Color32,
    /// Original dimensions when drag-resizing starts.
    pub drag_resize_start_size: Option<(u32, u32)>,
    /// Screen drag start position when drag-resizing.
    pub drag_resize_start_pos: Option<Pos2>,
    /// Whether an active drawing stroke is currently ongoing.
    pub is_drawing_stroke: bool,
}

impl Default for PaintState {
    fn default() -> Self {
        Self {
            active_mode: AppMode::Paint,
            active_tool: PaintTool::Brush,
            active_inspector_tab: InspectorTab::Properties,
            brush_stroke_width: 4.0,
            color_1_foreground: Color32::from_rgb(0, 0, 0), // Black
            color_2_background: Color32::from_rgb(255, 255, 255), // White
            is_color_1_selected: true,
            canvas_drag_handle: None,
            last_mouse_image_pos: None,
            hover_pixel_coord: None,
            hover_pixel_color: None,
            quick_export_format: SupportedFormat::Png,
            custom_color_picker_open: false,
            custom_color_temp: Color32::from_rgb(0, 0, 0),
            drag_resize_start_size: None,
            drag_resize_start_pos: None,
            is_drawing_stroke: false,
        }
    }
}

impl PaintState {
    /// Return the currently active color (Foreground if slot 1, Background if slot 2).
    pub fn active_color(&self) -> Color32 {
        if self.is_color_1_selected {
            self.color_1_foreground
        } else {
            self.color_2_background
        }
    }

    /// Set the currently active color slot.
    pub fn set_active_color(&mut self, color: Color32) {
        if self.is_color_1_selected {
            self.color_1_foreground = color;
        } else {
            self.color_2_background = color;
        }
    }

    /// Return the brush radius (half the stroke width).
    pub fn brush_radius(&self) -> f32 {
        self.brush_stroke_width / 2.0
    }

    /// Set the brush stroke width from radius.
    pub fn set_brush_radius(&mut self, radius: f32) {
        self.brush_stroke_width = (radius * 2.0).clamp(1.0, 128.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_mode_defaults_and_keys() {
        assert_eq!(AppMode::default(), AppMode::Paint);
        assert_eq!(AppMode::Viewer.icon(), "🖼️");
        assert_eq!(AppMode::Paint.icon(), "🎨");
        assert_eq!(AppMode::Batch.icon(), "⚡");
        assert_eq!(AppMode::Paint.text_key(), TextKey::ModePaint);
        assert_eq!(AppMode::Paint.tag_text_key(), TextKey::ModeTagPaint);
    }

    #[test]
    fn test_paint_tool_14_variants() {
        assert_eq!(PaintTool::ALL_TOOLS.len(), 14);
        assert_eq!(PaintTool::default(), PaintTool::Brush);
        assert_eq!(PaintTool::Brush.icon(), "🖌️");
        assert_eq!(PaintTool::Pencil.icon(), "✏️");
        assert_eq!(PaintTool::Eraser.icon(), "🧽");
        assert_eq!(PaintTool::Brush.text_key(), TextKey::ToolBrush);
        assert_eq!(PaintTool::FloodFill.text_key(), TextKey::ToolFloodFill);
    }

    #[test]
    fn test_inspector_tabs_6_variants() {
        assert_eq!(InspectorTab::ALL_TABS.len(), 6);
        assert_eq!(InspectorTab::default(), InspectorTab::Properties);
        assert_eq!(InspectorTab::Properties.icon(), "⚙️");
        assert_eq!(InspectorTab::Properties.text_key(), TextKey::TabProperties);
        assert_eq!(InspectorTab::Layers.text_key(), TextKey::TabLayers);
    }

    #[test]
    fn test_preset_palette_30_exact_colors() {
        assert_eq!(PRESET_PALETTE_30.len(), 30);
        // Swatch 1: Black
        assert_eq!(PRESET_PALETTE_30[0], Color32::from_rgb(0x00, 0x00, 0x00));
        // Swatch 5: White
        assert_eq!(PRESET_PALETTE_30[4], Color32::from_rgb(0xFF, 0xFF, 0xFF));
        // Swatch 7: Bright Red
        assert_eq!(PRESET_PALETTE_30[6], Color32::from_rgb(0xE5, 0x39, 0x35));
        // Swatch 18: Sky Blue
        assert_eq!(PRESET_PALETTE_30[17], Color32::from_rgb(0x03, 0xA9, 0xF4));
        // Swatch 30: Slate Gray
        assert_eq!(PRESET_PALETTE_30[29], Color32::from_rgb(0x37, 0x47, 0x4F));
    }

    #[test]
    fn test_paint_state_defaults() {
        let state = PaintState::default();
        assert_eq!(state.active_mode, AppMode::Paint);
        assert_eq!(state.active_tool, PaintTool::Brush);
        assert_eq!(state.active_inspector_tab, InspectorTab::Properties);
        assert_eq!(state.brush_stroke_width, 4.0);
        assert_eq!(state.brush_radius(), 2.0);
        assert_eq!(state.color_1_foreground, Color32::from_rgb(0, 0, 0));
        assert_eq!(state.color_2_background, Color32::from_rgb(255, 255, 255));
        assert!(state.is_color_1_selected);
        assert_eq!(state.active_color(), Color32::from_rgb(0, 0, 0));
    }

    #[test]
    fn test_paint_state_active_color_switching() {
        let mut state = PaintState::default();
        state.set_active_color(Color32::from_rgb(255, 0, 0));
        assert_eq!(state.color_1_foreground, Color32::from_rgb(255, 0, 0));

        state.is_color_1_selected = false;
        assert_eq!(state.active_color(), Color32::from_rgb(255, 255, 255));
        state.set_active_color(Color32::from_rgb(0, 255, 0));
        assert_eq!(state.color_2_background, Color32::from_rgb(0, 255, 0));
    }

    #[test]
    fn test_color_conversions() {
        let c = Color32::from_rgba_unmultiplied(10, 20, 30, 240);
        let rgba = color32_to_rgba(c);
        assert_eq!(rgba, image::Rgba([10, 20, 30, 240]));
        let c2 = rgba_to_color32(rgba);
        assert_eq!(c, c2);
    }
}
