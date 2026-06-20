use egui::{ComboBox, DragValue, Slider, Ui};

use crate::i18n::{Language, TextKey};
use crate::ui::state::{BlendModeUi, ToolId, UiState};
use crate::ui::theme::UiTokens;
use crate::ui::{CommandAvailability, UiAction};

pub fn show_contextual_toolbar(
    ui: &mut Ui,
    _tokens: &UiTokens,
    language: Language,
    state: &mut UiState,
    availability: CommandAvailability,
    actions: &mut Vec<UiAction>,
) {
    ui.horizontal_centered(|ui| match state.active_tool {
        ToolId::Brush | ToolId::Line | ToolId::Eraser => {
            compact_slider(
                ui,
                language.t(TextKey::Size),
                &mut state.brush_ui.size_px,
                1..=200,
                "px",
            );
            compact_slider(
                ui,
                language.t(TextKey::Hardness),
                &mut state.brush_ui.hardness,
                0..=100,
                "%",
            );
            compact_slider(
                ui,
                language.t(TextKey::Opacity),
                &mut state.brush_ui.opacity,
                0..=100,
                "%",
            );
            ui.color_edit_button_srgba(&mut state.brush_ui.color)
                .on_hover_text(language.t(TextKey::BrushColor));
            ui.label(language.t(TextKey::BlendMode));
            ComboBox::from_id_salt("brush_blend_mode")
                .selected_text(blend_label(language, state.brush_ui.blend_mode))
                .width(90.0)
                .show_ui(ui, |ui| {
                    for mode in [
                        BlendModeUi::Normal,
                        BlendModeUi::Multiply,
                        BlendModeUi::Screen,
                    ] {
                        ui.selectable_value(
                            &mut state.brush_ui.blend_mode,
                            mode,
                            blend_label(language, mode),
                        );
                    }
                });
        }
        ToolId::Crop => {
            ui.label(language.t(TextKey::CropRectangle));
            if ui
                .add_enabled(
                    availability.has_image,
                    egui::Button::new(language.t(TextKey::ApplyCrop)),
                )
                .clicked()
            {
                actions.push(UiAction::ApplyCrop);
            }
            if ui
                .add_enabled(
                    availability.has_selection,
                    egui::Button::new(language.t(TextKey::ClearSelection)),
                )
                .clicked()
            {
                actions.push(UiAction::ClearSelection);
            }
        }
        ToolId::Text => {
            ui.label(language.t(TextKey::TextToolHint));
            if ui
                .add_enabled(
                    availability.has_image,
                    egui::Button::new(language.t(TextKey::AddText)),
                )
                .clicked()
            {
                actions.push(UiAction::AddTextLayer);
            }
        }
        ToolId::Zoom => {
            if ui.button(language.t(TextKey::Fit)).clicked() {
                actions.push(UiAction::FitToWindow);
            }
            if ui.button("100%").clicked() {
                actions.push(UiAction::ActualSize);
            }
            ui.label(language.t(TextKey::ZoomToolHint));
        }
        ToolId::Move => {
            ui.label(language.t(TextKey::MoveToolHint));
        }
        ToolId::Lasso => {
            ui.label(language.t(TextKey::LassoToolHint));
        }
        ToolId::Shape => {
            ui.label(language.t(TextKey::ShapeToolHint));
        }
        ToolId::Eyedropper => {
            ui.label(language.t(TextKey::EyedropperToolHint));
        }
        ToolId::Stamp => {
            ui.label(language.t(TextKey::StampToolHint));
        }
    });
}

fn compact_slider(
    ui: &mut Ui,
    label: &str,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
    suffix: &str,
) {
    ui.label(label);
    ui.add(
        Slider::new(value, range)
            .show_value(false)
            .max_decimals(0)
            .min_decimals(0),
    )
    .on_hover_text(label);
    ui.add(
        DragValue::new(value)
            .range(0..=200)
            .suffix(suffix)
            .speed(1.0),
    );
}

fn blend_label(language: Language, mode: BlendModeUi) -> &'static str {
    match mode {
        BlendModeUi::Normal => language.t(TextKey::Normal),
        BlendModeUi::Multiply => language.t(TextKey::Multiply),
        BlendModeUi::Screen => language.t(TextKey::Screen),
    }
}
