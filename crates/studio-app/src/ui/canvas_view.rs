use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, TextureHandle, Ui, Vec2};

use crate::ui::theme::UiTokens;

pub struct CanvasResponse {
    pub viewport_response: Response,
    pub image_response: Response,
    pub image_rect: Rect,
    pub scale: f32,
}

pub struct CanvasInput<'a> {
    pub texture: Option<&'a TextureHandle>,
    pub texture_size: Vec2,
    pub fit_to_window: bool,
    pub zoom_percent: u32,
    pub pan: Vec2,
    pub pan_mode: bool,
    pub empty_hint: &'a str,
}

pub fn show_canvas_view(
    ui: &mut Ui,
    tokens: &UiTokens,
    input: CanvasInput<'_>,
) -> Option<CanvasResponse> {
    let available = ui.available_size().max(Vec2::splat(1.0));
    let (viewport_rect, viewport_response) = ui.allocate_exact_size(available, Sense::hover());
    ui.painter()
        .rect_filled(viewport_rect, 0.0, tokens.bg_tertiary);

    let Some(texture) = input.texture else {
        ui.painter().text(
            viewport_rect.center(),
            egui::Align2::CENTER_CENTER,
            input.empty_hint,
            egui::FontId::proportional(13.0),
            tokens.text_secondary,
        );
        return None;
    };

    let canvas_padding = Vec2::splat(24.0);
    let content_size = (available - canvas_padding * 2.0).max(Vec2::splat(1.0));
    let scale = if input.fit_to_window {
        (content_size.x / input.texture_size.x)
            .min(content_size.y / input.texture_size.y)
            .clamp(0.01, 1.0)
    } else {
        (input.zoom_percent as f32 / 100.0).clamp(0.1, 4.0)
    };
    let image_rect = Rect::from_center_size(
        viewport_rect.center() + input.pan,
        input.texture_size * scale,
    );
    let sense = if input.pan_mode {
        Sense::drag()
    } else {
        Sense::click_and_drag()
    };
    let image_response = ui.interact(
        if input.pan_mode {
            viewport_rect
        } else {
            image_rect
        },
        ui.id().with("canvas_image"),
        sense,
    );

    ui.painter()
        .rect_filled(image_rect.expand(1.0), 0.0, tokens.border_secondary);
    ui.painter().image(
        texture.id(),
        image_rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );

    let badge_text = format!("{}%", (scale * 100.0).round() as u32);
    let badge_rect = Rect::from_min_size(
        viewport_rect.right_bottom() - Vec2::new(72.0, 38.0),
        Vec2::new(56.0, 22.0),
    );
    ui.painter().rect(
        badge_rect,
        tokens.radius_md,
        tokens.bg_primary,
        Stroke::new(1.0, tokens.border_tertiary),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        badge_rect.center(),
        egui::Align2::CENTER_CENTER,
        badge_text,
        egui::FontId::proportional(11.0),
        tokens.text_secondary,
    );

    Some(CanvasResponse {
        viewport_response,
        image_response,
        image_rect,
        scale,
    })
}

pub fn paint_dashed_rect(ui: &Ui, rect: Rect, color: Color32) {
    let dash = 6.0;
    let gap = 4.0;
    paint_dashed_line(ui, rect.left_top(), rect.right_top(), dash, gap, color);
    paint_dashed_line(ui, rect.right_top(), rect.right_bottom(), dash, gap, color);
    paint_dashed_line(
        ui,
        rect.right_bottom(),
        rect.left_bottom(),
        dash,
        gap,
        color,
    );
    paint_dashed_line(ui, rect.left_bottom(), rect.left_top(), dash, gap, color);
}

fn paint_dashed_line(ui: &Ui, from: Pos2, to: Pos2, dash: f32, gap: f32, color: Color32) {
    let vector = to - from;
    let length = vector.length();
    if length <= f32::EPSILON {
        return;
    }
    let direction = vector / length;
    let mut offset = 0.0;
    while offset < length {
        let end = (offset + dash).min(length);
        ui.painter().line_segment(
            [from + direction * offset, from + direction * end],
            Stroke::new(1.0, color),
        );
        offset += dash + gap;
    }
}
