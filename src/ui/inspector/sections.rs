use super::types::InspectorAction;
use crate::db::{Database, VideoRecord};
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{Rect, RichText, Rounding, Sense, Stroke, Ui, Vec2};

/// Clip identity block + rating & flag (always visible at the top of the inspector).
pub struct OrganizationSection;

impl OrganizationSection {
    pub fn show(ui: &mut Ui, vid: &mut VideoRecord, db: &Database, selected_count: usize) -> Option<InspectorAction> {
        let mut action = None;

        egui::Frame::none()
            .inner_margin(egui::Margin { left: PANEL_PADDING, right: PANEL_PADDING, top: 10.0, bottom: 10.0 })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                // Clip name + one-line summary
                ui.add(
                    egui::Label::new(RichText::new(&vid.file_name).size(FONT_TITLE).strong().color(text_primary(ui)))
                        .truncate(),
                );
                let mut summary = format!("{} × {}  ·  {}", vid.width, vid.height, widgets::format_fps(vid.fps));
                if !vid.camera_name.is_empty() {
                    summary = format!("{}  ·  {}", vid.camera_name, summary);
                }
                ui.add(egui::Label::new(RichText::new(summary).size(FONT_SMALL).color(text_secondary(ui))).truncate());
                ui.add_space(8.0);

                widgets::field_row(ui, "Rating", |ui| {
                    if let Some(new_r) = widgets::rating_stars(ui, vid.rating, 15.0) {
                        vid.rating = new_r;
                        let _ = db.set_rating(vid.id, new_r);
                        action = Some(InspectorAction::BatchSetRating(new_r));
                    }
                });

                widgets::field_row(ui, "Flag", |ui| {
                    if let Some(new_flag) = widgets::flag_buttons(ui, vid.flag, false) {
                        vid.flag = new_flag;
                        let _ = db.set_flag(vid.id, new_flag);
                        action = Some(InspectorAction::BatchSetFlag(new_flag));
                    }
                });

                ui.add_space(8.0);
                let label = if selected_count > 1 {
                    format!("Open in MLV App ({} clips)", selected_count)
                } else {
                    "Open in MLV App".to_string()
                };
                let tooltip = if selected_count > 1 {
                    format!("Open all {} selected clips into a single MLV App session for batch grading and export", selected_count)
                } else {
                    "Open this clip in MLV App for grading and export".to_string()
                };
                let btn = egui::Button::new(
                    RichText::new(widgets::icon_label(icon::ARROW_SQUARE_OUT, &label))
                        .color(egui::Color32::WHITE)
                        .strong(),
                )
                .fill(ACCENT)
                .stroke(Stroke::NONE)
                .min_size(Vec2::new(ui.available_width(), CONTROL_HEIGHT + 4.0));
                if ui
                    .add(btn)
                    .on_hover_text(tooltip)
                    .clicked()
                {
                    action = Some(InspectorAction::OpenInMlvApp);
                }
            });

        action
    }
}

pub struct ProductionSlateSection;

impl ProductionSlateSection {
    pub fn show(ui: &mut Ui, vid: &mut VideoRecord, db: &Database) -> Option<InspectorAction> {
        let mut action = None;
        let trailing = vid.slate_label().map(|s| RichText::new(s).size(FONT_CAPTION).color(text_muted(ui)).into());

        widgets::section(ui, "Slate", true, trailing, |ui| {
            let mut changed = false;
            changed |= widgets::field_row(ui, "Scene", |ui| widgets::optional_text_field(ui, &mut vid.scene, "e.g. 12A"));
            changed |= widgets::field_row(ui, "Shot", |ui| widgets::optional_text_field(ui, &mut vid.shot, "e.g. 3"));
            changed |= widgets::field_row(ui, "Take", |ui| widgets::optional_text_field(ui, &mut vid.take, "e.g. 1"));
            changed |= widgets::field_row(ui, "Camera #", |ui| widgets::optional_text_field(ui, &mut vid.camera_unit, "e.g. A"));
            changed |= widgets::field_row(ui, "Location", |ui| widgets::optional_text_field(ui, &mut vid.location, "e.g. Stage B"));

            if changed {
                let _ = db.set_production_slate(
                    vid.id,
                    vid.scene.as_deref(),
                    vid.shot.as_deref(),
                    vid.take.as_deref(),
                    vid.location.as_deref(),
                    vid.camera_unit.as_deref(),
                );
                action = Some(InspectorAction::BatchSetSlate {
                    scene: vid.scene.clone(),
                    shot: vid.shot.clone(),
                    take: vid.take.clone(),
                    location: vid.location.clone(),
                    camera_unit: vid.camera_unit.clone(),
                });
            }
        });
        action
    }
}

/// Keyword chip with a remove affordance. Returns true when remove was clicked.
fn keyword_chip(ui: &mut Ui, text: &str) -> bool {
    let mut removed = false;
    egui::Frame::none()
        .fill(bg_section(ui))
        .stroke(Stroke::new(1.0_f32, border_strong(ui)))
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(egui::Margin { left: 8.0, right: 5.0, top: 2.0, bottom: 2.0 })
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.horizontal(|ui| {
                ui.label(RichText::new(text).size(FONT_SMALL).color(text_primary(ui)));
                let x = ui
                    .add(egui::Label::new(RichText::new(icon::X).size(10.0).color(text_muted(ui))).sense(Sense::click()))
                    .on_hover_text("Remove keyword");
                if x.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if x.clicked() {
                    removed = true;
                }
            });
        });
    removed
}

pub struct TagsSection;

impl TagsSection {
    pub fn show(
        ui: &mut Ui,
        vid: &mut VideoRecord,
        db: &Database,
        facets: &crate::db::Facets,
        inspector: &mut super::types::Inspector,
    ) -> Option<InspectorAction> {
        let mut action = None;
        let trailing = (!vid.tags.is_empty())
            .then(|| RichText::new(vid.tags.len().to_string()).size(FONT_CAPTION).color(text_muted(ui)).into());

        widgets::section(ui, "Keywords", true, trailing, |ui| {
            // 1. Existing keywords as removable chips on the active clip
            if !vid.tags.is_empty() {
                let mut to_remove = None;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                    for t in &vid.tags {
                        if keyword_chip(ui, t) {
                            to_remove = Some(t.clone());
                        }
                    }
                });
                if let Some(tag_name) = to_remove {
                    vid.tags.retain(|t| t != &tag_name);
                    let _ = db.remove_tag(vid.id, &tag_name);
                    action = Some(InspectorAction::BatchRemoveTag(tag_name));
                }
                ui.add_space(8.0);
            }

            // 2. Add new keyword with optional Parent selector (Hierarchy) + Open Keyword Manager
            let flat_options = crate::db::flatten_tree_with_connectors(&facets.full_tag_tree);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let btn_space = (CONTROL_HEIGHT + 2.0) * 2.0 + 4.0;
                let edit_w = (ui.available_width() - btn_space).max(60.0);
                let resp = ui.add_sized(
                    [edit_w, CONTROL_HEIGHT],
                    egui::TextEdit::singleline(&mut inspector.new_tag_input)
                        .hint_text(widgets::hint("New keyword… [Enter]"))
                        .margin(egui::vec2(6.0, 3.0)),
                );

                let is_enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let mut commit = false;
                if (resp.lost_focus() || resp.has_focus()) && is_enter {
                    commit = true;
                    resp.request_focus();
                }

                let can_add = !inspector.new_tag_input.trim().is_empty();
                ui.add_enabled_ui(can_add, |ui| {
                    if widgets::icon_button(ui, icon::PLUS, false, "Create & assign keyword [Enter]").clicked() {
                        commit = true;
                    }
                });

                if widgets::icon_button(ui, icon::TREE_STRUCTURE, false, "Open Keyword Taxonomy Manager…").clicked() {
                    action = Some(InspectorAction::OpenKeywordManager);
                }

                if commit && !inspector.new_tag_input.trim().is_empty() {
                    let tag_name = inspector.new_tag_input.trim().to_string();
                    let parent_id = inspector.new_tag_parent_id;
                    let _ = db.create_tag(&tag_name, parent_id);
                    if !vid.tags.contains(&tag_name) {
                        vid.tags.push(tag_name.clone());
                        let _ = db.add_tag(vid.id, &tag_name);
                    }
                    action = Some(InspectorAction::BatchAddTag(tag_name.clone()));
                    inspector.new_tag_input.clear();
                    inspector.new_tag_parent_id = None;
                }
            });

            // Parent keyword dropdown selector for new keyword (hierarchical with tree connector lines)
            if !flat_options.is_empty() {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Parent:").size(FONT_CAPTION).color(text_muted(ui)));
                    let current_parent_name = inspector
                        .new_tag_parent_id
                        .and_then(|pid| flat_options.iter().find(|t| t.id == pid).map(|t| t.name.as_str()))
                        .unwrap_or("None (Top Level)");

                    egui::ComboBox::from_id_salt("inspector_keyword_parent_select")
                        .selected_text(RichText::new(current_parent_name).size(FONT_CAPTION))
                        .width((ui.available_width() - 4.0).max(80.0))
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(inspector.new_tag_parent_id.is_none(), "None (Top Level)").clicked() {
                                inspector.new_tag_parent_id = None;
                            }
                            for opt in &flat_options {
                                let is_active = inspector.new_tag_parent_id == Some(opt.id);
                                if ui.selectable_label(is_active, &opt.display_label).clicked() {
                                    inspector.new_tag_parent_id = Some(opt.id);
                                }
                            }
                        });
                });
            }

            ui.add_space(6.0);
            widgets::toolbar_divider(ui);
            ui.add_space(4.0);

            // 3. Complete Keyword List with Checkboxes, Inline Child Creator (+), Count Badges, and Filter Arrow
            ui.label(RichText::new("ALL KEYWORDS (Check to assign)").size(FONT_CAPTION).color(text_muted(ui)).strong());
            ui.add_space(2.0);

            if facets.full_tag_tree.is_empty() {
                ui.label(RichText::new("No keywords in catalog yet.").size(FONT_SMALL).color(text_hint(ui)));
            } else {
                for node in &facets.full_tag_tree {
                    if let Some(act) = Self::render_inspector_keyword_node(ui, node, vid, db, 0) {
                        if action.is_none() {
                            action = Some(act);
                        }
                    }
                }
            }
        });
        action
    }

    /// Renders a hierarchical keyword item inside the Inspector matching the left Navigator's studio row design:
    /// - Indentation + expand/collapse caret
    /// - Checkbox to assign/unassign keyword on the selected clip(s)
    /// - Tag icon + Keyword name
    /// - Right-aligned count badge pill, `+` button to add a child keyword inline, and `→` filter arrow
    fn render_inspector_keyword_node(
        ui: &mut Ui,
        node: &crate::db::TagNode,
        vid: &mut VideoRecord,
        db: &Database,
        depth: usize,
    ) -> Option<InspectorAction> {
        let mut action = None;
        let id = ui.make_persistent_id(format!("insp_kw_{}", node.id));
        let mut is_expanded = ui.data_mut(|d| d.get_temp::<bool>(id).unwrap_or(true));
        let is_dark = ui.visuals().dark_mode;

        let add_child_target_id = ui.make_persistent_id("insp_add_child_target_id");
        let add_child_text_id = ui.make_persistent_id("insp_add_child_text");
        let add_child_focus_id = ui.make_persistent_id("insp_add_child_focus");

        let mut active_child_parent = ui.data_mut(|d| d.get_temp::<Option<i64>>(add_child_target_id).unwrap_or(None));
        let is_adding_child_here = active_child_parent == Some(node.id);

        let has_children = !node.children.is_empty();
        let indent = depth as f32 * 14.0;
        let is_checked = vid.tags.contains(&node.name);

        let row_height = 24.0;
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_height), Sense::click());
        let center_y = rect.center().y;

        // Right-side interactive buttons: Filter Arrow + Add Child (+)
        let arrow_size = 18.0;
        let arrow_rect = Rect::from_center_size(
            egui::Pos2::new(rect.max.x - arrow_size * 0.5 - 4.0, center_y),
            Vec2::new(arrow_size, arrow_size),
        );
        let arrow_resp = ui
            .interact(arrow_rect, id.with("arrow_filter"), Sense::click())
            .on_hover_text(format!("Filter catalog by keyword '{}'", node.name));
        let arrow_hovered = arrow_resp.hovered();

        let plus_rect = Rect::from_center_size(
            egui::Pos2::new(arrow_rect.min.x - arrow_size * 0.5 - 2.0, center_y),
            Vec2::new(arrow_size, arrow_size),
        );
        let plus_resp = ui
            .interact(plus_rect, id.with("plus_child"), Sense::click())
            .on_hover_text(format!("Add child keyword under '{}'", node.name));
        let plus_hovered = plus_resp.hovered();

        // Left-side caret interaction
        let mut cur_x = rect.min.x + 4.0 + indent;
        let caret_rect = Rect::from_center_size(egui::Pos2::new(cur_x + 6.0, center_y), Vec2::new(14.0, 14.0));
        let caret_resp = if has_children {
            Some(ui.interact(caret_rect, id.with("caret"), Sense::click()))
        } else {
            None
        };
        let caret_clicked = caret_resp.as_ref().map_or(false, |r| r.clicked());
        let caret_hovered = caret_resp.as_ref().map_or(false, |r| r.hovered());

        if caret_clicked {
            is_expanded = !is_expanded;
            ui.data_mut(|d| d.insert_temp(id, is_expanded));
        }

        let row_or_btn_hovered = response.hovered() || arrow_hovered || plus_hovered || is_adding_child_here;
        if row_or_btn_hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let painter = ui.painter();

        // Row background
        if is_checked {
            let bg_sel = if is_dark {
                ACCENT_DIM.gamma_multiply(0.55)
            } else {
                ACCENT.gamma_multiply(0.18)
            };
            painter.rect_filled(rect, Rounding::same(ROUNDING), bg_sel);
            if row_or_btn_hovered {
                painter.rect_stroke(rect, Rounding::same(ROUNDING), Stroke::new(1.0_f32, ACCENT));
            }
        } else if row_or_btn_hovered {
            painter.rect_filled(rect, Rounding::same(ROUNDING), bg_card_hover(ui));
        }

        // 1. Caret or dot
        if has_children {
            let caret_glyph = if is_expanded { icon::CARET_DOWN } else { icon::CARET_RIGHT };
            painter.text(
                caret_rect.center(),
                egui::Align2::CENTER_CENTER,
                caret_glyph,
                egui::FontId::proportional(11.0),
                if caret_hovered { text_primary(ui) } else { text_muted(ui) },
            );
        } else {
            painter.circle_filled(egui::Pos2::new(cur_x + 6.0, center_y), 1.5, text_muted(ui));
        }
        cur_x += 16.0;

        // 2. Studio Checkbox
        let cb_size = 14.0;
        let cb_rect = Rect::from_center_size(egui::Pos2::new(cur_x + cb_size * 0.5, center_y), Vec2::new(cb_size, cb_size));
        if is_checked {
            painter.rect_filled(cb_rect, Rounding::same(3.0), ACCENT);
            painter.text(
                cb_rect.center(),
                egui::Align2::CENTER_CENTER,
                icon::CHECK,
                egui::FontId::proportional(10.0),
                egui::Color32::WHITE,
            );
        } else {
            painter.rect_filled(cb_rect, Rounding::same(3.0), bg_field(ui));
            painter.rect_stroke(
                cb_rect,
                Rounding::same(3.0),
                Stroke::new(1.0_f32, if response.hovered() { ACCENT } else { border_strong(ui) }),
            );
        }
        cur_x += cb_size + 6.0;

        // 3. Tag Icon
        painter.text(
            egui::Pos2::new(cur_x, center_y),
            egui::Align2::LEFT_CENTER,
            icon::TAG,
            egui::FontId::proportional(12.0),
            if is_checked { ACCENT_HOVER } else { ACCENT },
        );
        cur_x += 15.0;

        // 4. Right Count Badge Pill
        let count_str = format!("{}", node.total_count);
        let badge_w = (count_str.len() as f32 * 6.5 + 10.0).max(18.0);
        let badge_right_anchor = if row_or_btn_hovered { plus_rect.min.x - 2.0 } else { arrow_rect.min.x - 4.0 };
        let badge_rect = Rect::from_center_size(
            egui::Pos2::new(badge_right_anchor - badge_w * 0.5, center_y),
            Vec2::new(badge_w, 15.0),
        );
        let badge_bg = if is_checked {
            if is_dark { ACCENT.gamma_multiply(0.35) } else { ACCENT.gamma_multiply(0.2) }
        } else {
            bg_field(ui)
        };
        painter.rect_filled(badge_rect, Rounding::same(8.0), badge_bg);
        painter.text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            &count_str,
            egui::FontId::proportional(FONT_CAPTION),
            if is_checked { text_primary(ui) } else { text_muted(ui) },
        );

        // 5. Plus button on hover (Add child keyword)
        if row_or_btn_hovered {
            let plus_bg = if plus_hovered || is_adding_child_here { ACCENT } else { egui::Color32::TRANSPARENT };
            painter.rect_filled(plus_rect, Rounding::same(3.0), plus_bg);
            painter.text(
                plus_rect.center(),
                egui::Align2::CENTER_CENTER,
                icon::PLUS,
                egui::FontId::proportional(11.0),
                if plus_hovered || is_adding_child_here {
                    egui::Color32::WHITE
                } else {
                    text_secondary(ui)
                },
            );
        }

        // 6. Filter Arrow button on right
        if row_or_btn_hovered {
            let arrow_bg = if arrow_hovered { ACCENT } else { egui::Color32::TRANSPARENT };
            painter.rect_filled(arrow_rect, Rounding::same(3.0), arrow_bg);
            painter.text(
                arrow_rect.center(),
                egui::Align2::CENTER_CENTER,
                icon::ARROW_RIGHT,
                egui::FontId::proportional(12.0),
                if arrow_hovered { egui::Color32::WHITE } else { text_secondary(ui) },
            );
        }

        // 7. Keyword Name Label
        let max_label_w = (badge_rect.min.x - cur_x - 6.0).max(10.0);
        let font_id = egui::FontId::proportional(FONT_BODY);
        let elided = widgets::elide(ui, &node.name, &font_id, max_label_w);
        painter.text(
            egui::Pos2::new(cur_x, center_y),
            egui::Align2::LEFT_CENTER,
            elided,
            font_id,
            if is_checked { text_primary(ui) } else { text_secondary(ui) },
        );

        // Handle clicks: Plus -> inline child input; Arrow -> filter catalog; Row -> toggle checkbox
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
        } else if arrow_resp.clicked() {
            action = Some(InspectorAction::FilterByTag(node.name.clone()));
        } else if response.clicked() && !caret_clicked {
            if !is_checked {
                if !vid.tags.contains(&node.name) {
                    vid.tags.push(node.name.clone());
                    let _ = db.add_tag(vid.id, &node.name);
                    action = Some(InspectorAction::BatchAddTag(node.name.clone()));
                }
            } else {
                vid.tags.retain(|t| t != &node.name);
                let _ = db.remove_tag(vid.id, &node.name);
                action = Some(InspectorAction::BatchRemoveTag(node.name.clone()));
            }
        }

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
                    if widgets::icon_button(ui, icon::CHECK, true, "Create & assign child keyword [Enter]").clicked() {
                        commit_child = true;
                    }
                });
                if esc_pressed || widgets::icon_button(ui, icon::X, false, "Cancel [Esc]").clicked() {
                    active_child_parent = None;
                    child_text.clear();
                } else if commit_child {
                    let clean = child_text.trim().to_string();
                    let _ = db.create_tag(&clean, Some(node.id));
                    if !vid.tags.contains(&clean) {
                        vid.tags.push(clean.clone());
                        let _ = db.add_tag(vid.id, &clean);
                    }
                    action = Some(InspectorAction::BatchAddTag(clean));
                    child_text.clear();
                    active_child_parent = None;
                    is_expanded = true;
                    ui.data_mut(|d| d.insert_temp(id, true));
                }
            });

            ui.data_mut(|d| {
                d.insert_temp(add_child_target_id, active_child_parent);
                d.insert_temp(add_child_text_id, child_text);
                d.insert_temp(add_child_focus_id, should_focus);
            });
        }

        // Recursively render child keywords if expanded
        if has_children && is_expanded {
            for child in &node.children {
                if let Some(sub_act) = Self::render_inspector_keyword_node(ui, child, vid, db, depth + 1) {
                    if action.is_none() {
                        action = Some(sub_act);
                    }
                }
            }
        }

        action
    }
}

pub struct NotesSection;

impl NotesSection {
    pub fn show(ui: &mut Ui, vid: &mut VideoRecord, db: &Database) -> Option<InspectorAction> {
        let mut action = None;
        widgets::section(ui, "Notes", true, None, |ui| {
            let response = ui.add(
                egui::TextEdit::multiline(&mut vid.notes)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .margin(egui::vec2(6.0, 4.0))
                    .hint_text(widgets::hint("Add notes or comments…")),
            );
            if response.changed() {
                let _ = db.set_notes(vid.id, &vid.notes);
                action = Some(InspectorAction::BatchSetNotes(vid.notes.clone()));
            }
        });
        action
    }
}
