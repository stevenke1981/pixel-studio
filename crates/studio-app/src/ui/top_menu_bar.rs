use egui::{Align, Button, Layout, RichText, Ui, Vec2};

use crate::i18n::{Language, TextKey};
use crate::ui::theme::UiTokens;
use crate::ui::{CommandAvailability, UiAction};

pub fn show_top_menu_bar(
    ui: &mut Ui,
    tokens: &UiTokens,
    language: Language,
    availability: CommandAvailability,
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
                language.t(TextKey::LegacyTools),
                true,
                UiAction::ToggleLegacyTools,
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
                if ui
                    .add_enabled(
                        availability.has_image,
                        Button::new(language.t(TextKey::Export)),
                    )
                    .clicked()
                {
                    actions.push(UiAction::ExportComposite);
                }
                if ui
                    .add_enabled(
                        availability.can_redo || availability.can_redo_image,
                        Button::new("→"),
                    )
                    .on_hover_text(language.t(TextKey::Redo))
                    .clicked()
                {
                    actions.push(if availability.can_redo {
                        UiAction::Redo
                    } else {
                        UiAction::ImageRedo
                    });
                }
                if ui
                    .add_enabled(
                        availability.can_undo || availability.can_undo_image,
                        Button::new("←"),
                    )
                    .on_hover_text(language.t(TextKey::Undo))
                    .clicked()
                {
                    actions.push(if availability.can_undo {
                        UiAction::Undo
                    } else {
                        UiAction::ImageUndo
                    });
                }
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
