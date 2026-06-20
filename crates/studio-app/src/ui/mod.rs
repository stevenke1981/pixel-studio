pub mod canvas_view;
pub mod contextual_toolbar;
pub mod right_panel;
pub mod state;
pub mod status_bar;
pub mod theme;
pub mod tool_rail;
pub mod top_menu_bar;

use crate::i18n::Language;
use state::{AdjustUiValues, ToolId};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiAction {
    OpenImage,
    OpenFolder,
    PasteImage,
    OpenProject,
    SaveProject,
    SaveAs,
    ExportComposite,
    Reload,
    ClearCanvas,
    RestoreCanvas,
    Undo,
    Redo,
    ImageUndo,
    ImageRedo,
    RotateLeft,
    RotateRight,
    Rotate180,
    FlipHorizontal,
    FlipVertical,
    Grayscale,
    Invert,
    AutoContrast,
    ResetEdits,
    AddTextLayer,
    AddImageLayer,
    DuplicateLayer,
    DeleteLayer,
    ApplyCrop,
    ClearSelection,
    ApplyAdjustments(AdjustUiValues),
    SelectTool(ToolId),
    SelectLayer(usize),
    ToggleLayerVisibility(usize, bool),
    SetLayerOpacity(usize, u32),
    SetLanguage(Language),
    FitToWindow,
    ActualSize,
    SetZoom(u32),
    ToggleLegacyTools,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CommandAvailability {
    pub has_image: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub can_undo_image: bool,
    pub can_redo_image: bool,
    pub has_selection: bool,
    pub has_selected_layer: bool,
    pub can_restore_canvas: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKindUi {
    Text,
    Image,
}

#[derive(Debug, Clone)]
pub struct LayerView {
    pub index: usize,
    pub name: String,
    pub visible: bool,
    pub selected: bool,
    pub opacity_percent: u32,
    pub kind: LayerKindUi,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HistoryView {
    pub image_undo_count: usize,
    pub layer_undo_count: usize,
}
