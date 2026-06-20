use egui::{Button, Context, RichText, Ui, Vec2};

use crate::i18n::{Language, TextKey};
use crate::ui::UiAction;
use crate::ui::state::{ToolId, UiState};
use crate::ui::theme::UiTokens;

pub fn show_tool_rail(
    ctx: &Context,
    ui: &mut Ui,
    tokens: &UiTokens,
    language: Language,
    state: &mut UiState,
    actions: &mut Vec<UiAction>,
) {
    if !ctx.egui_wants_keyboard_input() {
        for tool in ToolId::RAIL_TOOLS {
            if ctx.input(|input| {
                !input.modifiers.ctrl
                    && !input.modifiers.command
                    && !input.modifiers.alt
                    && input.key_pressed(tool.shortcut())
            }) {
                select_tool(state, tool, actions);
            }
        }
    }

    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        for tool in ToolId::RAIL_TOOLS {
            if tool == ToolId::Text {
                ui.add_space(3.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(24.0, 1.0), egui::Sense::hover());
                ui.painter().line_segment(
                    [rect.left_center(), rect.right_center()],
                    egui::Stroke::new(1.0, tokens.border_tertiary),
                );
                ui.add_space(3.0);
            }
            if tool == ToolId::Zoom {
                let remaining = ui.available_height() - 38.0;
                if remaining > 0.0 {
                    ui.add_space(remaining);
                }
            }

            let selected = state.active_tool == tool;
            let text = RichText::new(tool.icon()).size(18.0).color(if selected {
                tokens.text_info
            } else {
                tokens.text_secondary
            });
            let mut button = Button::new(text).min_size(Vec2::splat(34.0));
            if selected {
                button = button.fill(tokens.bg_info).stroke(egui::Stroke::NONE);
            } else {
                button = button
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE);
            }
            if ui
                .add(button)
                .on_hover_text(format!(
                    "{} ({})",
                    tool_label(language, tool),
                    tool.shortcut_name()
                ))
                .clicked()
            {
                select_tool(state, tool, actions);
            }
        }
    });
}

fn select_tool(state: &mut UiState, tool: ToolId, actions: &mut Vec<UiAction>) {
    if state.active_tool != tool {
        state.active_tool = tool;
        actions.push(UiAction::SelectTool(tool));
    }
}

pub fn tool_label(language: Language, tool: ToolId) -> &'static str {
    language.t(match tool {
        ToolId::Move => TextKey::Move,
        ToolId::Lasso => TextKey::Lasso,
        ToolId::Crop => TextKey::Crop,
        ToolId::Brush => TextKey::BrushTool,
        ToolId::Line => TextKey::LineTool,
        ToolId::Eraser => TextKey::Eraser,
        ToolId::Stamp => TextKey::Stamp,
        ToolId::Text => TextKey::TextTool,
        ToolId::Shape => TextKey::Shape,
        ToolId::Eyedropper => TextKey::Eyedropper,
        ToolId::Zoom => TextKey::Zoom,
    })
}
