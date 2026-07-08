use egui::{Button, RichText, Ui, Vec2};

use crate::i18n::{Language, TextKey};
use crate::ui::state::ToolId;
use crate::ui::theme::UiTokens;
use crate::ui::tool_icons::{ToolIcon, ToolIconTextures};
use crate::ui::{CommandAvailability, UiAction};

pub struct TopChrome<'a> {
    tokens: &'a UiTokens,
    language: Language,
    availability: CommandAvailability,
    active_tool: ToolId,
}

impl<'a> TopChrome<'a> {
    pub fn new(
        tokens: &'a UiTokens,
        language: Language,
        availability: CommandAvailability,
        active_tool: ToolId,
    ) -> Self {
        Self {
            tokens,
            language,
            availability,
            active_tool,
        }
    }

    pub fn show_menu_bar(&self, ui: &mut Ui, actions: &mut Vec<UiAction>) {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            ui.label(
                RichText::new("▣  Pixel Studio")
                    .size(14.0)
                    .color(self.tokens.text_primary),
            );

            self.file_menu(ui, actions);
            self.edit_menu(ui, actions);
            self.image_menu(ui, actions);
            self.layers_menu(ui, actions);
            self.filters_menu(ui, actions);
            self.view_menu(ui, actions);
            ui.label(
                RichText::new(self.language.t(TextKey::Drawing))
                    .size(13.0)
                    .color(self.tokens.text_secondary),
            );
        });
    }

    pub fn show_common_toolbar(
        &self,
        ui: &mut Ui,
        icons: &mut ToolIconTextures,
        actions: &mut Vec<UiAction>,
    ) {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            self.action_cell(
                ui,
                icons,
                ToolbarAction::new(
                    ToolIcon::Open,
                    self.language.t(TextKey::Open),
                    self.language.t(TextKey::OpenImage),
                    true,
                    UiAction::OpenImage,
                ),
                actions,
            );
            self.action_cell(
                ui,
                icons,
                ToolbarAction::new(
                    ToolIcon::Paste,
                    self.language.t(TextKey::Paste),
                    self.language.t(TextKey::PasteImage),
                    true,
                    UiAction::PasteImage,
                ),
                actions,
            );
            group_separator(ui, self.tokens);
            self.action_cell(
                ui,
                icons,
                ToolbarAction::new(
                    ToolIcon::Save,
                    self.language.t(TextKey::Save),
                    self.language.t(TextKey::SaveProject),
                    self.availability.has_image,
                    UiAction::SaveProject,
                ),
                actions,
            );
            self.action_cell(
                ui,
                icons,
                ToolbarAction::new(
                    ToolIcon::SaveAs,
                    self.language.t(TextKey::SaveAsNewFile),
                    self.language.t(TextKey::SaveAsNewFile),
                    self.availability.has_image,
                    UiAction::SaveAs,
                ),
                actions,
            );
            group_separator(ui, self.tokens);
            self.action_cell(
                ui,
                icons,
                ToolbarAction::new(
                    ToolIcon::Undo,
                    self.language.t(TextKey::Undo),
                    self.language.t(TextKey::Undo),
                    self.can_undo(),
                    self.undo_action(),
                ),
                actions,
            );
            self.action_cell(
                ui,
                icons,
                ToolbarAction::new(
                    ToolIcon::Redo,
                    self.language.t(TextKey::Redo),
                    self.language.t(TextKey::Redo),
                    self.can_redo(),
                    self.redo_action(),
                ),
                actions,
            );
            group_separator(ui, self.tokens);
            self.tool_cell(
                ui,
                icons,
                ToolId::Crop,
                self.language.t(TextKey::Crop),
                actions,
            );
            self.tool_cell(
                ui,
                icons,
                ToolId::Brush,
                self.language.t(TextKey::BrushTool),
                actions,
            );
            self.tool_cell(
                ui,
                icons,
                ToolId::Line,
                self.language.t(TextKey::LineTool),
                actions,
            );
            group_separator(ui, self.tokens);
            self.action_cell(
                ui,
                icons,
                ToolbarAction::new(
                    ToolIcon::Fit,
                    self.language.t(TextKey::Fit),
                    self.language.t(TextKey::Fit),
                    self.availability.has_image,
                    UiAction::FitToWindow,
                ),
                actions,
            );
            self.action_cell(
                ui,
                icons,
                ToolbarAction::new(
                    ToolIcon::ActualSize,
                    "100%",
                    "100%",
                    self.availability.has_image,
                    UiAction::ActualSize,
                ),
                actions,
            );
            group_separator(ui, self.tokens);
            self.action_cell(
                ui,
                icons,
                ToolbarAction::new(
                    ToolIcon::Export,
                    self.language.t(TextKey::Export),
                    self.language.t(TextKey::Export),
                    self.availability.has_image,
                    UiAction::ExportComposite,
                ),
                actions,
            );
        });
    }

    fn file_menu(&self, ui: &mut Ui, actions: &mut Vec<UiAction>) {
        ui.menu_button(self.language.t(TextKey::File), |ui| {
            self.menu_action(
                ui,
                self.language.t(TextKey::OpenImage),
                true,
                UiAction::OpenImage,
                actions,
            );
            self.menu_action(
                ui,
                self.language.t(TextKey::OpenFolder),
                true,
                UiAction::OpenFolder,
                actions,
            );
            self.menu_action(
                ui,
                self.language.t(TextKey::PasteImage),
                true,
                UiAction::PasteImage,
                actions,
            );
            self.menu_action(
                ui,
                self.language.t(TextKey::Reload),
                self.availability.has_image,
                UiAction::Reload,
                actions,
            );
            ui.separator();
            self.menu_action(
                ui,
                self.language.t(TextKey::OpenProject),
                true,
                UiAction::OpenProject,
                actions,
            );
            self.menu_action(
                ui,
                self.language.t(TextKey::SaveProject),
                self.availability.has_image,
                UiAction::SaveProject,
                actions,
            );
            self.menu_action(
                ui,
                self.language.t(TextKey::SaveAsNewFile),
                self.availability.has_image,
                UiAction::SaveAs,
                actions,
            );
            self.menu_action(
                ui,
                self.language.t(TextKey::Export),
                self.availability.has_image,
                UiAction::ExportComposite,
                actions,
            );
        });
    }

    fn edit_menu(&self, ui: &mut Ui, actions: &mut Vec<UiAction>) {
        ui.menu_button(self.language.t(TextKey::Edit), |ui| {
            self.menu_action(
                ui,
                self.language.t(TextKey::Undo),
                self.availability.can_undo,
                UiAction::Undo,
                actions,
            );
            self.menu_action(
                ui,
                self.language.t(TextKey::Redo),
                self.availability.can_redo,
                UiAction::Redo,
                actions,
            );
            self.menu_action(
                ui,
                self.language.t(TextKey::ImageUndo),
                self.availability.can_undo_image,
                UiAction::ImageUndo,
                actions,
            );
            self.menu_action(
                ui,
                self.language.t(TextKey::ImageRedo),
                self.availability.can_redo_image,
                UiAction::ImageRedo,
                actions,
            );
            ui.separator();
            self.menu_action(
                ui,
                self.language.t(TextKey::ClearCanvas),
                self.availability.has_image,
                UiAction::ClearCanvas,
                actions,
            );
            self.menu_action(
                ui,
                self.language.t(TextKey::RestoreCanvas),
                self.availability.can_restore_canvas,
                UiAction::RestoreCanvas,
                actions,
            );
        });
    }

    fn image_menu(&self, ui: &mut Ui, actions: &mut Vec<UiAction>) {
        ui.menu_button(self.language.t(TextKey::Image), |ui| {
            for (label, action) in [
                (self.language.t(TextKey::RotateLeft), UiAction::RotateLeft),
                (self.language.t(TextKey::RotateRight), UiAction::RotateRight),
                (self.language.t(TextKey::Rotate180), UiAction::Rotate180),
                (self.language.t(TextKey::FlipH), UiAction::FlipHorizontal),
                (self.language.t(TextKey::FlipV), UiAction::FlipVertical),
            ] {
                self.menu_action(ui, label, self.availability.has_image, action, actions);
            }
        });
    }

    fn layers_menu(&self, ui: &mut Ui, actions: &mut Vec<UiAction>) {
        ui.menu_button(self.language.t(TextKey::Layers), |ui| {
            self.menu_action(
                ui,
                self.language.t(TextKey::AddText),
                self.availability.has_image,
                UiAction::AddTextLayer,
                actions,
            );
            self.menu_action(
                ui,
                self.language.t(TextKey::AddImage),
                self.availability.has_image,
                UiAction::AddImageLayer,
                actions,
            );
            self.menu_action(
                ui,
                self.language.t(TextKey::DuplicateLayer),
                self.availability.has_selected_layer,
                UiAction::DuplicateLayer,
                actions,
            );
            self.menu_action(
                ui,
                self.language.t(TextKey::DeleteLayer),
                self.availability.has_selected_layer,
                UiAction::DeleteLayer,
                actions,
            );
        });
    }

    fn filters_menu(&self, ui: &mut Ui, actions: &mut Vec<UiAction>) {
        ui.menu_button(self.language.t(TextKey::Filters), |ui| {
            for (label, action) in [
                (self.language.t(TextKey::Grayscale), UiAction::Grayscale),
                (self.language.t(TextKey::Invert), UiAction::Invert),
                (
                    self.language.t(TextKey::AutoContrast),
                    UiAction::AutoContrast,
                ),
                (self.language.t(TextKey::ResetEdits), UiAction::ResetEdits),
            ] {
                self.menu_action(ui, label, self.availability.has_image, action, actions);
            }
        });
    }

    fn view_menu(&self, ui: &mut Ui, actions: &mut Vec<UiAction>) {
        ui.menu_button(self.language.t(TextKey::View), |ui| {
            self.menu_action(
                ui,
                self.language.t(TextKey::Fit),
                self.availability.has_image,
                UiAction::FitToWindow,
                actions,
            );
            self.menu_action(
                ui,
                "100%",
                self.availability.has_image,
                UiAction::ActualSize,
                actions,
            );
            ui.separator();
            self.menu_action(
                ui,
                self.language.t(TextKey::LeftTools),
                true,
                UiAction::ToggleLeftTools,
                actions,
            );
            ui.separator();
            for value in [Language::ZhTw, Language::En] {
                if ui
                    .selectable_label(self.language == value, value.name())
                    .clicked()
                {
                    actions.push(UiAction::SetLanguage(value));
                    ui.close();
                }
            }
        });
    }

    fn menu_action(
        &self,
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

    fn action_cell(
        &self,
        ui: &mut Ui,
        icons: &mut ToolIconTextures,
        item: ToolbarAction<'_>,
        actions: &mut Vec<UiAction>,
    ) {
        let text_color = if item.enabled {
            self.tokens.text_primary
        } else {
            self.tokens.text_tertiary
        };
        let button = Button::new((
            icons.image(ui.ctx(), item.icon, 20.0),
            RichText::new(item.label).size(12.0).color(text_color),
        ))
        .min_size(Vec2::new(72.0, 58.0))
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE);
        if ui
            .add_enabled(item.enabled, button)
            .on_hover_text(item.tooltip)
            .clicked()
        {
            actions.push(item.action);
        }
    }

    fn tool_cell(
        &self,
        ui: &mut Ui,
        icons: &mut ToolIconTextures,
        tool: ToolId,
        label: &str,
        actions: &mut Vec<UiAction>,
    ) {
        let selected = self.active_tool == tool;
        let text_color = if selected {
            self.tokens.text_info
        } else {
            self.tokens.text_primary
        };
        let button = Button::new((
            icons.image(ui.ctx(), ToolIcon::from(tool), 20.0),
            RichText::new(label).size(12.0).color(text_color),
        ))
        .min_size(Vec2::new(72.0, 58.0))
        .fill(if selected {
            self.tokens.bg_info
        } else {
            egui::Color32::TRANSPARENT
        })
        .stroke(egui::Stroke::new(
            1.0,
            if selected {
                self.tokens.border_info
            } else {
                egui::Color32::TRANSPARENT
            },
        ));
        if ui.add(button).on_hover_text(label).clicked() {
            actions.push(UiAction::SelectTool(tool));
        }
    }

    fn can_undo(&self) -> bool {
        self.availability.can_undo || self.availability.can_undo_image
    }

    fn can_redo(&self) -> bool {
        self.availability.can_redo || self.availability.can_redo_image
    }

    fn undo_action(&self) -> UiAction {
        if self.availability.can_undo {
            UiAction::Undo
        } else {
            UiAction::ImageUndo
        }
    }

    fn redo_action(&self) -> UiAction {
        if self.availability.can_redo {
            UiAction::Redo
        } else {
            UiAction::ImageRedo
        }
    }
}

pub fn show_menu_bar(
    ui: &mut Ui,
    tokens: &UiTokens,
    language: Language,
    availability: CommandAvailability,
    active_tool: ToolId,
    actions: &mut Vec<UiAction>,
) {
    TopChrome::new(tokens, language, availability, active_tool).show_menu_bar(ui, actions);
}

pub fn show_common_toolbar(
    ui: &mut Ui,
    tokens: &UiTokens,
    icons: &mut ToolIconTextures,
    language: Language,
    availability: CommandAvailability,
    active_tool: ToolId,
    actions: &mut Vec<UiAction>,
) {
    TopChrome::new(tokens, language, availability, active_tool)
        .show_common_toolbar(ui, icons, actions);
}

#[derive(Clone, Copy)]
struct ToolbarAction<'a> {
    icon: ToolIcon,
    label: &'a str,
    tooltip: &'a str,
    enabled: bool,
    action: UiAction,
}

impl<'a> ToolbarAction<'a> {
    fn new(
        icon: ToolIcon,
        label: &'a str,
        tooltip: &'a str,
        enabled: bool,
        action: UiAction,
    ) -> Self {
        Self {
            icon,
            label,
            tooltip,
            enabled,
            action,
        }
    }
}

fn group_separator(ui: &mut Ui, tokens: &UiTokens) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 54.0), egui::Sense::hover());
    ui.painter().line_segment(
        [rect.center_top(), rect.center_bottom()],
        egui::Stroke::new(1.0, tokens.border_tertiary),
    );
}
