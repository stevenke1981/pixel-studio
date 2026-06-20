use egui::{Align, Button, Layout, RichText, Slider, Ui, Vec2};

use crate::i18n::{Language, TextKey};
use crate::ui::UiAction;
use crate::ui::state::UiState;
use crate::ui::theme::UiTokens;

pub fn show_status_bar(
    ui: &mut Ui,
    tokens: &UiTokens,
    language: Language,
    dimensions: Option<(u32, u32)>,
    state: &mut UiState,
    actions: &mut Vec<UiAction>,
) {
    ui.horizontal_centered(|ui| {
        let (width, height) = dimensions.unwrap_or((0, 0));
        let dimensions = if width == 0 || height == 0 {
            "— × —".to_owned()
        } else {
            format!("{width} × {height}")
        };
        ui.label(
            RichText::new(format!(
                "{} px · {} · {}",
                dimensions,
                language.t(TextKey::ColorModeRgb),
                language.t(TextKey::BitDepth8)
            ))
            .size(11.0)
            .color(tokens.text_secondary),
        );

        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width().max(0.0), 20.0),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.label(
                    RichText::new(format!("{}%", state.zoom_percent))
                        .size(11.0)
                        .color(tokens.text_secondary),
                );
                if ui
                    .add_sized([22.0, 20.0], Button::new("+").stroke(egui::Stroke::NONE))
                    .on_hover_text(language.t(TextKey::ZoomIn))
                    .clicked()
                {
                    state.zoom_percent = (state.zoom_percent + 10).min(400);
                    actions.push(UiAction::SetZoom(state.zoom_percent));
                }
                let changed = ui
                    .add(
                        Slider::new(&mut state.zoom_percent, 10..=400)
                            .step_by(10.0)
                            .show_value(false),
                    )
                    .changed();
                if changed {
                    state.zoom_percent = ((state.zoom_percent + 5) / 10 * 10).clamp(10, 400);
                    actions.push(UiAction::SetZoom(state.zoom_percent));
                }
                if ui
                    .add_sized([22.0, 20.0], Button::new("−").stroke(egui::Stroke::NONE))
                    .on_hover_text(language.t(TextKey::ZoomOut))
                    .clicked()
                {
                    state.zoom_percent = state.zoom_percent.saturating_sub(10).max(10);
                    actions.push(UiAction::SetZoom(state.zoom_percent));
                }
            },
        );
    });
}
