use crate::db::{Facets, FilterCriteria, PickFlag};
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};

#[derive(Debug, Clone, PartialEq)]
pub enum NavigatorAction {
    FilterChanged,
    OpenTagInMlvApp(String),
    OpenCatalogInMlvApp,
    OpenFlagInMlvApp(PickFlag),
}

pub struct Navigator;

impl Navigator {
    pub fn show(
        ui: &mut Ui,
        filter: &mut FilterCriteria,
        facets: &Facets,
        db: &crate::db::Database,
        open_keyword_manager: &mut bool,
        open_ingest_dialog: &mut bool,
    ) -> Option<NavigatorAction> {
        let mut action = None;

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
                        let (clicked, resp) = Self::nav_item(ui, is_all, icon::FOLDER, "All Clips", facets.total, None, None);
                        resp.context_menu(|ui| {
                            if ui.button(widgets::icon_label(icon::ARROW_SQUARE_OUT, &format!("Open All Clips in MLV App ({} clips)", facets.total))).clicked() {
                                action = Some(NavigatorAction::OpenCatalogInMlvApp);
                                ui.close_menu();
                            }
                        });
                        if clicked {
                            filter.flag = None;
                            filter.camera = None;
                            filter.lens = None;
                            filter.tag = None;
                            action = Some(NavigatorAction::FilterChanged);
                        }

                        // Flagged / Picks
                        let is_picked = filter.flag == Some(PickFlag::Picked);
                        let (clicked, resp) = Self::nav_item(ui, is_picked, icon::FLAG, "Picks", facets.picked, Some(FLAG_PICKED), Some(icon_fill_font(13.0)));
                        resp.context_menu(|ui| {
                            if ui.button(widgets::icon_label(icon::ARROW_SQUARE_OUT, &format!("Open Picks in MLV App ({} clips)", facets.picked))).clicked() {
                                action = Some(NavigatorAction::OpenFlagInMlvApp(PickFlag::Picked));
                                ui.close_menu();
                            }
                        });
                        if clicked {
                            filter.flag = if is_picked { None } else { Some(PickFlag::Picked) };
                            action = Some(NavigatorAction::FilterChanged);
                        }

                        // Rejected
                        let is_rejected = filter.flag == Some(PickFlag::Rejected);
                        let (clicked, resp) = Self::nav_item(ui, is_rejected, icon::X_CIRCLE, "Rejected", facets.rejected, Some(FLAG_REJECTED), Some(icon_fill_font(13.0)));
                        resp.context_menu(|ui| {
                            if ui.button(widgets::icon_label(icon::ARROW_SQUARE_OUT, &format!("Open Rejected in MLV App ({} clips)", facets.rejected))).clicked() {
                                action = Some(NavigatorAction::OpenFlagInMlvApp(PickFlag::Rejected));
                                ui.close_menu();
                            }
                        });
                        if clicked {
                            filter.flag = if is_rejected { None } else { Some(PickFlag::Rejected) };
                            action = Some(NavigatorAction::FilterChanged);
                        }

                        // Unflagged
                        let is_unflagged = filter.flag == Some(PickFlag::Unflagged);
                        let (clicked, resp) = Self::nav_item(ui, is_unflagged, icon::CIRCLE, "Unflagged", facets.unflagged, None, None);
                        resp.context_menu(|ui| {
                            if ui.button(widgets::icon_label(icon::ARROW_SQUARE_OUT, &format!("Open Unflagged in MLV App ({} clips)", facets.unflagged))).clicked() {
                                action = Some(NavigatorAction::OpenFlagInMlvApp(PickFlag::Unflagged));
                                ui.close_menu();
                            }
                        });
                        if clicked {
                            filter.flag = if is_unflagged { None } else { Some(PickFlag::Unflagged) };
                            action = Some(NavigatorAction::FilterChanged);
                        }

                        ui.add_space(6.0);
                        let ingest_btn = egui::Button::new(
                            RichText::new(widgets::icon_label(icon::DOWNLOAD_SIMPLE, "Ingest from Card..."))
                                .size(FONT_SMALL)
                                .color(text_primary(ui)),
                        )
                        .fill(bg_card(ui))
                        .stroke(Stroke::new(1.0_f32, border_subtle(ui)))
                        .rounding(Rounding::same(ROUNDING))
                        .min_size(Vec2::new(ui.available_width(), 22.0));

                        if ui.add(ingest_btn).on_hover_text("Ingest MLV videos from SD/CF card into folder [Ctrl+I]").clicked() {
                            *open_ingest_dialog = true;
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
                        let (clicked, _) = Self::nav_item(ui, is_sel, icon::CAMERA, &cam.name, cam.count, None, None);
                        if clicked {
                            filter.camera = if is_sel { None } else { Some(cam.name.clone()) };
                            action = Some(NavigatorAction::FilterChanged);
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
                        let (clicked, _) = Self::nav_item(ui, is_sel, icon::APERTURE, &lens.name, lens.count, None, None);
                        if clicked {
                            filter.lens = if is_sel { None } else { Some(lens.name.clone()) };
                            action = Some(NavigatorAction::FilterChanged);
                        }
                    }
                });

                // Section: KEYWORDS / TAGS (Hierarchical tree like Lightroom)
                let total_tags_count = facets.tags.len();
                let trailing_tags = (total_tags_count > 0).then(|| RichText::new(total_tags_count.to_string()).size(FONT_CAPTION).color(text_muted(ui)).into());
                widgets::section(ui, "Keywords", true, trailing_tags, |ui| {
                    // Quick Add Keyword directly from the left Navigator sidebar
                    let kw_input_id = ui.make_persistent_id("nav_new_kw_input");
                    let kw_parent_id = ui.make_persistent_id("nav_new_kw_parent");
                    let mut new_kw_text = ui.data_mut(|d| d.get_temp::<String>(kw_input_id).unwrap_or_default());
                    let mut new_kw_parent = ui.data_mut(|d| d.get_temp::<Option<i64>>(kw_parent_id).unwrap_or(None));

                    let flat_options = crate::db::flatten_tree_with_connectors(&facets.full_tag_tree);

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let btn_space = (CONTROL_HEIGHT + 2.0) * 2.0 + 4.0;
                        let edit_w = (ui.available_width() - btn_space).max(60.0);
                        let resp = ui.add_sized(
                            [edit_w, CONTROL_HEIGHT],
                            egui::TextEdit::singleline(&mut new_kw_text)
                                .hint_text(widgets::hint("New keyword… [Enter]"))
                                .margin(egui::vec2(6.0, 3.0)),
                        );

                        let is_enter = (resp.lost_focus() || resp.has_focus())
                            && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let can_add = !new_kw_text.trim().is_empty();
                        let mut commit = is_enter && can_add;

                        ui.add_enabled_ui(can_add, |ui| {
                            if widgets::icon_button(ui, icon::PLUS, false, "Create keyword [Enter]").clicked() {
                                commit = true;
                            }
                        });

                        if widgets::icon_button(ui, icon::TREE_STRUCTURE, false, "Open Keyword Taxonomy Manager…").clicked() {
                            *open_keyword_manager = true;
                        }

                        if commit {
                            let clean_name = new_kw_text.trim().to_string();
                            let _ = db.create_tag(&clean_name, new_kw_parent);
                            new_kw_text.clear();
                            action = Some(NavigatorAction::FilterChanged);
                            if is_enter {
                                resp.request_focus();
                            }
                        }
                    });

                    if !flat_options.is_empty() {
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Parent:").size(FONT_CAPTION).color(text_muted(ui)));
                            let current_parent_name = new_kw_parent
                                .and_then(|pid| flat_options.iter().find(|t| t.id == pid).map(|t| t.name.as_str()))
                                .unwrap_or("None (Top Level)");

                            egui::ComboBox::from_id_salt("navigator_keyword_parent_select")
                                .selected_text(RichText::new(current_parent_name).size(FONT_CAPTION))
                                .width((ui.available_width() - 4.0).max(80.0))
                                .show_ui(ui, |ui| {
                                    if ui.selectable_label(new_kw_parent.is_none(), "None (Top Level)").clicked() {
                                        new_kw_parent = None;
                                    }
                                    for opt in &flat_options {
                                        let is_active = new_kw_parent == Some(opt.id);
                                        if ui.selectable_label(is_active, &opt.display_label).clicked() {
                                            new_kw_parent = Some(opt.id);
                                        }
                                    }
                                });
                        });
                    }

                    ui.data_mut(|d| {
                        d.insert_temp(kw_input_id, new_kw_text);
                        d.insert_temp(kw_parent_id, new_kw_parent);
                    });

                    ui.add_space(4.0);

                    if facets.tag_tree.is_empty() {
                        ui.label(RichText::new("No keywords created").size(FONT_SMALL).color(text_hint(ui)));
                    } else {
                        for node in &facets.tag_tree {
                            Self::render_keyword_tree_node(ui, node, filter, db, 0, &mut action);
                        }
                    }
                });

                // Section: SHORTCUTS CHEAT SHEET
                widgets::section(ui, "Keyboard Shortcuts", false, None, |ui| {
                    let shortcuts = [
                        ("G", "Grid View"),
                        ("E / Enter", "Player View"),
                        ("Ctrl + M", "Open in MLV App"),
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

        action
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
    ) -> (bool, egui::Response) {
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

        let clicked = !is_disabled && response.clicked();
        (clicked, response)
    }

    /// Renders a hierarchical keyword node like Lightroom:
    /// - Indented according to depth level
    /// - Expand / collapse caret if node has children
    /// - Tag glyph + Keyword name
    /// - Aggregated count badge
    /// - Right-click context menu (Open in MLV App, Filter, Add Child)
    /// - Hover arrow icon on the right to immediately filter the view to this keyword
    fn render_keyword_tree_node(
        ui: &mut Ui,
        node: &crate::db::TagNode,
        filter: &mut FilterCriteria,
        db: &crate::db::Database,
        depth: usize,
        action: &mut Option<NavigatorAction>,
    ) -> bool {
        let is_sel = filter.tag.as_ref() == Some(&node.name);
        let is_disabled = node.total_count == 0 && !is_sel;
        let id = ui.make_persistent_id(format!("kw_node_{}", node.id));
        let mut is_expanded = ui.data_mut(|d| d.get_temp::<bool>(id).unwrap_or(true));
        let is_dark = ui.visuals().dark_mode;

        let add_child_target_id = ui.make_persistent_id("nav_add_child_target_id");
        let add_child_text_id = ui.make_persistent_id("nav_add_child_text");
        let add_child_focus_id = ui.make_persistent_id("nav_add_child_focus");

        let mut active_child_parent = ui.data_mut(|d| d.get_temp::<Option<i64>>(add_child_target_id).unwrap_or(None));
        let is_adding_child_here = active_child_parent == Some(node.id);

        let indent = depth as f32 * 14.0;
        let row_height = 24.0;
        let sense = if is_disabled { Sense::hover() } else { Sense::click() };
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_height), sense);

        // Right-click context menu for keyword node
        response.context_menu(|ui| {
            if ui.button(widgets::icon_label(icon::ARROW_SQUARE_OUT, &format!("Open in MLV App ({} clips)", node.total_count))).clicked() {
                *action = Some(NavigatorAction::OpenTagInMlvApp(node.name.clone()));
                ui.close_menu();
            }
            ui.separator();
            if ui.button(widgets::icon_label(icon::FUNNEL, &format!("Filter by '{}'", node.name))).clicked() {
                filter.tag = Some(node.name.clone());
                *action = Some(NavigatorAction::FilterChanged);
                ui.close_menu();
            }
            if ui.button(widgets::icon_label(icon::PLUS, &format!("Add Child Keyword under '{}'", node.name))).clicked() {
                active_child_parent = Some(node.id);
                ui.data_mut(|d| {
                    d.insert_temp(add_child_target_id, active_child_parent);
                    d.insert_temp(add_child_text_id, String::new());
                    d.insert_temp(add_child_focus_id, true);
                });
                ui.close_menu();
            }
        });

        // Right side: Plus (Add Child) button + Count badge + Filter Arrow button
        let center_y = rect.center().y;
        let arrow_size = 18.0;
        let arrow_rect = Rect::from_center_size(
            Pos2::new(rect.max.x - arrow_size * 0.5 - 4.0, center_y),
            Vec2::new(arrow_size, arrow_size),
        );
        let arrow_resp = ui.interact(arrow_rect, id.with("arrow_filter"), sense);
        let arrow_hovered = !is_disabled && arrow_resp.hovered();

        let plus_rect = Rect::from_center_size(
            Pos2::new(arrow_rect.min.x - arrow_size * 0.5 - 2.0, center_y),
            Vec2::new(arrow_size, arrow_size),
        );
        let plus_resp = ui
            .interact(plus_rect, id.with("plus_child"), Sense::click())
            .on_hover_text(format!("Add child keyword under '{}'", node.name));
        let plus_hovered = plus_resp.hovered();

        let row_or_btn_hovered = response.hovered() || arrow_hovered || plus_hovered || is_adding_child_here;

        let painter = ui.painter();

        // Row background
        if is_sel {
            let bg_sel = if is_dark { ACCENT_DIM } else { ACCENT };
            painter.rect_filled(rect, Rounding::same(ROUNDING), bg_sel);
            painter.rect_stroke(rect, Rounding::same(ROUNDING), Stroke::new(1.0_f32, ACCENT));
        } else if (!is_disabled && response.hovered()) || plus_hovered || is_adding_child_here {
            painter.rect_filled(rect, Rounding::same(ROUNDING), bg_card_hover(ui));
        }

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

        // Right Count Badge Pill
        let count_str = format!("{}", node.total_count);
        let badge_w = (count_str.len() as f32 * 6.5 + 10.0).max(18.0);
        let badge_right_anchor = if row_or_btn_hovered { plus_rect.min.x - 2.0 } else { arrow_rect.min.x - 4.0 };
        let badge_rect = Rect::from_center_size(
            Pos2::new(badge_right_anchor - badge_w * 0.5, center_y),
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

        // Plus button on hover (Add child keyword)
        if row_or_btn_hovered {
            let plus_bg = if plus_hovered || is_adding_child_here { ACCENT } else { Color32::TRANSPARENT };
            painter.rect_filled(plus_rect, Rounding::same(3.0), plus_bg);
            painter.text(
                plus_rect.center(),
                Align2::CENTER_CENTER,
                icon::PLUS,
                FontId::proportional(11.0),
                if plus_hovered || is_adding_child_here || is_sel {
                    Color32::WHITE
                } else {
                    text_secondary(ui)
                },
            );
        }

        // Arrow button on right (Lightroom style filter arrow)
        if !is_disabled && (row_or_btn_hovered || is_sel) {
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

        if plus_resp.clicked() {
            if is_adding_child_here {
                active_child_parent = None;
            } else {
                active_child_parent = Some(node.id);
                ui.data_mut(|d| {
                    d.insert_temp(add_child_text_id, String::new());
                    d.insert_temp(add_child_focus_id, true);
                });
            }
            ui.data_mut(|d| d.insert_temp(add_child_target_id, active_child_parent));
        } else if !is_disabled && (arrow_resp.clicked() || response.clicked()) {
            filter.tag = if is_sel { None } else { Some(node.name.clone()) };
            *action = Some(NavigatorAction::FilterChanged);
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

        // Inline Add Child input row directly below this keyword node
        if active_child_parent == Some(node.id) {
            let mut child_text = ui.data_mut(|d| d.get_temp::<String>(add_child_text_id).unwrap_or_default());
            let mut should_focus = ui.data_mut(|d| d.get_temp::<bool>(add_child_focus_id).unwrap_or(false));

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                ui.add_space(indent + 16.0);
                ui.label(RichText::new("└").size(FONT_SMALL).color(ACCENT));

                let btn_space = (CONTROL_HEIGHT + 2.0) * 2.0 + 4.0;
                let edit_w = (ui.available_width() - btn_space).max(50.0);
                let edit_resp = ui.add_sized(
                    [edit_w, CONTROL_HEIGHT],
                    egui::TextEdit::singleline(&mut child_text)
                        .hint_text(widgets::hint(&format!("Child of {}…", node.name)))
                        .margin(egui::vec2(5.0, 2.0)),
                );
                if should_focus {
                    edit_resp.request_focus();
                    should_focus = false;
                }

                let enter_pressed = (edit_resp.lost_focus() || edit_resp.has_focus())
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let esc_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));

                let can_save = !child_text.trim().is_empty();
                let mut commit_child = enter_pressed && can_save;

                ui.add_enabled_ui(can_save, |ui| {
                    if widgets::icon_button(ui, icon::CHECK, true, "Create child keyword [Enter]").clicked() {
                        commit_child = true;
                    }
                });
                if esc_pressed || widgets::icon_button(ui, icon::X, false, "Cancel [Esc]").clicked() {
                    active_child_parent = None;
                    child_text.clear();
                } else if commit_child {
                    let clean = child_text.trim().to_string();
                    let _ = db.create_tag(&clean, Some(node.id));
                    child_text.clear();
                    active_child_parent = None;
                    is_expanded = true;
                    ui.data_mut(|d| d.insert_temp(id, true));
                    *action = Some(NavigatorAction::FilterChanged);
                }
            });

            ui.data_mut(|d| {
                d.insert_temp(add_child_target_id, active_child_parent);
                d.insert_temp(add_child_text_id, child_text);
                d.insert_temp(add_child_focus_id, should_focus);
            });
        }

        // Recursively render children if expanded
        if has_children && is_expanded {
            for child in &node.children {
                Self::render_keyword_tree_node(ui, child, filter, db, depth + 1, action);
            }
        }

        !is_disabled && (response.clicked() || arrow_resp.clicked())
    }
}
