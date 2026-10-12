//! Central design system: palette, typography, spacing and icon font setup.
//!
//! Every UI module must take its colors and metrics from here so the whole
//! application stays visually consistent (neutral graphite surfaces, a single
//! accent color, and semantic colors reserved for ratings / flags / warnings).

use egui::{Color32, FontFamily, FontId, Margin, Rounding, Stroke, Style, TextStyle, Visuals};

// ─── Surfaces (darkest → lightest) ──────────────────────────────────────────
/// Video viewer / letterbox background.
pub const BG_VIEWER: Color32 = Color32::from_rgb(12, 12, 13);
/// Text-field / input well background.
pub const BG_FIELD: Color32 = Color32::from_rgb(20, 20, 22);
/// Main workspace (grid canvas) background.
pub const BG_APP: Color32 = Color32::from_rgb(24, 24, 26);
/// Side panels, toolbars and status bar.
pub const BG_PANEL: Color32 = Color32::from_rgb(32, 32, 35);
/// Section header strips inside panels.
pub const BG_SECTION: Color32 = Color32::from_rgb(40, 40, 44);
/// Clip cards and buttons at rest.
pub const BG_CARD: Color32 = Color32::from_rgb(38, 38, 42);
pub const BG_CARD_HOVER: Color32 = Color32::from_rgb(48, 48, 53);
pub const BG_CARD_SELECTED: Color32 = Color32::from_rgb(42, 54, 72);
/// Thumbnail well (behind letterboxed thumbnails).
pub const BG_THUMB: Color32 = Color32::from_rgb(16, 16, 18);

// ─── Lines ──────────────────────────────────────────────────────────────────
pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(48, 48, 53);
pub const BORDER_STRONG: Color32 = Color32::from_rgb(66, 66, 72);
pub const BORDER_SELECTED: Color32 = ACCENT;

// ─── Text ───────────────────────────────────────────────────────────────────
/// Values, file names, user-entered content.
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(222, 222, 226);
/// Field labels, secondary metadata.
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(150, 150, 158);
/// Section titles, counts, de-emphasised info.
pub const TEXT_MUTED: Color32 = Color32::from_rgb(112, 112, 120);
/// Placeholder (hint) text inside empty input fields — clearly dimmer than
/// any real value so users can tell "empty" from "filled in" at a glance.
pub const TEXT_HINT: Color32 = Color32::from_rgb(88, 88, 96);
pub const TEXT_DISABLED: Color32 = Color32::from_rgb(80, 80, 86);

// ─── Accent & semantic colors ───────────────────────────────────────────────
/// The single interactive accent (selection, focus, active toggles, playhead progress).
pub const ACCENT: Color32 = Color32::from_rgb(64, 132, 220);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(88, 152, 236);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(36, 58, 92);
pub const RATING: Color32 = Color32::from_rgb(232, 180, 48);
pub const FLAG_PICKED: Color32 = Color32::from_rgb(82, 190, 120);
pub const FLAG_REJECTED: Color32 = Color32::from_rgb(226, 86, 86);
pub const WARNING: Color32 = Color32::from_rgb(230, 160, 60);
pub const DANGER_FILL: Color32 = Color32::from_rgb(170, 48, 48);

// ─── Theme-aware dynamic colors (Dark & Light mode) ─────────────────────────

/// Returns the primary text color for the active theme.
pub fn text_primary(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(222, 222, 226)
    } else {
        Color32::from_rgb(20, 22, 26)
    }
}

/// Returns the secondary text color for the active theme.
pub fn text_secondary(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(150, 150, 158)
    } else {
        Color32::from_rgb(75, 80, 92)
    }
}

/// Returns the muted caption / counter text color for the active theme.
pub fn text_muted(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(112, 112, 120)
    } else {
        Color32::from_rgb(105, 110, 122)
    }
}

/// Returns the hint / placeholder text color for the active theme.
pub fn text_hint(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(88, 88, 96)
    } else {
        Color32::from_rgb(135, 140, 152)
    }
}

/// Returns the disabled text color for the active theme.
pub fn text_disabled(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(80, 80, 86)
    } else {
        Color32::from_rgb(160, 165, 175)
    }
}

/// Card surface at rest (dark: dark gray, light: crisp pure white).
pub fn bg_card(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(38, 38, 42)
    } else {
        Color32::WHITE
    }
}

/// Card surface on hover.
pub fn bg_card_hover(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(48, 48, 53)
    } else {
        Color32::from_rgb(242, 245, 250)
    }
}

/// Card surface when selected.
pub fn bg_card_selected(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(42, 54, 72)
    } else {
        Color32::from_rgb(220, 234, 254)
    }
}

/// Section header strip background (dark: dark graphite, light: clean cool gray).
pub fn bg_section(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(40, 40, 44)
    } else {
        Color32::from_rgb(234, 237, 243)
    }
}

/// Input well / gutter / badge pill background.
pub fn bg_field(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(20, 20, 22)
    } else {
        Color32::from_rgb(243, 245, 248)
    }
}

/// Subtle hairline border.
pub fn border_subtle(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(48, 48, 53)
    } else {
        Color32::from_rgb(215, 218, 226)
    }
}

/// Emphasized border.
pub fn border_strong(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(66, 66, 72)
    } else {
        Color32::from_rgb(180, 185, 196)
    }
}

// ─── Metrics ────────────────────────────────────────────────────────────────
pub const ROUNDING: f32 = 3.0;
pub const CONTROL_HEIGHT: f32 = 22.0;
/// Fixed label column width used by every label/value row in the inspector.
pub const LABEL_WIDTH: f32 = 92.0;
pub const PANEL_PADDING: f32 = 10.0;
pub const TOOLBAR_HEIGHT: f32 = 34.0;

// ─── Type scale ─────────────────────────────────────────────────────────────
pub const FONT_BODY: f32 = 12.5;
pub const FONT_SMALL: f32 = 11.0;
pub const FONT_CAPTION: f32 = 10.5;
pub const FONT_TITLE: f32 = 14.0;

/// Named font family holding the *filled* Phosphor glyphs (same code points
/// as the regular set), used for lit stars, active flags etc.
pub const ICON_FILL_FAMILY: &str = "phosphor-fill";

pub fn icon_fill_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(ICON_FILL_FAMILY.into()))
}

/// Register the Phosphor icon font (regular glyphs fall back inside the normal
/// proportional/monospace families, filled glyphs get their own family).
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("phosphor".into(), egui_phosphor::Variant::Regular.font_data());
    fonts
        .font_data
        .insert(ICON_FILL_FAMILY.into(), egui_phosphor::Variant::Fill.font_data());

    if let Some(keys) = fonts.families.get_mut(&FontFamily::Proportional) {
        keys.insert(1.min(keys.len()), "phosphor".into());
    }
    if let Some(keys) = fonts.families.get_mut(&FontFamily::Monospace) {
        keys.push("phosphor".into());
    }
    let mut fill_keys = vec![ICON_FILL_FAMILY.to_string()];
    if let Some(prop) = fonts.families.get(&FontFamily::Proportional) {
        fill_keys.extend(prop.iter().cloned());
    }
    fonts
        .families
        .insert(FontFamily::Name(ICON_FILL_FAMILY.into()), fill_keys);

    ctx.set_fonts(fonts);
}

/// Configure egui style for a neutral, professional post-production look
/// Configure egui style for a neutral, professional post-production look
/// (supports Dark and Light modes).
pub fn apply_theme(ctx: &egui::Context, mode: crate::config::ThemeMode) {
    install_fonts(ctx);

    let is_dark = match mode {
        crate::config::ThemeMode::Dark => true,
        crate::config::ThemeMode::Light => false,
        crate::config::ThemeMode::System => true, // default to dark in creative media apps
    };

    let mut visuals = if is_dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.override_text_color = None;

    if is_dark {
        visuals.panel_fill = BG_PANEL;
        visuals.window_fill = BG_PANEL;
        visuals.window_stroke = Stroke::new(1.0_f32, BORDER_STRONG);
        visuals.extreme_bg_color = BG_FIELD; // text edit wells, scroll gutters
        visuals.faint_bg_color = Color32::from_rgb(36, 36, 40); // striped rows
        visuals.code_bg_color = BG_FIELD;
        visuals.hyperlink_color = ACCENT_HOVER;
        visuals.warn_fg_color = WARNING;
        visuals.error_fg_color = FLAG_REJECTED;

        // Interactive widgets at rest & hover
        let r = Rounding::same(ROUNDING);
        visuals.widgets.noninteractive.bg_fill = BG_PANEL;
        visuals.widgets.noninteractive.weak_bg_fill = BG_FIELD;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
        visuals.widgets.noninteractive.rounding = r;

        visuals.widgets.inactive.bg_fill = BG_CARD;
        visuals.widgets.inactive.weak_bg_fill = BG_CARD;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
        visuals.widgets.inactive.rounding = r;
        visuals.widgets.inactive.expansion = 0.0;

        visuals.widgets.hovered.bg_fill = BG_CARD_HOVER;
        visuals.widgets.hovered.weak_bg_fill = BG_CARD_HOVER;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, BORDER_STRONG);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
        visuals.widgets.hovered.rounding = r;
        visuals.widgets.hovered.expansion = 0.0;

        visuals.widgets.active.bg_fill = ACCENT_DIM;
        visuals.widgets.active.weak_bg_fill = ACCENT_DIM;
        visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
        visuals.widgets.active.rounding = r;
        visuals.widgets.active.expansion = 0.0;

        visuals.widgets.open.bg_fill = BG_CARD_HOVER;
        visuals.widgets.open.weak_bg_fill = BG_CARD_HOVER;
        visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, BORDER_STRONG);
        visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
        visuals.widgets.open.rounding = r;

        visuals.selection.bg_fill = ACCENT_DIM;
        visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT_HOVER);
        visuals.text_cursor.stroke = Stroke::new(1.5_f32, ACCENT_HOVER);
    } else {
        // High contrast light mode
        let bg_light_panel = Color32::from_rgb(238, 239, 242);
        let bg_light_card = Color32::from_rgb(255, 255, 255);
        let bg_light_field = Color32::from_rgb(245, 246, 248);
        let border_light = Color32::from_rgb(205, 208, 215);
        let text_light_primary = Color32::from_rgb(24, 25, 28);

        visuals.panel_fill = bg_light_panel;
        visuals.window_fill = bg_light_panel;
        visuals.window_stroke = Stroke::new(1.0_f32, border_light);
        visuals.extreme_bg_color = bg_light_field;
        visuals.faint_bg_color = Color32::from_rgb(240, 242, 246);
        visuals.code_bg_color = bg_light_field;
        visuals.hyperlink_color = ACCENT;
        visuals.warn_fg_color = WARNING;
        visuals.error_fg_color = FLAG_REJECTED;

        let r = Rounding::same(ROUNDING);
        visuals.widgets.noninteractive.bg_fill = bg_light_panel;
        visuals.widgets.noninteractive.weak_bg_fill = bg_light_field;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, border_light);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, text_light_primary);
        visuals.widgets.noninteractive.rounding = r;

        visuals.widgets.inactive.bg_fill = bg_light_card;
        visuals.widgets.inactive.weak_bg_fill = bg_light_card;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, border_light);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, text_light_primary);
        visuals.widgets.inactive.rounding = r;

        visuals.widgets.hovered.bg_fill = Color32::from_rgb(240, 242, 248);
        visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(240, 242, 248);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, text_light_primary);
        visuals.widgets.hovered.rounding = r;

        visuals.widgets.active.bg_fill = Color32::from_rgb(215, 230, 252);
        visuals.widgets.active.weak_bg_fill = Color32::from_rgb(215, 230, 252);
        visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, text_light_primary);
        visuals.widgets.active.rounding = r;

        visuals.widgets.open.bg_fill = Color32::from_rgb(240, 242, 248);
        visuals.widgets.open.weak_bg_fill = Color32::from_rgb(240, 242, 248);
        visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, border_light);
        visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, text_light_primary);
        visuals.widgets.open.rounding = r;

        visuals.selection.bg_fill = Color32::from_rgb(200, 222, 250);
        visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
        visuals.text_cursor.stroke = Stroke::new(1.5_f32, ACCENT);
    }

    visuals.window_rounding = Rounding::same(6.0);
    visuals.menu_rounding = Rounding::same(4.0);
    visuals.slider_trailing_fill = true;
    visuals.striped = false;
    visuals.indent_has_left_vline = false;

    let mut style = Style {
        visuals,
        ..Style::default()
    };

    style.text_styles = [
        (TextStyle::Small, FontId::proportional(FONT_CAPTION)),
        (TextStyle::Body, FontId::proportional(FONT_BODY)),
        (TextStyle::Button, FontId::proportional(FONT_BODY)),
        (TextStyle::Heading, FontId::proportional(16.0)),
        (TextStyle::Monospace, FontId::monospace(12.0)),
    ]
    .into();

    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.window_margin = Margin::same(14.0);
    style.spacing.menu_margin = Margin::same(4.0);
    style.spacing.button_padding = egui::vec2(8.0, 3.0);
    style.spacing.interact_size = egui::vec2(24.0, CONTROL_HEIGHT);
    style.spacing.combo_height = 300.0;
    style.spacing.slider_width = 110.0;
    style.spacing.icon_width = 13.0;
    style.spacing.icon_width_inner = 7.0;

    // Solid, clearly visible scrollbars with high contrast in both dark and light modes
    let mut scroll_style = egui::style::ScrollStyle::solid();
    scroll_style.bar_width = 8.0;
    scroll_style.bar_inner_margin = 2.0;
    scroll_style.floating = false;
    scroll_style.foreground_color = true; // Use text/foreground color for thumb so it contrasts against panel
    scroll_style.dormant_handle_opacity = if is_dark { 0.50 } else { 0.40 };
    scroll_style.active_handle_opacity = if is_dark { 0.85 } else { 0.75 };
    scroll_style.interact_handle_opacity = 1.0;
    scroll_style.dormant_background_opacity = if is_dark { 0.15 } else { 0.10 };
    scroll_style.active_background_opacity = if is_dark { 0.25 } else { 0.20 };
    scroll_style.interact_background_opacity = 0.35;
    style.spacing.scroll = scroll_style;

    style.interaction.tooltip_delay = 0.35;

    ctx.set_style(style);
}
