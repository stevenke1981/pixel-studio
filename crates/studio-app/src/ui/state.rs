use egui::Color32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolId {
    Move,
    Lasso,
    Crop,
    Brush,
    Line,
    Eraser,
    Stamp,
    Text,
    Shape,
    Eyedropper,
    Zoom,
}

impl ToolId {
    pub const RAIL_TOOLS: [Self; 11] = [
        Self::Move,
        Self::Lasso,
        Self::Crop,
        Self::Brush,
        Self::Line,
        Self::Eraser,
        Self::Stamp,
        Self::Text,
        Self::Shape,
        Self::Eyedropper,
        Self::Zoom,
    ];

    pub fn icon(self) -> &'static str {
        match self {
            Self::Move => "↖",
            Self::Lasso => "〰",
            Self::Crop => "C",
            Self::Brush => "B",
            Self::Line => "/",
            Self::Eraser => "▱",
            Self::Stamp => "▣",
            Self::Text => "T",
            Self::Shape => "□",
            Self::Eyedropper => "◉",
            Self::Zoom => "Z",
        }
    }

    pub fn shortcut(self) -> egui::Key {
        match self {
            Self::Move => egui::Key::V,
            Self::Lasso => egui::Key::L,
            Self::Crop => egui::Key::C,
            Self::Brush => egui::Key::B,
            Self::Line => egui::Key::N,
            Self::Eraser => egui::Key::E,
            Self::Stamp => egui::Key::S,
            Self::Text => egui::Key::T,
            Self::Shape => egui::Key::U,
            Self::Eyedropper => egui::Key::I,
            Self::Zoom => egui::Key::Z,
        }
    }

    pub fn shortcut_name(self) -> &'static str {
        match self {
            Self::Line => "N",
            Self::Move => "V",
            Self::Lasso => "L",
            Self::Crop => "C",
            Self::Brush => "B",
            Self::Eraser => "E",
            Self::Stamp => "S",
            Self::Text => "T",
            Self::Shape => "U",
            Self::Eyedropper => "I",
            Self::Zoom => "Z",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightPanelTab {
    Layers,
    Adjust,
    History,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftToolCategory {
    File,
    Browse,
    Drawing,
    Edit,
    Layers,
    Export,
    Gif,
    Batch,
    C2pa,
}

impl LeftToolCategory {
    pub const ALL: [Self; 9] = [
        Self::File,
        Self::Browse,
        Self::Drawing,
        Self::Edit,
        Self::Layers,
        Self::Export,
        Self::Gif,
        Self::Batch,
        Self::C2pa,
    ];

    pub fn icon(self) -> &'static str {
        match self {
            Self::File => "F",
            Self::Browse => "O",
            Self::Drawing => "B",
            Self::Edit => "E",
            Self::Layers => "L",
            Self::Export => "X",
            Self::Gif => "G",
            Self::Batch => "Q",
            Self::C2pa => "P",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BrushUiSettings {
    pub size_px: u32,
    pub hardness: u32,
    pub opacity: u32,
    pub blend_mode: BlendModeUi,
    pub color: Color32,
}

impl Default for BrushUiSettings {
    fn default() -> Self {
        Self {
            size_px: 60,
            hardness: 80,
            opacity: 100,
            blend_mode: BlendModeUi::Normal,
            color: Color32::BLACK,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendModeUi {
    Normal,
    Multiply,
    Screen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdjustUiValues {
    pub brightness: i32,
    pub contrast: i32,
    pub saturation: i32,
    pub temperature: i32,
}

impl Default for AdjustUiValues {
    fn default() -> Self {
        Self {
            brightness: 50,
            contrast: 50,
            saturation: 50,
            temperature: 50,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UiState {
    pub active_tool: ToolId,
    pub left_category: LeftToolCategory,
    pub left_tools_open: bool,
    pub right_panel_tab: RightPanelTab,
    pub brush_ui: BrushUiSettings,
    pub adjust_ui: AdjustUiValues,
    pub zoom_percent: u32,
    pub layer_opacity_edit: Option<(usize, u32)>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            active_tool: ToolId::Move,
            left_category: LeftToolCategory::Drawing,
            left_tools_open: true,
            right_panel_tab: RightPanelTab::Layers,
            brush_ui: BrushUiSettings::default(),
            adjust_ui: AdjustUiValues::default(),
            zoom_percent: 100,
            layer_opacity_edit: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BlendModeUi, LeftToolCategory, RightPanelTab, ToolId, UiState};

    #[test]
    fn ui_defaults_match_design_spec() {
        let state = UiState::default();
        assert_eq!(state.active_tool, ToolId::Move);
        assert_eq!(state.left_category, LeftToolCategory::Drawing);
        assert!(state.left_tools_open);
        assert_eq!(state.right_panel_tab, RightPanelTab::Layers);
        assert_eq!(state.brush_ui.size_px, 60);
        assert_eq!(state.brush_ui.hardness, 80);
        assert_eq!(state.brush_ui.opacity, 100);
        assert_eq!(state.brush_ui.blend_mode, BlendModeUi::Normal);
        assert_eq!(state.zoom_percent, 100);
    }
}
