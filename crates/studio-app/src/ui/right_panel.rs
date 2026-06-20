use egui::{Align, Button, Layout, RichText, Slider, Ui, Vec2};

use crate::i18n::{Language, TextKey};
use crate::ui::state::{RightPanelTab, UiState};
use crate::ui::theme::UiTokens;
use crate::ui::{CommandAvailability, HistoryView, LayerKindUi, LayerView, UiAction};

pub struct RightPanelInput<'a> {
    pub tokens: &'a UiTokens,
    pub language: Language,
    pub availability: CommandAvailability,
    pub layers: &'a [LayerView],
    pub history: HistoryView,
}

pub fn show_right_panel(
    ui: &mut Ui,
    state: &mut UiState,
    input: RightPanelInput<'_>,
    actions: &mut Vec<UiAction>,
) {
    show_tabs(ui, input.tokens, input.language, &mut state.right_panel_tab);
    ui.add_space(6.0);
    match state.right_panel_tab {
        RightPanelTab::Layers => show_layers(
            ui,
            input.tokens,
            input.language,
            state,
            input.layers,
            actions,
        ),
        RightPanelTab::Adjust => show_adjustments(
            ui,
            input.language,
            state,
            input.availability.has_image,
            actions,
        ),
        RightPanelTab::History => show_history(ui, input.tokens, input.language, input.history),
    }
}

fn show_tabs(ui: &mut Ui, tokens: &UiTokens, language: Language, active: &mut RightPanelTab) {
    let tab_width = ui.available_width() / 3.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (tab, label) in [
            (RightPanelTab::Layers, language.t(TextKey::Layers)),
            (RightPanelTab::Adjust, language.t(TextKey::Adjust)),
            (RightPanelTab::History, language.t(TextKey::History)),
        ] {
            let selected = *active == tab;
            let response = ui.add_sized(
                [tab_width, 32.0],
                Button::new(RichText::new(label).size(12.0).color(if selected {
                    tokens.text_primary
                } else {
                    tokens.text_secondary
                }))
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE)
                .corner_radius(0),
            );
            if response.clicked() {
                *active = tab;
            }
            if selected {
                ui.painter().line_segment(
                    [response.rect.left_bottom(), response.rect.right_bottom()],
                    egui::Stroke::new(2.0, tokens.border_info),
                );
            }
        }
    });
}

fn show_layers(
    ui: &mut Ui,
    tokens: &UiTokens,
    language: Language,
    state: &mut UiState,
    layers: &[LayerView],
    actions: &mut Vec<UiAction>,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for layer in layers.iter().rev() {
                let fill = if layer.selected {
                    tokens.bg_info
                } else {
                    egui::Color32::TRANSPARENT
                };
                egui::Frame::new()
                    .fill(fill)
                    .corner_radius(tokens.radius_md)
                    .inner_margin(4.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .add_sized(
                                    [24.0, 26.0],
                                    Button::new(if layer.visible { "◉" } else { "○" })
                                        .stroke(egui::Stroke::NONE),
                                )
                                .on_hover_text(language.t(TextKey::Visible))
                                .clicked()
                            {
                                actions.push(UiAction::ToggleLayerVisibility(
                                    layer.index,
                                    !layer.visible,
                                ));
                            }

                            let (thumb, _) =
                                ui.allocate_exact_size(Vec2::splat(26.0), egui::Sense::hover());
                            let thumb_color = match layer.kind {
                                LayerKindUi::Text => egui::Color32::from_rgb(245, 196, 179),
                                LayerKindUi::Image => egui::Color32::from_rgb(159, 225, 203),
                            };
                            ui.painter().rect(
                                thumb,
                                4.0,
                                thumb_color,
                                egui::Stroke::new(1.0, tokens.border_tertiary),
                                egui::StrokeKind::Inside,
                            );

                            if ui
                                .add_sized(
                                    [ui.available_width().max(40.0), 26.0],
                                    Button::new(
                                        RichText::new(&layer.name)
                                            .size(12.0)
                                            .color(tokens.text_primary),
                                    )
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::NONE),
                                )
                                .clicked()
                            {
                                actions.push(UiAction::SelectLayer(layer.index));
                            }
                        });

                        if layer.selected {
                            let mut opacity = state
                                .layer_opacity_edit
                                .filter(|(index, _)| *index == layer.index)
                                .map(|(_, value)| value)
                                .unwrap_or(layer.opacity_percent);
                            ui.horizontal(|ui| {
                                ui.label(language.t(TextKey::Opacity));
                                let response =
                                    ui.add(Slider::new(&mut opacity, 0..=100).suffix("%"));
                                if response.changed() {
                                    state.layer_opacity_edit = Some((layer.index, opacity));
                                }
                                if response.drag_stopped()
                                    || (response.changed() && !response.dragged())
                                {
                                    actions.push(UiAction::SetLayerOpacity(layer.index, opacity));
                                    state.layer_opacity_edit = None;
                                }
                            });
                        }
                    });
                ui.add_space(3.0);
            }

            egui::Frame::new()
                .fill(tokens.bg_secondary)
                .corner_radius(tokens.radius_md)
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("▧").color(tokens.text_tertiary));
                        ui.label(language.t(TextKey::BackgroundLayer));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new("▣").color(tokens.text_tertiary));
                        });
                    });
                });
        });
}

fn show_adjustments(
    ui: &mut Ui,
    language: Language,
    state: &mut UiState,
    has_image: bool,
    actions: &mut Vec<UiAction>,
) {
    adjustment_slider(
        ui,
        language.t(TextKey::Brightness),
        &mut state.adjust_ui.brightness,
    );
    adjustment_slider(
        ui,
        language.t(TextKey::Contrast),
        &mut state.adjust_ui.contrast,
    );
    adjustment_slider(
        ui,
        language.t(TextKey::Saturation),
        &mut state.adjust_ui.saturation,
    );
    adjustment_slider(
        ui,
        language.t(TextKey::Temperature),
        &mut state.adjust_ui.temperature,
    );
    ui.add_space(6.0);
    if ui
        .add_enabled(
            has_image,
            Button::new(language.t(TextKey::ApplyAdjustments)),
        )
        .clicked()
    {
        actions.push(UiAction::ApplyAdjustments(state.adjust_ui));
    }
}

fn adjustment_slider(ui: &mut Ui, label: &str, value: &mut i32) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(value.to_string());
        });
    });
    ui.add(Slider::new(value, 0..=100).show_value(false));
    ui.add_space(6.0);
}

fn show_history(ui: &mut Ui, tokens: &UiTokens, language: Language, history: HistoryView) {
    ui.label(language.t(TextKey::OpenImage));
    if history.image_undo_count > 0 {
        ui.label(format!(
            "{} × {}",
            language.t(TextKey::ImageEdits),
            history.image_undo_count
        ));
    }
    if history.layer_undo_count > 0 {
        ui.label(format!(
            "{} × {}",
            language.t(TextKey::LayerEdits),
            history.layer_undo_count
        ));
    }
    ui.label(
        RichText::new(language.t(TextKey::CurrentState))
            .size(12.0)
            .color(tokens.text_info),
    );
}
