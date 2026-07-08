use egui::{ComboBox, DragValue, Ui};

use crate::i18n::{Language, TextKey};
use crate::ui::state::{BlendModeUi, ToolId, UiState};
use crate::ui::theme::UiTokens;
use crate::ui::{CommandAvailability, UiAction};

pub struct ContextualToolbar<'a> {
    tokens: &'a UiTokens,
    language: Language,
    availability: CommandAvailability,
}

impl<'a> ContextualToolbar<'a> {
    pub fn new(
        tokens: &'a UiTokens,
        language: Language,
        availability: CommandAvailability,
    ) -> Self {
        Self {
            tokens,
            language,
            availability,
        }
    }

    pub fn show(&self, ui: &mut Ui, state: &mut UiState, actions: &mut Vec<UiAction>) {
        ui.horizontal_centered(|ui| match state.active_tool {
            ToolId::Brush | ToolId::Line | ToolId::Eraser => self.show_brush_controls(ui, state),
            ToolId::Crop => self.show_crop_controls(ui, actions),
            ToolId::Text => self.show_text_controls(ui, actions),
            ToolId::Zoom => self.show_zoom_controls(ui, actions),
            ToolId::Move => self.hint(ui, TextKey::MoveToolHint),
            ToolId::Hand => self.hint(ui, TextKey::HandToolHint),
            ToolId::Lasso => self.hint(ui, TextKey::LassoToolHint),
            ToolId::Shape => self.hint(ui, TextKey::ShapeToolHint),
            ToolId::Eyedropper => self.hint(ui, TextKey::EyedropperToolHint),
            ToolId::Stamp => self.hint(ui, TextKey::StampToolHint),
        });
    }

    fn show_brush_controls(&self, ui: &mut Ui, state: &mut UiState) {
        ui.spacing_mut().item_spacing.x = 14.0;
        stepper(
            ui,
            &format!("{}:", self.language.t(TextKey::Size)),
            &mut state.brush_ui.size_px,
            1,
            200,
            "px",
        );
        stepper(
            ui,
            &format!("{}:", self.language.t(TextKey::Hardness)),
            &mut state.brush_ui.hardness,
            0,
            100,
            "%",
        );
        percent_editor(
            ui,
            &format!("{}:", self.language.t(TextKey::Opacity)),
            &mut state.brush_ui.opacity,
        );
        ui.label(format!("{}:", self.language.t(TextKey::BrushColor)));
        ui.color_edit_button_srgba(&mut state.brush_ui.color)
            .on_hover_text(self.language.t(TextKey::BrushColor));
        ui.label(format!("{}:", self.language.t(TextKey::BlendMode)));
        ComboBox::from_id_salt("brush_blend_mode")
            .selected_text(blend_label(self.language, state.brush_ui.blend_mode))
            .width(140.0)
            .show_ui(ui, |ui| {
                for mode in [
                    BlendModeUi::Normal,
                    BlendModeUi::Multiply,
                    BlendModeUi::Screen,
                ] {
                    ui.selectable_value(
                        &mut state.brush_ui.blend_mode,
                        mode,
                        blend_label(self.language, mode),
                    );
                }
            });
    }

    fn show_crop_controls(&self, ui: &mut Ui, actions: &mut Vec<UiAction>) {
        ui.label(self.language.t(TextKey::CropRectangle));
        if ui
            .add_enabled(
                self.availability.has_image,
                egui::Button::new(self.language.t(TextKey::ApplyCrop)),
            )
            .clicked()
        {
            actions.push(UiAction::ApplyCrop);
        }
        if ui
            .add_enabled(
                self.availability.has_selection,
                egui::Button::new(self.language.t(TextKey::ClearSelection)),
            )
            .clicked()
        {
            actions.push(UiAction::ClearSelection);
        }
    }

    fn show_text_controls(&self, ui: &mut Ui, actions: &mut Vec<UiAction>) {
        self.hint(ui, TextKey::TextToolHint);
        if ui
            .add_enabled(
                self.availability.has_image,
                egui::Button::new(self.language.t(TextKey::AddText)),
            )
            .clicked()
        {
            actions.push(UiAction::AddTextLayer);
        }
    }

    fn show_zoom_controls(&self, ui: &mut Ui, actions: &mut Vec<UiAction>) {
        if ui.button(self.language.t(TextKey::Fit)).clicked() {
            actions.push(UiAction::FitToWindow);
        }
        if ui.button("100%").clicked() {
            actions.push(UiAction::ActualSize);
        }
        self.hint(ui, TextKey::ZoomToolHint);
    }

    fn hint(&self, ui: &mut Ui, key: TextKey) {
        ui.colored_label(self.tokens.text_secondary, self.language.t(key));
    }
}

pub fn show_contextual_toolbar(
    ui: &mut Ui,
    tokens: &UiTokens,
    language: Language,
    state: &mut UiState,
    availability: CommandAvailability,
    actions: &mut Vec<UiAction>,
) {
    ContextualToolbar::new(tokens, language, availability).show(ui, state, actions);
}

fn stepper(ui: &mut Ui, label: &str, value: &mut u32, min: u32, max: u32, suffix: &str) {
    ui.label(label);
    if ui.small_button("-").clicked() {
        *value = value.saturating_sub(1).max(min);
    }
    ui.add(
        DragValue::new(value)
            .range(min..=max)
            .suffix(suffix)
            .speed(1.0),
    );
    if ui.small_button("+").clicked() {
        *value = (*value + 1).min(max);
    }
}

fn percent_editor(ui: &mut Ui, label: &str, value: &mut u32) {
    ui.label(label);
    ui.add(DragValue::new(value).range(0..=100).suffix("%").speed(1.0));
}

fn blend_label(language: Language, mode: BlendModeUi) -> &'static str {
    match mode {
        BlendModeUi::Normal => language.t(TextKey::Normal),
        BlendModeUi::Multiply => language.t(TextKey::Multiply),
        BlendModeUi::Screen => language.t(TextKey::Screen),
    }
}
