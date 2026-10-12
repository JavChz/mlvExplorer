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
            text_primary(ui),
        );
        painter.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0_f32, border_subtle(ui)));

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
                let trailing_cam = (!facets.cameras.is_empty()).then(|| RichText::new(facets.cameras.len().to_string()).size(FONT_CAPTION).color(text_muted(ui)).into());
                widgets::section(ui, "Cameras", true, trailing_cam, |ui| {
                    if facets.cameras.is_empty() {
                        ui.label(RichText::new("No cameras detected").size(FONT_SMALL).color(text_hint(ui)));
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
                let trailing_lens = (!facets.lenses.is_empty()).then(|| RichText::new(facets.lenses.len().to_string()).size(FONT_CAPTION).color(text_muted(ui)).into());
                widgets::section(ui, "Lenses", true, trailing_lens, |ui| {
                    if facets.lenses.is_empty() {
                        ui.label(RichText::new("No lenses detected").size(FONT_SMALL).color(text_hint(ui)));
                    }
                    for lens in &facets.lenses {
                        let is_sel = filter.lens.as_ref() == Some(&lens.name);
                        if Self::nav_item(ui, is_sel, icon::APERTURE, &lens.name, lens.count, None, None) {
                            filter.lens = if is_sel { None } else { Some(lens.name.clone()) };
                            changed = true;
                        }
                    }
                });

                // Section: KEYWORDS / TAGS (Hierarchical tree like Lightroom)
                let total_tags_count = facets.tags.len();
                let trailing_tags = (total_tags_count > 0).then(|| RichText::new(total_tags_count.to_string()).size(FONT_CAPTION).color(text_muted(ui)).into());
                widgets::section(ui, "Keywords", true, trailing_tags, |ui| {
                    if facets.tag_tree.is_empty() {
                        ui.label(RichText::new("No keywords created").size(FONT_SMALL).color(text_hint(ui)));
                    } else {
                        for node in &facets.tag_tree {
                            if Self::render_keyword_tree_node(ui, node, filter, 0, &mut changed) {
                                // Handled inside helper
                            }
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
                            ui.label(RichText::new(desc).size(FONT_SMALL).color(text_secondary(ui)));
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
        let is_dark = ui.visuals().dark_mode;
        let is_disabled = count == 0 && !is_selected && label != "All Clips";
        let sense = if is_disabled { Sense::hover() } else { Sense::click() };
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), sense);
        let painter = ui.painter();

        // Row background
        if is_selected {
            let bg_sel = if is_dark { ACCENT_DIM } else { ACCENT };
            painter.rect_filled(rect, Rounding::same(ROUNDING), bg_sel);
            painter.rect_stroke(rect, Rounding::same(ROUNDING), Stroke::new(1.0_f32, ACCENT));
        } else if !is_disabled && response.hovered() {
            painter.rect_filled(rect, Rounding::same(ROUNDING), bg_card_hover(ui));
        }

        // Left Icon + Label
        let icon_col = if is_disabled {
            text_disabled(ui)
        } else {
            glyph_color.unwrap_or(if is_selected { Color32::WHITE } else { text_secondary(ui) })
        };
        let text_col = if is_disabled {
            text_disabled(ui)
        } else if is_selected {
            Color32::WHITE
        } else {
            text_primary(ui)
        };
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
            if is_dark { ACCENT.gamma_multiply(0.4) } else { Color32::from_white_alpha(50) }
        } else if is_disabled {
            bg_field(ui).gamma_multiply(0.45)
        } else {
            bg_field(ui)
        };
        let badge_text_col = if is_disabled {
            text_disabled(ui)
        } else if is_selected {
            Color32::WHITE
        } else {
            text_muted(ui)
        };
        painter.rect_filled(badge_rect, Rounding::same(8.0), badge_bg);
        painter.text(
            badge_rect.center(),
            Align2::CENTER_CENTER,
            &count_str,
            FontId::proportional(FONT_CAPTION),
            badge_text_col,
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

        !is_disabled && response.clicked()
    }

    /// Renders a hierarchical keyword node like Lightroom:
    /// - Indented according to depth level
    /// - Expand / collapse caret if node has children
    /// - Tag glyph + Keyword name
    /// - Aggregated count badge
    /// - Hover arrow icon on the right to immediately filter the view to this keyword
    fn render_keyword_tree_node(
        ui: &mut Ui,
        node: &crate::db::TagNode,
        filter: &mut FilterCriteria,
        depth: usize,
        changed: &mut bool,
    ) -> bool {
        let is_sel = filter.tag.as_ref() == Some(&node.name);
        let is_disabled = node.total_count == 0 && !is_sel;
        let id = ui.make_persistent_id(format!("kw_node_{}", node.id));
        let mut is_expanded = ui.data_mut(|d| d.get_temp::<bool>(id).unwrap_or(true));
        let is_dark = ui.visuals().dark_mode;

        let indent = depth as f32 * 14.0;
        let row_height = 24.0;
        let sense = if is_disabled { Sense::hover() } else { Sense::click() };
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_height), sense);
        let painter = ui.painter();

        // Row background
        if is_sel {
            let bg_sel = if is_dark { ACCENT_DIM } else { ACCENT };
            painter.rect_filled(rect, Rounding::same(ROUNDING), bg_sel);
            painter.rect_stroke(rect, Rounding::same(ROUNDING), Stroke::new(1.0_f32, ACCENT));
        } else if !is_disabled && response.hovered() {
            painter.rect_filled(rect, Rounding::same(ROUNDING), bg_card_hover(ui));
        }

        let center_y = rect.center().y;
        let mut cur_x = rect.min.x + 4.0 + indent;

        // Caret toggle for children
        let has_children = !node.children.is_empty();
        if has_children {
            let caret_glyph = if is_expanded { icon::CARET_DOWN } else { icon::CARET_RIGHT };
            let caret_rect = Rect::from_center_size(Pos2::new(cur_x + 6.0, center_y), Vec2::new(14.0, 14.0));
            let caret_resp = ui.interact(caret_rect, id.with("caret"), Sense::click());
            if caret_resp.clicked() {
                is_expanded = !is_expanded;
                ui.data_mut(|d| d.insert_temp(id, is_expanded));
            }
            painter.text(
                caret_rect.center(),
                Align2::CENTER_CENTER,
                caret_glyph,
                FontId::proportional(11.0),
                if caret_resp.hovered() {
                    text_primary(ui)
                } else if is_disabled {
                    text_disabled(ui)
                } else {
                    text_muted(ui)
                },
            );
        } else {
            // Bullet or subtle dot
            let dot_col = if is_disabled {
                text_disabled(ui)
            } else if is_sel {
                Color32::WHITE
            } else {
                text_muted(ui)
            };
            painter.circle_filled(Pos2::new(cur_x + 6.0, center_y), 1.5, dot_col);
        }
        cur_x += 16.0;

        // Tag icon
        let tag_col = if is_disabled {
            text_disabled(ui)
        } else if is_sel {
            Color32::WHITE
        } else {
            ACCENT
        };
        painter.text(
            Pos2::new(cur_x, center_y),
            Align2::LEFT_CENTER,
            icon::TAG,
            FontId::proportional(12.0),
            tag_col,
        );
        cur_x += 16.0;

        // Right side: Count badge + Filter Arrow button
        let arrow_size = 18.0;
        let arrow_rect = Rect::from_center_size(
            Pos2::new(rect.max.x - arrow_size * 0.5 - 4.0, center_y),
            Vec2::new(arrow_size, arrow_size),
        );
        let arrow_resp = ui.interact(arrow_rect, id.with("arrow_filter"), sense);
        let arrow_hovered = !is_disabled && arrow_resp.hovered();

        // Right Count Badge Pill
        let count_str = format!("{}", node.total_count);
        let badge_w = (count_str.len() as f32 * 6.5 + 10.0).max(18.0);
        let badge_rect = Rect::from_center_size(
            Pos2::new(arrow_rect.min.x - badge_w * 0.5 - 4.0, center_y),
            Vec2::new(badge_w, 15.0),
        );

        let badge_bg = if is_sel {
            if is_dark { ACCENT.gamma_multiply(0.4) } else { Color32::from_white_alpha(50) }
        } else if is_disabled {
            bg_field(ui).gamma_multiply(0.45)
        } else {
            bg_field(ui)
        };
        let badge_text_col = if is_disabled {
            text_disabled(ui)
        } else if is_sel {
            Color32::WHITE
        } else {
            text_muted(ui)
        };
        painter.rect_filled(badge_rect, Rounding::same(8.0), badge_bg);
        painter.text(
            badge_rect.center(),
            Align2::CENTER_CENTER,
            &count_str,
            FontId::proportional(FONT_CAPTION),
            badge_text_col,
        );

        // Arrow button on right (Lightroom style filter arrow)
        if !is_disabled && (response.hovered() || is_sel || arrow_hovered) {
            let arrow_bg = if arrow_hovered { ACCENT } else { Color32::TRANSPARENT };
            painter.rect_filled(arrow_rect, Rounding::same(3.0), arrow_bg);
            painter.text(
                arrow_rect.center(),
                Align2::CENTER_CENTER,
                icon::ARROW_RIGHT,
                FontId::proportional(12.0),
                if arrow_hovered || is_sel { Color32::WHITE } else { text_secondary(ui) },
            );
        }

        // Clicking the arrow or row sets/toggles filtering to this keyword
        if !is_disabled && (arrow_resp.clicked() || response.clicked()) {
            filter.tag = if is_sel { None } else { Some(node.name.clone()) };
            *changed = true;
        }

        // Text label in middle with safe elision
        let max_label_w = (badge_rect.min.x - cur_x - 6.0).max(10.0);
        let font_id = FontId::proportional(FONT_BODY);
        let elided = widgets::elide(ui, &node.name, &font_id, max_label_w);
        let label_col = if is_disabled {
            text_disabled(ui)
        } else if is_sel {
            Color32::WHITE
        } else {
            text_primary(ui)
        };
        painter.text(
            Pos2::new(cur_x, center_y),
            Align2::LEFT_CENTER,
            elided,
            font_id,
            label_col,
        );

        // Recursively render children if expanded
        if has_children && is_expanded {
            for child in &node.children {
                Self::render_keyword_tree_node(ui, child, filter, depth + 1, changed);
            }
        }

        !is_disabled && (response.clicked() || arrow_resp.clicked())
    }
}
