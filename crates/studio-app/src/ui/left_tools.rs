use egui::{Button, Context, Grid, RichText, Ui, Vec2};

use crate::i18n::{Language, TextKey};
use crate::ui::UiAction;
use crate::ui::state::{LeftToolCategory, ToolId, UiState};
use crate::ui::theme::UiTokens;

pub fn show_category_rail(
    ctx: &Context,
    ui: &mut Ui,
    tokens: &UiTokens,
    language: Language,
    state: &mut UiState,
    actions: &mut Vec<UiAction>,
) {
    process_tool_shortcuts(ctx, state, actions);

    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        for category in LeftToolCategory::ALL {
            let selected = state.left_category == category && state.left_tools_open;
            let text = RichText::new(category.icon())
                .size(15.0)
                .color(if selected {
                    tokens.text_info
                } else {
                    tokens.text_secondary
                });
            let button = Button::new(text)
                .min_size(Vec2::splat(34.0))
                .fill(if selected {
                    tokens.bg_info
                } else {
                    egui::Color32::TRANSPARENT
                })
                .stroke(egui::Stroke::NONE);
            if ui
                .add(button)
                .on_hover_text(category_label(language, category))
                .clicked()
            {
                state.left_category = category;
                state.left_tools_open = true;
            }
        }
    });
}

pub fn show_tool_picker(
    ui: &mut Ui,
    tokens: &UiTokens,
    language: Language,
    state: &mut UiState,
    actions: &mut Vec<UiAction>,
) {
    Grid::new("left_canvas_tools")
        .num_columns(2)
        .spacing([6.0, 6.0])
        .show(ui, |ui| {
            for (index, tool) in ToolId::RAIL_TOOLS.into_iter().enumerate() {
                let selected = state.active_tool == tool;
                let label = format!("{}  {}", tool.icon(), tool_label(language, tool));
                let button = Button::new(RichText::new(label).size(12.0).color(if selected {
                    tokens.text_info
                } else {
                    tokens.text_secondary
                }))
                .min_size(Vec2::new(126.0, 32.0))
                .fill(if selected {
                    tokens.bg_info
                } else {
                    egui::Color32::TRANSPARENT
                })
                .stroke(egui::Stroke::new(1.0, tokens.border_tertiary));
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
                if index % 2 == 1 {
                    ui.end_row();
                }
            }
        });
}

pub fn category_label(language: Language, category: LeftToolCategory) -> &'static str {
    language.t(match category {
        LeftToolCategory::File => TextKey::File,
        LeftToolCategory::Browse => TextKey::BrowseTools,
        LeftToolCategory::Drawing => TextKey::Drawing,
        LeftToolCategory::Edit => TextKey::Edit,
        LeftToolCategory::Layers => TextKey::Layers,
        LeftToolCategory::Export => TextKey::Export,
        LeftToolCategory::Gif => TextKey::GifMaker,
        LeftToolCategory::Batch => TextKey::Batch,
        LeftToolCategory::C2pa => TextKey::C2pa,
    })
}

fn process_tool_shortcuts(ctx: &Context, state: &mut UiState, actions: &mut Vec<UiAction>) {
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    for tool in ToolId::RAIL_TOOLS {
        if ctx.input(|input| {
            !input.modifiers.ctrl
                && !input.modifiers.command
                && !input.modifiers.alt
                && input.key_pressed(tool.shortcut())
        }) {
            state.left_category = LeftToolCategory::Drawing;
            state.left_tools_open = true;
            select_tool(state, tool, actions);
        }
    }
}

fn select_tool(state: &mut UiState, tool: ToolId, actions: &mut Vec<UiAction>) {
    if state.active_tool != tool {
        state.active_tool = tool;
        actions.push(UiAction::SelectTool(tool));
    }
}

fn tool_label(language: Language, tool: ToolId) -> &'static str {
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
