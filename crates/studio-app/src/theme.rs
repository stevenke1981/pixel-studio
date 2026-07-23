use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Vec2};

// AcmeUI focused-productivity light palette, mapped to egui semantic roles.
pub const BACKGROUND: Color32 = Color32::from_rgb(237, 241, 246);
pub const SURFACE: Color32 = Color32::from_rgb(255, 255, 255);
pub const SURFACE_ALT: Color32 = Color32::from_rgb(245, 247, 250);
pub const BORDER: Color32 = Color32::from_rgb(190, 200, 213);
pub const PRIMARY: Color32 = Color32::from_rgb(15, 108, 189);
pub const PRIMARY_DARK: Color32 = Color32::from_rgb(12, 86, 151);
pub const PRIMARY_TEXT: Color32 = Color32::WHITE;
pub const ACCENT: Color32 = Color32::from_rgb(220, 238, 255);
pub const TEXT: Color32 = Color32::from_rgb(18, 24, 32);
pub const MUTED_TEXT: Color32 = Color32::from_rgb(72, 83, 96);

pub fn configure(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = Vec2::splat(8.0);
    style.spacing.button_padding = Vec2::new(14.0, 8.0);
    style.spacing.interact_size.y = 38.0;
    style.spacing.combo_width = 132.0;
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(22.0, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(15.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(13.0, FontFamily::Proportional),
    );

    let visuals = &mut style.visuals;
    visuals.dark_mode = false;
    visuals.panel_fill = SURFACE;
    visuals.window_fill = SURFACE;
    visuals.extreme_bg_color = SURFACE;
    visuals.faint_bg_color = SURFACE_ALT;
    visuals.code_bg_color = SURFACE_ALT;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.5_f32, PRIMARY);
    visuals.hyperlink_color = PRIMARY;
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.disabled_alpha = 0.62;

    visuals.widgets.noninteractive.bg_fill = SURFACE;
    visuals.widgets.noninteractive.weak_bg_fill = SURFACE_ALT;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, MUTED_TEXT);

    visuals.widgets.inactive.bg_fill = SURFACE_ALT;
    visuals.widgets.inactive.weak_bg_fill = SURFACE_ALT;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);

    visuals.widgets.hovered.bg_fill = ACCENT;
    visuals.widgets.hovered.weak_bg_fill = ACCENT;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.5_f32, PRIMARY);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5_f32, PRIMARY_DARK);

    visuals.widgets.active.bg_fill = PRIMARY;
    visuals.widgets.active.weak_bg_fill = PRIMARY;
    visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, PRIMARY_DARK);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5_f32, PRIMARY_TEXT);

    visuals.widgets.open.bg_fill = ACCENT;
    visuals.widgets.open.weak_bg_fill = ACCENT;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, PRIMARY);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, PRIMARY_DARK);

    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(5);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(5);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(5);
    visuals.widgets.active.corner_radius = CornerRadius::same(5);
    visuals.widgets.open.corner_radius = CornerRadius::same(5);

    ctx.set_global_style(style);
}
