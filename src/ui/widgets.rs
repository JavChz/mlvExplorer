//! Reusable, consistently-styled building blocks shared by every panel.
//!
//! Keeping all row heights, label widths, section headers, icon buttons and
//! rating controls here guarantees that the grid, inspector and viewer look
//! and behave identically.

use crate::ui::theme::*;
use egui::{
    Align, Align2, Color32, FontId, Layout, Pos2, Rect, Response, RichText, Rounding, Sense,
    Stroke, Ui, Vec2, WidgetText,
};
pub use egui_phosphor::regular as icon;

// ─── Text helpers ───────────────────────────────────────────────────────────

/// Placeholder text for `TextEdit::hint_text` — always visibly dimmer than real values.
pub fn hint(text: &str) -> RichText {
    RichText::new(text).color(TEXT_HINT)
}

/// `"<icon>  <label>"` with consistent icon/label spacing.
pub fn icon_label(glyph: &str, label: &str) -> String {
    format!("{glyph}  {label}")
}

/// UTF-8 safe elision of `text` so it fits into `max_width` with the given font.
pub fn elide(ui: &Ui, text: &str, font: &FontId, max_width: f32) -> String {
    let measure = |s: &str| {
        ui.fonts(|f| f.layout_no_wrap(s.to_owned(), font.clone(), Color32::WHITE).size().x)
    };
    if max_width <= 0.0 {
        return String::new();
    }
    if measure(text) <= max_width {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let candidate: String = chars[..mid].iter().collect::<String>() + "…";
        if measure(&candidate) <= max_width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    chars[..lo].iter().collect::<String>() + "…"
}

/// SMPTE-style non-drop timecode `HH:MM:SS:FF` for a frame index.
pub fn timecode(frame: usize, fps: f32) -> String {
    let base = fps.round().max(1.0) as usize;
    let total_secs = frame / base;
    let ff = frame % base;
    let hh = total_secs / 3600;
    let mm = (total_secs / 60) % 60;
    let ss = total_secs % 60;
    format!("{hh:02}:{mm:02}:{ss:02}:{ff:02}")
}

/// Human readable byte size (binary units, as shown by Resolve / Explorer).
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b >= KB * KB * KB {
        format!("{:.2} GB", b / (KB * KB * KB))
    } else if b >= KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else {
        format!("{:.0} KB", b / KB)
    }
}

/// Frame rate formatted the way NLEs display it (23.976, 24, 29.97, 50…).
pub fn format_fps(fps: f32) -> String {
    let s = format!("{fps:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{s} fps")
}

// ─── Sections ───────────────────────────────────────────────────────────────

/// Full-bleed collapsible section with a header strip, used in the inspector.
/// The body is padded uniformly so every section aligns to the same grid.
pub fn section<R>(
    ui: &mut Ui,
    title: &str,
    default_open: bool,
    trailing: Option<WidgetText>,
    add_body: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let id = ui.make_persistent_id(("inspector_section", title));
    let mut state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);

    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::click());
    if resp.clicked() {
        state.toggle(ui);
    }
    let painter = ui.painter();
    let bg = if resp.hovered() { BG_CARD_HOVER } else { BG_SECTION };
    painter.rect_filled(rect, Rounding::ZERO, bg);
    painter.line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0_f32, BORDER_SUBTLE),
    );
    let caret = if state.is_open() { icon::CARET_DOWN } else { icon::CARET_RIGHT };
    painter.text(
        Pos2::new(rect.min.x + PANEL_PADDING, rect.center().y),
        Align2::LEFT_CENTER,
        caret,
        FontId::proportional(11.0),
        TEXT_MUTED,
    );
    painter.text(
        Pos2::new(rect.min.x + PANEL_PADDING + 16.0, rect.center().y),
        Align2::LEFT_CENTER,
        title.to_uppercase(),
        FontId::proportional(FONT_CAPTION),
        TEXT_SECONDARY,
    );
    if let Some(t) = trailing {
        let galley = t.into_galley(ui, Some(egui::TextWrapMode::Truncate), rect.width() * 0.5, egui::TextStyle::Small);
        let pos = Pos2::new(
            rect.max.x - PANEL_PADDING - galley.size().x,
            rect.center().y - galley.size().y * 0.5,
        );
        ui.painter().galley(pos, galley, TEXT_SECONDARY);
    }

    let out = state.show_body_unindented(ui, |ui| {
        egui::Frame::none()
            .inner_margin(egui::Margin {
                left: PANEL_PADDING,
                right: PANEL_PADDING,
                top: 8.0,
                bottom: 10.0,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                add_body(ui)
            })
            .inner
    });
    state.store(ui.ctx());
    out.map(|r| r.inner)
}

/// Lightweight uppercase caption used to title lists in the navigator.
pub fn caption(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text.to_uppercase())
            .size(FONT_CAPTION)
            .color(TEXT_MUTED)
            .extra_letter_spacing(0.6),
    );
}

// ─── Label / value rows (fixed label column) ────────────────────────────────

/// A form row: fixed-width left label, content fills the rest.
pub fn field_row<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.set_min_height(CONTROL_HEIGHT);
        ui.allocate_ui_with_layout(
            Vec2::new(LABEL_WIDTH, CONTROL_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(LABEL_WIDTH);
                ui.add(egui::Label::new(RichText::new(label).color(TEXT_SECONDARY)).truncate());
            },
        );
        add(ui)
    })
    .inner
}

/// Read-only property row. Long values are truncated (full text on hover).
pub fn prop_row(ui: &mut Ui, label: &str, value: impl Into<WidgetText>) {
    field_row(ui, label, |ui| {
        ui.add(egui::Label::new(value).truncate());
    });
}

/// Full-width single-line text field with a properly dimmed placeholder.
pub fn text_field(ui: &mut Ui, value: &mut String, placeholder: &str) -> Response {
    ui.add_sized(
        [ui.available_width(), CONTROL_HEIGHT],
        egui::TextEdit::singleline(value)
            .hint_text(hint(placeholder))
            .margin(egui::vec2(6.0, 3.0)),
    )
}

/// Optional string field bound to an `Option<String>`; returns true when changed.
pub fn optional_text_field(ui: &mut Ui, value: &mut Option<String>, placeholder: &str) -> bool {
    let mut buf = value.clone().unwrap_or_default();
    let changed = text_field(ui, &mut buf, placeholder).changed();
    if changed {
        *value = if buf.trim().is_empty() { None } else { Some(buf) };
    }
    changed
}

// ─── Buttons ────────────────────────────────────────────────────────────────

/// Square, frameless icon button (toolbar style). Highlights when `active`.
pub fn icon_button(ui: &mut Ui, glyph: &str, active: bool, tooltip: &str) -> Response {
    icon_button_colored(ui, glyph, active, None, tooltip)
}

pub fn icon_button_colored(
    ui: &mut Ui,
    glyph: &str,
    active: bool,
    active_color: Option<Color32>,
    tooltip: &str,
) -> Response {
    let size = Vec2::splat(CONTROL_HEIGHT + 2.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let enabled = ui.is_enabled();
    let painter = ui.painter();
    if active && active_color.is_none() {
        painter.rect_filled(rect, Rounding::same(ROUNDING), ACCENT_DIM);
    }
    if resp.hovered() && enabled {
        painter.rect_filled(rect, Rounding::same(ROUNDING), Color32::from_white_alpha(14));
    }
    let col = if !enabled {
        TEXT_DISABLED
    } else if active {
        active_color.unwrap_or(Color32::WHITE)
    } else if resp.hovered() {
        Color32::WHITE
    } else {
        TEXT_SECONDARY
    };
    let font = if active && active_color.is_some() {
        icon_fill_font(15.0)
    } else {
        FontId::proportional(15.0)
    };
    painter.text(rect.center(), Align2::CENTER_CENTER, glyph, font, col);
    if tooltip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tooltip)
    }
}

/// Text toggle button (segmented-control look) — accent fill when selected.
pub fn toggle_button(ui: &mut Ui, selected: bool, text: impl Into<String>) -> Response {
    let text: String = text.into();
    let rich = RichText::new(text).color(if selected { Color32::WHITE } else { TEXT_SECONDARY });
    let mut btn = egui::Button::new(rich).min_size(Vec2::new(0.0, CONTROL_HEIGHT));
    if selected {
        btn = btn.fill(ACCENT_DIM).stroke(Stroke::new(1.0_f32, ACCENT));
    } else {
        btn = btn.fill(Color32::TRANSPARENT).stroke(Stroke::NONE);
    }
    ui.add(btn)
}

/// Primary call-to-action button (accent fill).
pub fn primary_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text.into()).color(Color32::WHITE))
            .fill(ACCENT)
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(0.0, CONTROL_HEIGHT)),
    )
}

/// Standard secondary button with consistent height.
pub fn button(ui: &mut Ui, text: impl Into<String>) -> Response {
    ui.add(egui::Button::new(text.into()).min_size(Vec2::new(0.0, CONTROL_HEIGHT)))
}

// ─── Rating & flags ─────────────────────────────────────────────────────────

/// Interactive 5-star rating control. Clicking the current rating clears it.
/// Returns the new rating when changed.
pub fn rating_stars(ui: &mut Ui, rating: u8, size: f32) -> Option<u8> {
    let mut result = None;
    let spacing = 2.0;
    let total = Vec2::new(size * 5.0 + spacing * 4.0, size.max(CONTROL_HEIGHT));
    let (rect, _) = ui.allocate_exact_size(total, Sense::hover());

    // Determine hover preview
    let mut hover_star = 0u8;
    let star_rects: Vec<Rect> = (0..5)
        .map(|i| {
            Rect::from_min_size(
                Pos2::new(rect.min.x + i as f32 * (size + spacing), rect.center().y - size * 0.5),
                Vec2::splat(size),
            )
        })
        .collect();
    for (i, r) in star_rects.iter().enumerate() {
        let id = ui.id().with(("star", i, rect.min.x as i32, rect.min.y as i32));
        let resp = ui.interact(*r, id, Sense::click());
        if resp.hovered() {
            hover_star = i as u8 + 1;
        }
        if resp.clicked() {
            let s = i as u8 + 1;
            result = Some(if rating == s { 0 } else { s });
        }
    }

    let painter = ui.painter();
    for (i, r) in star_rects.iter().enumerate() {
        let s = i as u8 + 1;
        let lit = if hover_star > 0 { s <= hover_star } else { s <= rating };
        let (font, col) = if lit {
            let c = if hover_star > 0 { RATING.gamma_multiply(0.8) } else { RATING };
            (icon_fill_font(size), c)
        } else {
            (FontId::proportional(size), TEXT_MUTED)
        };
        painter.text(r.center(), Align2::CENTER_CENTER, icon::STAR, font, col);
    }
    if hover_star > 0 {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    result
}

/// Paints a read-only star row (used inside grid cards).
pub fn paint_stars(painter: &egui::Painter, left_center: Pos2, rating: u8, size: f32, spacing: f32) -> [Rect; 5] {
    let mut rects = [Rect::NOTHING; 5];
    for i in 0..5u8 {
        let c = Pos2::new(left_center.x + size * 0.5 + i as f32 * (size + spacing), left_center.y);
        rects[i as usize] = Rect::from_center_size(c, Vec2::splat(size + spacing));
        let lit = i < rating;
        let (font, col) = if lit {
            (icon_fill_font(size), RATING)
        } else {
            (FontId::proportional(size), Color32::from_gray(78))
        };
        painter.text(c, Align2::CENTER_CENTER, icon::STAR, font, col);
    }
    rects
}

/// Pick / Reject flag toggles (click an active flag again to unflag).
/// `compact` renders icon-only buttons. Returns the new flag when changed.
pub fn flag_buttons(ui: &mut Ui, flag: crate::db::PickFlag, compact: bool) -> Option<crate::db::PickFlag> {
    use crate::db::PickFlag;
    let mut out = None;
    let items = [
        (PickFlag::Picked, icon::FLAG, "Pick", FLAG_PICKED, "Flag as Pick [P]"),
        (PickFlag::Rejected, icon::X_CIRCLE, "Reject", FLAG_REJECTED, "Flag as Reject [X]"),
    ];
    for (value, glyph, label, color, tip) in items {
        let selected = flag == value;
        let resp = if compact {
            icon_button_colored(ui, glyph, selected, Some(color), tip)
        } else {
            let text_col = if selected { color } else { TEXT_SECONDARY };
            let mut job = egui::text::LayoutJob::default();
            let glyph_font = if selected { icon_fill_font(FONT_BODY + 1.0) } else { FontId::proportional(FONT_BODY + 1.0) };
            job.append(glyph, 0.0, egui::TextFormat::simple(glyph_font, text_col));
            job.append(label, 6.0, egui::TextFormat::simple(FontId::proportional(FONT_BODY), text_col));
            let mut btn = egui::Button::new(job).min_size(Vec2::new(78.0, CONTROL_HEIGHT));
            btn = if selected {
                btn.fill(color.gamma_multiply(0.16)).stroke(Stroke::new(1.0_f32, color.gamma_multiply(0.7)))
            } else {
                btn.fill(BG_CARD)
            };
            ui.add(btn).on_hover_text(tip)
        };
        if resp.clicked() {
            out = Some(if selected { PickFlag::Unflagged } else { value });
        }
    }
    out
}

// ─── Anamorphic desqueeze ───────────────────────────────────────────────────

pub const DESQUEEZE_OPTIONS: [(f32, &str); 6] = [
    (1.0, "None (1.0×)"),
    (1.333, "1.33×"),
    (1.5, "1.5×"),
    (1.75, "1.75×"),
    (2.0, "2.0×"),
    (3.0, "3.0× (1×3 binning)"),
];

pub fn desqueeze_label(factor: f32) -> String {
    DESQUEEZE_OPTIONS
        .iter()
        .find(|(f, _)| (factor - f).abs() < 0.03)
        .map(|(_, l)| l.to_string())
        .unwrap_or_else(|| format!("{factor:.2}×"))
}

/// Anamorphic desqueeze dropdown. Returns the new factor when changed.
pub fn desqueeze_combo(ui: &mut Ui, id_salt: impl std::hash::Hash, current: f32, width: f32) -> Option<f32> {
    let mut out = None;
    egui::ComboBox::from_id_salt(id_salt)
        .width(width)
        .selected_text(desqueeze_label(current))
        .show_ui(ui, |ui| {
            for (factor, label) in DESQUEEZE_OPTIONS {
                let sel = (current - factor).abs() < 0.03;
                if ui.selectable_label(sel, label).clicked() && !sel {
                    out = Some(factor);
                }
            }
        })
        .response
        .on_hover_text("Anamorphic desqueeze (horizontal pixel aspect ratio)");
    out
}

// ─── Painter helpers ────────────────────────────────────────────────────────

/// Small dark overlay badge for thumbnails. Returns the badge rect.
pub fn paint_badge(
    painter: &egui::Painter,
    anchor: Pos2,
    align: Align2,
    text: &str,
    font: FontId,
    text_color: Color32,
    fill: Color32,
) -> Rect {
    let galley = painter.layout_no_wrap(text.to_owned(), font, text_color);
    let size = galley.size() + Vec2::new(10.0, 4.0);
    let rect = align.anchor_size(anchor, size);
    painter.rect_filled(rect, Rounding::same(2.0), fill);
    painter.galley(rect.min + Vec2::new(5.0, 2.0), galley, text_color);
    rect
}

/// Keyboard key-cap label used in the shortcut reference.
pub fn keycap(ui: &mut Ui, key: &str) {
    egui::Frame::none()
        .fill(BG_FIELD)
        .stroke(Stroke::new(1.0_f32, BORDER_STRONG))
        .rounding(Rounding::same(3.0))
        .inner_margin(egui::Margin::symmetric(5.0, 1.0))
        .show(ui, |ui| {
            ui.label(RichText::new(key).monospace().size(FONT_CAPTION).color(TEXT_PRIMARY));
        });
}

/// Thin vertical divider for toolbars.
pub fn toolbar_divider(ui: &mut Ui) {
    ui.add_space(2.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 16.0), Sense::hover());
    ui.painter().rect_filled(rect, Rounding::ZERO, BORDER_STRONG);
    ui.add_space(2.0);
}

/// Standard panel frame helpers.
pub fn toolbar_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(BG_PANEL)
        .inner_margin(egui::Margin::symmetric(8.0, 5.0))
        .stroke(Stroke::NONE)
}
