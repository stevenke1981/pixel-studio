use egui::{Align, Button, Layout, RichText, Ui, Vec2};

use crate::i18n::{Language, TextKey};
use crate::ui::state::ToolId;
use crate::ui::theme::UiTokens;
use crate::ui::{CommandAvailability, UiAction};

pub fn show_top_menu_bar(
    ui: &mut Ui,
    tokens: &UiTokens,
    language: Language,
    availability: CommandAvailability,
    active_tool: ToolId,
    actions: &mut Vec<UiAction>,
) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        ui.label(
            RichText::new("▣  Pixel Studio")
                .size(14.0)
                .color(tokens.text_primary),
        );

        ui.menu_button(language.t(TextKey::File), |ui| {
            menu_action(
                ui,
                language.t(TextKey::OpenImage),
                true,
                UiAction::OpenImage,
                actions,
            );
            menu_action(
                ui,
                language.t(TextKey::OpenFolder),
                true,
                UiAction::OpenFolder,
                actions,
            );
            menu_action(
                ui,
                language.t(TextKey::PasteImage),
                true,
                UiAction::PasteImage,
                actions,
            );
            menu_action(
                ui,
                language.t(TextKey::Reload),
                availability.has_image,
                UiAction::Reload,
                actions,
            );
            ui.separator();
            menu_action(
                ui,
                language.t(TextKey::OpenProject),
                true,
                UiAction::OpenProject,
                actions,
            );
            menu_action(
                ui,
                language.t(TextKey::SaveProject),
                availability.has_image,
                UiAction::SaveProject,
                actions,
            );
            menu_action(
                ui,
                language.t(TextKey::SaveAsNewFile),
                availability.has_image,
                UiAction::SaveAs,
                actions,
            );
            menu_action(
                ui,
                language.t(TextKey::Export),
                availability.has_image,
                UiAction::ExportComposite,
                actions,
            );
        });

        ui.menu_button(language.t(TextKey::Edit), |ui| {
            menu_action(
                ui,
                language.t(TextKey::Undo),
                availability.can_undo,
                UiAction::Undo,
                actions,
            );
            menu_action(
                ui,
                language.t(TextKey::Redo),
                availability.can_redo,
                UiAction::Redo,
                actions,
            );
            menu_action(
                ui,
                language.t(TextKey::ImageUndo),
                availability.can_undo_image,
                UiAction::ImageUndo,
                actions,
            );
            menu_action(
                ui,
                language.t(TextKey::ImageRedo),
                availability.can_redo_image,
                UiAction::ImageRedo,
                actions,
            );
            ui.separator();
            menu_action(
                ui,
                language.t(TextKey::ClearCanvas),
                availability.has_image,
                UiAction::ClearCanvas,
                actions,
            );
            menu_action(
                ui,
                language.t(TextKey::RestoreCanvas),
                availability.can_restore_canvas,
                UiAction::RestoreCanvas,
                actions,
            );
        });

        ui.menu_button(language.t(TextKey::Image), |ui| {
            for (label, action) in [
                (language.t(TextKey::RotateLeft), UiAction::RotateLeft),
                (language.t(TextKey::RotateRight), UiAction::RotateRight),
                (language.t(TextKey::Rotate180), UiAction::Rotate180),
                (language.t(TextKey::FlipH), UiAction::FlipHorizontal),
                (language.t(TextKey::FlipV), UiAction::FlipVertical),
            ] {
                menu_action(ui, label, availability.has_image, action, actions);
            }
        });

        ui.menu_button(language.t(TextKey::Layers), |ui| {
            menu_action(
                ui,
                language.t(TextKey::AddText),
                availability.has_image,
                UiAction::AddTextLayer,
                actions,
            );
            menu_action(
                ui,
                language.t(TextKey::AddImage),
                availability.has_image,
                UiAction::AddImageLayer,
                actions,
            );
            menu_action(
                ui,
                language.t(TextKey::DuplicateLayer),
                availability.has_selected_layer,
                UiAction::DuplicateLayer,
                actions,
            );
            menu_action(
                ui,
                language.t(TextKey::DeleteLayer),
                availability.has_selected_layer,
                UiAction::DeleteLayer,
                actions,
            );
        });

        ui.menu_button(language.t(TextKey::Filters), |ui| {
            for (label, action) in [
                (language.t(TextKey::Grayscale), UiAction::Grayscale),
                (language.t(TextKey::Invert), UiAction::Invert),
                (language.t(TextKey::AutoContrast), UiAction::AutoContrast),
                (language.t(TextKey::ResetEdits), UiAction::ResetEdits),
            ] {
                menu_action(ui, label, availability.has_image, action, actions);
            }
        });

        ui.menu_button(language.t(TextKey::View), |ui| {
            menu_action(
                ui,
                language.t(TextKey::Fit),
                availability.has_image,
                UiAction::FitToWindow,
                actions,
            );
            menu_action(
                ui,
                "100%",
                availability.has_image,
                UiAction::ActualSize,
                actions,
            );
            ui.separator();
            menu_action(
                ui,
                language.t(TextKey::LeftTools),
                true,
                UiAction::ToggleLeftTools,
                actions,
            );
            ui.separator();
            for value in [Language::ZhTw, Language::En] {
                if ui
                    .selectable_label(language == value, value.name())
                    .clicked()
                {
                    actions.push(UiAction::SetLanguage(value));
                    ui.close();
                }
            }
        });

        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width().max(0.0), 24.0),
            Layout::right_to_left(Align::Center),
            |ui| {
                toolbar_action_button(
                    ui,
                    tokens,
                    "↥",
                    language.t(TextKey::Export),
                    availability.has_image,
                    UiAction::ExportComposite,
                    actions,
                );
                toolbar_action_button(
                    ui,
                    tokens,
                    "1:1",
                    "100%",
                    availability.has_image,
                    UiAction::ActualSize,
                    actions,
                );
                toolbar_action_button(
                    ui,
                    tokens,
                    "□",
                    language.t(TextKey::Fit),
                    availability.has_image,
                    UiAction::FitToWindow,
                    actions,
                );
                ui.separator();
                toolbar_tool_button(
                    ui,
                    tokens,
                    active_tool,
                    ToolId::Line,
                    language.t(TextKey::LineTool),
                    actions,
                );
                toolbar_tool_button(
                    ui,
                    tokens,
                    active_tool,
                    ToolId::Brush,
                    language.t(TextKey::BrushTool),
                    actions,
                );
                toolbar_tool_button(
                    ui,
                    tokens,
                    active_tool,
                    ToolId::Crop,
                    language.t(TextKey::Crop),
                    actions,
                );
                ui.separator();
                toolbar_redo_button(ui, tokens, language, availability, actions);
                toolbar_undo_button(ui, tokens, language, availability, actions);
                ui.separator();
                toolbar_action_button(
                    ui,
                    tokens,
                    "S+",
                    language.t(TextKey::SaveAsNewFile),
                    availability.has_image,
                    UiAction::SaveAs,
                    actions,
                );
                toolbar_action_button(
                    ui,
                    tokens,
                    "S",
                    language.t(TextKey::SaveProject),
                    availability.has_image,
                    UiAction::SaveProject,
                    actions,
                );
                toolbar_action_button(
                    ui,
                    tokens,
                    "⧉",
                    language.t(TextKey::PasteImage),
                    true,
                    UiAction::PasteImage,
                    actions,
                );
                toolbar_action_button(
                    ui,
                    tokens,
                    "O",
                    language.t(TextKey::OpenImage),
                    true,
                    UiAction::OpenImage,
                    actions,
                );
            },
        );
    });
}

fn menu_action(
    ui: &mut Ui,
    label: &str,
    enabled: bool,
    action: UiAction,
    actions: &mut Vec<UiAction>,
) {
    if ui.add_enabled(enabled, Button::new(label)).clicked() {
        actions.push(action);
        ui.close();
    }
}

fn toolbar_action_button(
    ui: &mut Ui,
    tokens: &UiTokens,
    icon: &str,
    tooltip: &str,
    enabled: bool,
    action: UiAction,
    actions: &mut Vec<UiAction>,
) {
    let width = match icon.chars().count() {
        0 | 1 => 28.0,
        2 => 34.0,
        _ => 40.0,
    };
    let text = RichText::new(icon).size(12.0).color(tokens.text_secondary);
    let button = Button::new(text)
        .min_size(Vec2::new(width, 24.0))
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::new(1.0, tokens.border_tertiary));
    if ui
        .add_enabled(enabled, button)
        .on_hover_text(tooltip)
        .clicked()
    {
        actions.push(action);
    }
}

fn toolbar_tool_button(
    ui: &mut Ui,
    tokens: &UiTokens,
    active_tool: ToolId,
    tool: ToolId,
    tooltip: &str,
    actions: &mut Vec<UiAction>,
) {
    let selected = active_tool == tool;
    let text = RichText::new(tool.icon()).size(12.0).color(if selected {
        tokens.text_info
    } else {
        tokens.text_secondary
    });
    let button = Button::new(text)
        .min_size(Vec2::new(28.0, 24.0))
        .fill(if selected {
            tokens.bg_info
        } else {
            egui::Color32::TRANSPARENT
        })
        .stroke(egui::Stroke::new(
            1.0,
            if selected {
                tokens.border_info
            } else {
                tokens.border_tertiary
            },
        ));
    if ui.add(button).on_hover_text(tooltip).clicked() {
        actions.push(UiAction::SelectTool(tool));
    }
}

fn toolbar_undo_button(
    ui: &mut Ui,
    tokens: &UiTokens,
    language: Language,
    availability: CommandAvailability,
    actions: &mut Vec<UiAction>,
) {
    let enabled = availability.can_undo || availability.can_undo_image;
    let action = if availability.can_undo {
        UiAction::Undo
    } else {
        UiAction::ImageUndo
    };
    toolbar_action_button(
        ui,
        tokens,
        "←",
        language.t(TextKey::Undo),
        enabled,
        action,
        actions,
    );
}

fn toolbar_redo_button(
    ui: &mut Ui,
    tokens: &UiTokens,
    language: Language,
    availability: CommandAvailability,
    actions: &mut Vec<UiAction>,
) {
    let enabled = availability.can_redo || availability.can_redo_image;
    let action = if availability.can_redo {
        UiAction::Redo
    } else {
        UiAction::ImageRedo
    };
    toolbar_action_button(
        ui,
        tokens,
        "→",
        language.t(TextKey::Redo),
        enabled,
        action,
        actions,
    );
}
