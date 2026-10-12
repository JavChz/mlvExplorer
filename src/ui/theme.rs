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

// Backwards-compatible aliases (older call sites)
pub const ACCENT_BLUE: Color32 = ACCENT;
pub const ACCENT_GOLD: Color32 = RATING;

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
/// (in the spirit of DaVinci Resolve / Lightroom / Photoshop dark UI).
pub fn apply_theme(ctx: &egui::Context) {
    install_fonts(ctx);

    let mut visuals = Visuals::dark();
    // IMPORTANT: do NOT set `override_text_color`. It forces every text —
    // including TextEdit placeholder hints — to the same color, which made
    // placeholders indistinguishable from real values.
    visuals.override_text_color = None;
    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_PANEL;
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER_STRONG);
    visuals.window_rounding = Rounding::same(6.0);
    visuals.menu_rounding = Rounding::same(4.0);
    visuals.extreme_bg_color = BG_FIELD; // text edit wells, scroll gutters
    visuals.faint_bg_color = Color32::from_rgb(36, 36, 40); // striped rows
    visuals.code_bg_color = BG_FIELD;
    visuals.hyperlink_color = ACCENT_HOVER;
    visuals.warn_fg_color = WARNING;
    visuals.error_fg_color = FLAG_REJECTED;
    visuals.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 6.0),
        blur: 18.0,
        spread: 0.0,
        color: Color32::from_black_alpha(110),
    };
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 4.0),
        blur: 10.0,
        spread: 0.0,
        color: Color32::from_black_alpha(90),
    };

    let r = Rounding::same(ROUNDING);

    // Labels / static content
    visuals.widgets.noninteractive.bg_fill = BG_PANEL;
    // `weak_bg_fill` is what egui tints toward for "weak" (hint) text.
    visuals.widgets.noninteractive.weak_bg_fill = BG_FIELD;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.noninteractive.rounding = r;

    // Buttons / inputs at rest
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

    // Selected toggles / text selection / focused text edits
    visuals.selection.bg_fill = ACCENT_DIM;
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT_HOVER);
    visuals.text_cursor.stroke = Stroke::new(1.5_f32, ACCENT_HOVER);

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
    style.spacing.scroll = egui::style::ScrollStyle::thin();
    style.interaction.tooltip_delay = 0.35;

    ctx.set_style(style);
}
