use egui::{Color32, Stroke, Style, Visuals};

pub const BG_APP: Color32 = Color32::from_rgb(18, 18, 20);
pub const BG_PANEL: Color32 = Color32::from_rgb(24, 24, 28);
pub const BG_CARD: Color32 = Color32::from_rgb(32, 32, 38);
pub const BG_CARD_HOVER: Color32 = Color32::from_rgb(42, 42, 50);
pub const BG_CARD_SELECTED: Color32 = Color32::from_rgb(38, 52, 74);
pub const BORDER_SELECTED: Color32 = Color32::from_rgb(59, 130, 246); // Lightroom/Resolve blue

pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(44, 44, 52);
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(244, 244, 245);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(161, 161, 170);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(113, 113, 122);

pub const ACCENT_BLUE: Color32 = Color32::from_rgb(59, 130, 246);
pub const ACCENT_GOLD: Color32 = Color32::from_rgb(234, 179, 8);
pub const FLAG_PICKED: Color32 = Color32::from_rgb(34, 197, 94);
pub const FLAG_REJECTED: Color32 = Color32::from_rgb(239, 68, 68);

/// Configure egui style for a sleek, modern photo/video studio dark appearance
pub fn apply_theme(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_PANEL;
    visuals.extreme_bg_color = BG_APP;
    visuals.faint_bg_color = BG_APP;

    // Subtle, clean widget styles
    visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_SECONDARY);

    visuals.widgets.inactive.bg_fill = BG_CARD;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.inactive.rounding = egui::Rounding::same(4.0);

    visuals.widgets.hovered.bg_fill = BG_CARD_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT_BLUE);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.hovered.rounding = egui::Rounding::same(4.0);

    visuals.widgets.active.bg_fill = BG_CARD_SELECTED;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT_BLUE);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.active.rounding = egui::Rounding::same(4.0);

    visuals.selection.bg_fill = Color32::from_rgb(30, 64, 120);
    visuals.selection.stroke = Stroke::new(1.5_f32, ACCENT_BLUE);

    let mut style = Style::default();
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(10.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);

    ctx.set_style(style);
}
