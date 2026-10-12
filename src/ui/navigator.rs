use crate::db::{Facets, FilterCriteria, PickFlag};
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};

pub struct Navigator;

impl Navigator {
    pub fn show(
        ui: &mut Ui,
        filter: &mut FilterCriteria,
        facets: &Facets,
    ) -> bool {
        let mut changed = false;

        // Navigator title bar / header
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), TOOLBAR_HEIGHT), Sense::hover());
        let painter = ui.painter();
        painter.text(
            Pos2::new(rect.min.x + PANEL_PADDING, rect.center().y),
            Align2::LEFT_CENTER,
            "Library",
            FontId::proportional(FONT_BODY + 0.5),
            TEXT_PRIMARY,
        );
        painter.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0_f32, BORDER_SUBTLE));

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);

                // Section: SMART COLLECTIONS / CATALOG
                egui::Frame::none()
                    .inner_margin(egui::Margin::symmetric(PANEL_PADDING, 0.0))
                    .show(ui, |ui| {
                        widgets::caption(ui, "Catalog");
                        ui.add_space(4.0);

                        // All Clips
                        let is_all = filter.flag.is_none() && filter.camera.is_none() && filter.lens.is_none() && filter.tag.is_none();
                        if Self::nav_item(ui, is_all, icon::FOLDER, "All Clips", facets.total, None, None) {
                            filter.flag = None;
                            filter.camera = None;
                            filter.lens = None;
                            filter.tag = None;
                            changed = true;
                        }

                        // Flagged / Picks
                        let is_picked = filter.flag == Some(PickFlag::Picked);
                        if Self::nav_item(ui, is_picked, icon::FLAG, "Picks", facets.picked, Some(FLAG_PICKED), Some(icon_fill_font(13.0))) {
                            filter.flag = if is_picked { None } else { Some(PickFlag::Picked) };
                            changed = true;
                        }

                        // Rejected
                        let is_rejected = filter.flag == Some(PickFlag::Rejected);
                        if Self::nav_item(ui, is_rejected, icon::X_CIRCLE, "Rejected", facets.rejected, Some(FLAG_REJECTED), Some(icon_fill_font(13.0))) {
                            filter.flag = if is_rejected { None } else { Some(PickFlag::Rejected) };
                            changed = true;
                        }

                        // Unflagged
                        let is_unflagged = filter.flag == Some(PickFlag::Unflagged);
                        if Self::nav_item(ui, is_unflagged, icon::CIRCLE, "Unflagged", facets.unflagged, None, None) {
                            filter.flag = if is_unflagged { None } else { Some(PickFlag::Unflagged) };
                            changed = true;
                        }
                    });

                ui.add_space(8.0);

                // Section: CAMERAS
                let trailing_cam = (!facets.cameras.is_empty()).then(|| RichText::new(facets.cameras.len().to_string()).size(FONT_CAPTION).color(TEXT_MUTED).into());
                widgets::section(ui, "Cameras", true, trailing_cam, |ui| {
                    if facets.cameras.is_empty() {
                        ui.label(RichText::new("No cameras detected").size(FONT_SMALL).color(TEXT_HINT));
                    }
                    for cam in &facets.cameras {
                        let is_sel = filter.camera.as_ref() == Some(&cam.name);
                        if Self::nav_item(ui, is_sel, icon::CAMERA, &cam.name, cam.count, None, None) {
                            filter.camera = if is_sel { None } else { Some(cam.name.clone()) };
                            changed = true;
                        }
                    }
                });

                // Section: LENSES
                let trailing_lens = (!facets.lenses.is_empty()).then(|| RichText::new(facets.lenses.len().to_string()).size(FONT_CAPTION).color(TEXT_MUTED).into());
                widgets::section(ui, "Lenses", true, trailing_lens, |ui| {
                    if facets.lenses.is_empty() {
                        ui.label(RichText::new("No lenses detected").size(FONT_SMALL).color(TEXT_HINT));
                    }
                    for lens in &facets.lenses {
                        let is_sel = filter.lens.as_ref() == Some(&lens.name);
                        if Self::nav_item(ui, is_sel, icon::APERTURE, &lens.name, lens.count, None, None) {
                            filter.lens = if is_sel { None } else { Some(lens.name.clone()) };
                            changed = true;
                        }
                    }
                });

                // Section: KEYWORDS / TAGS
                let trailing_tags = (!facets.tags.is_empty()).then(|| RichText::new(facets.tags.len().to_string()).size(FONT_CAPTION).color(TEXT_MUTED).into());
                widgets::section(ui, "Keywords", true, trailing_tags, |ui| {
                    if facets.tags.is_empty() {
                        ui.label(RichText::new("No keywords created").size(FONT_SMALL).color(TEXT_HINT));
                    }
                    for tag in &facets.tags {
                        let is_sel = filter.tag.as_ref() == Some(&tag.name);
                        if Self::nav_item(ui, is_sel, icon::TAG, &tag.name, tag.count, Some(ACCENT_HOVER), None) {
                            filter.tag = if is_sel { None } else { Some(tag.name.clone()) };
                            changed = true;
                        }
                    }
                });

                // Section: SHORTCUTS CHEAT SHEET
                widgets::section(ui, "Keyboard Shortcuts", false, None, |ui| {
                    let shortcuts = [
                        ("G", "Grid View"),
                        ("E / Enter", "Player View"),
                        ("Space", "Play / Pause"),
                        ("Left / Right", "Step 1 Frame"),
                        ("Home / End", "First / Last Frame"),
                        ("1 – 5", "Set Star Rating"),
                        ("0", "Clear Rating"),
                        ("P", "Flag Pick"),
                        ("X", "Flag Reject"),
                        ("U", "Clear Flag"),
                        ("Z", "Toggle Fit / 100%"),
                        ("F", "Toggle Fullscreen"),
                    ];

                    for (key, desc) in shortcuts {
                        ui.horizontal(|ui| {
                            ui.set_min_height(20.0);
                            widgets::keycap(ui, key);
                            ui.add_space(4.0);
                            ui.label(RichText::new(desc).size(FONT_SMALL).color(TEXT_SECONDARY));
                        });
                    }
                });

                ui.add_space(10.0);
            });

        changed
    }

    /// Renders a consistent, professional studio row with icon, title, and right-aligned badge pill
    fn nav_item(
        ui: &mut Ui,
        is_selected: bool,
        icon_str: &str,
        label: &str,
        count: usize,
        glyph_color: Option<Color32>,
        custom_icon_font: Option<FontId>,
    ) -> bool {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::click());
        let painter = ui.painter();

        // Row background
        if is_selected {
            painter.rect_filled(rect, Rounding::same(ROUNDING), ACCENT_DIM);
            painter.rect_stroke(rect, Rounding::same(ROUNDING), Stroke::new(1.0_f32, ACCENT));
        } else if response.hovered() {
            painter.rect_filled(rect, Rounding::same(ROUNDING), BG_CARD_HOVER);
        }

        // Left Icon + Label
        let icon_col = glyph_color.unwrap_or(if is_selected { Color32::WHITE } else { TEXT_SECONDARY });
        let text_col = if is_selected { Color32::WHITE } else { TEXT_PRIMARY };
        let text_y = rect.center().y;

        let icon_font = custom_icon_font.unwrap_or_else(|| FontId::proportional(13.0));
        painter.text(
            Pos2::new(rect.min.x + 8.0, text_y),
            Align2::LEFT_CENTER,
            icon_str,
            icon_font,
            icon_col,
        );

        // Right Count Badge Pill
        let count_str = format!("{}", count);
        let badge_w = (count_str.len() as f32 * 6.5 + 12.0).max(20.0);
        let badge_rect = Rect::from_center_size(
            Pos2::new(rect.max.x - badge_w * 0.5 - 6.0, text_y),
            Vec2::new(badge_w, 16.0),
        );

        let badge_bg = if is_selected {
            ACCENT.gamma_multiply(0.4)
        } else {
            BG_FIELD
        };
        painter.rect_filled(badge_rect, Rounding::same(8.0), badge_bg);
        painter.text(
            badge_rect.center(),
            Align2::CENTER_CENTER,
            &count_str,
            FontId::proportional(FONT_CAPTION),
            if is_selected { Color32::WHITE } else { TEXT_MUTED },
        );

        // Text label in middle with safe elision
        let max_label_w = (badge_rect.min.x - (rect.min.x + 28.0) - 8.0).max(10.0);
        let font_id = FontId::proportional(FONT_BODY);
        let elided = widgets::elide(ui, label, &font_id, max_label_w);

        painter.text(
            Pos2::new(rect.min.x + 28.0, text_y),
            Align2::LEFT_CENTER,
            elided,
            font_id,
            text_col,
        );

        response.clicked()
    }
}
