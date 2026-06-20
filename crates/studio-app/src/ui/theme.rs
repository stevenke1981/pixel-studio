use egui::{Color32, Context, CornerRadius, Stroke, Vec2, epaint::Shadow};

#[derive(Debug, Clone, Copy)]
pub struct UiTokens {
    pub bg_primary: Color32,
    pub bg_secondary: Color32,
    pub bg_tertiary: Color32,
    pub bg_info: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_tertiary: Color32,
    pub text_info: Color32,
    pub border_tertiary: Color32,
    pub border_secondary: Color32,
    pub border_info: Color32,
    pub radius_md: u8,
    pub radius_lg: u8,
}

impl UiTokens {
    pub fn from_ctx(ctx: &Context) -> Self {
        if ctx.global_style().visuals.dark_mode {
            Self::dark()
        } else {
            Self::light()
        }
    }

    fn light() -> Self {
        Self {
            bg_primary: Color32::from_rgb(255, 255, 255),
            bg_secondary: Color32::from_rgb(245, 245, 243),
            bg_tertiary: Color32::from_rgb(236, 236, 234),
            bg_info: Color32::from_rgb(230, 241, 251),
            text_primary: Color32::from_rgb(26, 26, 26),
            text_secondary: Color32::from_rgb(107, 107, 102),
            text_tertiary: Color32::from_rgb(154, 152, 147),
            text_info: Color32::from_rgb(12, 68, 124),
            border_tertiary: Color32::from_black_alpha(20),
            border_secondary: Color32::from_black_alpha(38),
            border_info: Color32::from_rgb(55, 138, 221),
            radius_md: 8,
            radius_lg: 12,
        }
    }

    fn dark() -> Self {
        Self {
            bg_primary: Color32::from_rgb(28, 28, 26),
            bg_secondary: Color32::from_rgb(38, 38, 36),
            bg_tertiary: Color32::from_rgb(47, 47, 44),
            bg_info: Color32::from_rgb(4, 44, 83),
            text_primary: Color32::from_rgb(241, 239, 232),
            text_secondary: Color32::from_rgb(180, 178, 169),
            text_tertiary: Color32::from_rgb(136, 135, 128),
            text_info: Color32::from_rgb(133, 183, 235),
            border_tertiary: Color32::from_white_alpha(26),
            border_secondary: Color32::from_white_alpha(46),
            border_info: Color32::from_rgb(55, 138, 221),
            radius_md: 8,
            radius_lg: 12,
        }
    }
}

pub fn apply_visuals(ctx: &Context, tokens: &UiTokens) {
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = Vec2::new(6.0, 5.0);
    style.spacing.button_padding = Vec2::new(8.0, 4.0);
    style.visuals.panel_fill = tokens.bg_primary;
    style.visuals.window_fill = tokens.bg_primary;
    style.visuals.extreme_bg_color = tokens.bg_secondary;
    style.visuals.faint_bg_color = tokens.bg_secondary;
    style.visuals.window_stroke = Stroke::new(1.0, tokens.border_tertiary);
    style.visuals.window_corner_radius = CornerRadius::same(tokens.radius_lg);
    style.visuals.menu_corner_radius = CornerRadius::same(tokens.radius_md);
    style.visuals.window_shadow = Shadow::NONE;

    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = CornerRadius::same(tokens.radius_md);
    }
    style.visuals.widgets.noninteractive.fg_stroke.color = tokens.text_primary;
    style.visuals.widgets.inactive.fg_stroke.color = tokens.text_secondary;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, tokens.border_secondary);
    style.visuals.widgets.hovered.bg_fill = tokens.bg_secondary;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, tokens.border_secondary);
    style.visuals.widgets.hovered.fg_stroke.color = tokens.text_primary;
    style.visuals.widgets.active.bg_fill = tokens.bg_info;
    style.visuals.widgets.active.fg_stroke.color = tokens.text_info;
    style.visuals.selection.bg_fill = tokens.bg_info;
    style.visuals.selection.stroke = Stroke::new(1.0, tokens.border_info);
    ctx.set_global_style(style);
}

#[cfg(test)]
mod tests {
    use egui::{Context, Visuals};

    use super::UiTokens;

    #[test]
    fn tokens_follow_context_dark_mode() {
        let ctx = Context::default();
        ctx.set_visuals(Visuals::dark());
        let dark = UiTokens::from_ctx(&ctx);
        ctx.set_visuals(Visuals::light());
        let light = UiTokens::from_ctx(&ctx);
        assert_ne!(dark.bg_primary, light.bg_primary);
        assert_ne!(dark.text_primary, light.text_primary);
    }
}
