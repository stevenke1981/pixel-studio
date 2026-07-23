use egui::{Color32, CornerRadius, Stroke, Vec2};

pub const BACKGROUND: Color32 = Color32::from_rgb(14, 16, 19);
pub const SURFACE: Color32 = Color32::from_rgb(23, 26, 31);
pub const SURFACE_ALT: Color32 = Color32::from_rgb(34, 38, 45);
pub const BORDER: Color32 = Color32::from_rgb(50, 55, 65);
pub const PRIMARY: Color32 = Color32::from_rgb(155, 135, 255);
pub const PRIMARY_DARK: Color32 = Color32::from_rgb(51, 43, 91);
pub const TEXT: Color32 = Color32::from_rgb(245, 246, 248);
pub const MUTED_TEXT: Color32 = Color32::from_rgb(169, 175, 184);

pub fn configure(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = Vec2::splat(8.0);
    style.spacing.button_padding = Vec2::new(12.0, 7.0);
    style.spacing.interact_size.y = 34.0;
    style.spacing.combo_width = 120.0;

    let visuals = &mut style.visuals;
    visuals.dark_mode = true;
    visuals.panel_fill = SURFACE;
    visuals.window_fill = SURFACE;
    visuals.extreme_bg_color = BACKGROUND;
    visuals.faint_bg_color = SURFACE_ALT;
    visuals.code_bg_color = SURFACE_ALT;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = PRIMARY_DARK;
    visuals.selection.stroke = Stroke::new(1.5_f32, PRIMARY);
    visuals.hyperlink_color = PRIMARY;
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.noninteractive.bg_fill = SURFACE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.inactive.bg_fill = SURFACE_ALT;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(45, 48, 65);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, PRIMARY);
    visuals.widgets.active.bg_fill = PRIMARY_DARK;
    visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, PRIMARY);
    visuals.widgets.open.bg_fill = PRIMARY_DARK;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, PRIMARY);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(5);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(5);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(5);
    visuals.widgets.active.corner_radius = CornerRadius::same(5);
    visuals.widgets.open.corner_radius = CornerRadius::same(5);

    ctx.set_global_style(style);
}
