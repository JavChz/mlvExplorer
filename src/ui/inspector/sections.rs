use super::types::InspectorAction;
use crate::db::{Database, VideoRecord};
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{RichText, Sense, Stroke, Ui, Vec2};

/// Clip identity block + rating & flag (always visible at the top of the inspector).
pub struct OrganizationSection;

impl OrganizationSection {
    pub fn show(ui: &mut Ui, vid: &mut VideoRecord, db: &Database) -> Option<InspectorAction> {
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

                ui.add_space(6.0);
                let btn = egui::Button::new(widgets::icon_label(icon::ARROW_SQUARE_OUT, "Open in MLV App"))
                    .min_size(Vec2::new(ui.available_width(), CONTROL_HEIGHT + 2.0));
                if ui
                    .add(btn)
                    .on_hover_text("Open this clip in MLV App for grading and export")
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

            // 2. Add new keyword with optional Parent selector (Hierarchy)
            let all_tag_records = db.get_all_tags().unwrap_or_default();
            ui.horizontal(|ui| {
                let add_w = CONTROL_HEIGHT + 2.0;
                let resp = ui.add_sized(
                    [ui.available_width() - add_w - ui.spacing().item_spacing.x, CONTROL_HEIGHT],
                    egui::TextEdit::singleline(&mut inspector.new_tag_input)
                        .hint_text(widgets::hint("New keyword…"))
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
                    if widgets::icon_button(ui, icon::PLUS, false, "Create keyword [Enter]").clicked() {
                        commit = true;
                    }
                });

                if commit && !inspector.new_tag_input.trim().is_empty() {
                    let tag_name = inspector.new_tag_input.trim().to_string();
                    let parent_id = inspector.new_tag_parent_id;
                    let _ = db.create_tag(&tag_name, parent_id);
                    if !vid.tags.contains(&tag_name) {
                        vid.tags.push(tag_name.clone());
                        let _ = db.add_tag(vid.id, &tag_name);
                        action = Some(InspectorAction::BatchAddTag(tag_name.clone()));
                    }
                    inspector.new_tag_input.clear();
                    inspector.new_tag_parent_id = None;
                }
            });

            // Parent keyword dropdown selector for new keyword
            if !all_tag_records.is_empty() {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Parent:").size(FONT_CAPTION).color(text_muted(ui)));
                    let current_parent_name = inspector
                        .new_tag_parent_id
                        .and_then(|pid| all_tag_records.iter().find(|t| t.id == pid).map(|t| t.name.as_str()))
                        .unwrap_or("None (Top Level)");

                    egui::ComboBox::from_id_salt("inspector_keyword_parent_select")
                        .selected_text(RichText::new(current_parent_name).size(FONT_CAPTION))
                        .width(ui.available_width() - 8.0)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(inspector.new_tag_parent_id.is_none(), "None (Top Level)").clicked() {
                                inspector.new_tag_parent_id = None;
                            }
                            for tag_rec in &all_tag_records {
                                let is_active = inspector.new_tag_parent_id == Some(tag_rec.id);
                                if ui.selectable_label(is_active, &tag_rec.name).clicked() {
                                    inspector.new_tag_parent_id = Some(tag_rec.id);
                                }
                            }
                        });
                });
            }

            ui.add_space(8.0);
            widgets::toolbar_divider(ui);
            ui.add_space(4.0);

            // 3. Complete Keyword List with Checkboxes (Apply to clip) and Filter Arrow
            ui.label(RichText::new("ALL KEYWORDS (Check to assign)").size(FONT_CAPTION).color(text_muted(ui)).strong());
            ui.add_space(2.0);

            if facets.tag_tree.is_empty() {
                ui.label(RichText::new("No keywords in catalog yet.").size(FONT_SMALL).color(text_hint(ui)));
            } else {
                for node in &facets.tag_tree {
                    if let Some(act) = Self::render_inspector_keyword_node(ui, node, vid, db, 0) {
                        action = Some(act);
                    }
                }
            }
        });
        action
    }

    /// Renders a hierarchical keyword item inside the Inspector with:
    /// - Indentation based on tree depth
    /// - Checkbox toggle (adds / removes keyword on active selection)
    /// - Keyword name label
    /// - Right-side arrow button (quickly filter catalog by this keyword)
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

        let has_children = !node.children.is_empty();
        let indent = depth as f32 * 14.0;
        let mut is_checked = vid.tags.contains(&node.name);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if indent > 0.0 {
                ui.add_space(indent);
            }

            // Expand / Collapse Caret
            if has_children {
                let caret = if is_expanded { icon::CARET_DOWN } else { icon::CARET_RIGHT };
                if ui.add(egui::Label::new(RichText::new(caret).size(11.0).color(text_muted(ui))).sense(Sense::click())).clicked() {
                    is_expanded = !is_expanded;
                    ui.data_mut(|d| d.insert_temp(id, is_expanded));
                }
            } else {
                ui.add_space(10.0);
            }

            // Checkbox to toggle keyword on active clip
            let cb_resp = ui.checkbox(&mut is_checked, "");
            if cb_resp.changed() {
                if is_checked {
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

            // Keyword name label
            let text_color = if is_checked { text_primary(ui) } else { text_secondary(ui) };
            let lbl_resp = ui.add(egui::Label::new(RichText::new(&node.name).size(FONT_BODY).color(text_color)).sense(Sense::click()));
            if lbl_resp.clicked() {
                // Clicking label also toggles the checkbox
                if !is_checked {
                    vid.tags.push(node.name.clone());
                    let _ = db.add_tag(vid.id, &node.name);
                    action = Some(InspectorAction::BatchAddTag(node.name.clone()));
                } else {
                    vid.tags.retain(|t| t != &node.name);
                    let _ = db.remove_tag(vid.id, &node.name);
                    action = Some(InspectorAction::BatchRemoveTag(node.name.clone()));
                }
            }

            // Right-aligned Filter Arrow Button (Lightroom style filter arrow)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let filter_btn = ui.add(
                    egui::Button::new(RichText::new(icon::ARROW_RIGHT).size(12.0).color(text_muted(ui)))
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(Stroke::NONE),
                )
                .on_hover_text(format!("Filter catalog by keyword '{}'", node.name));

                if filter_btn.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if filter_btn.clicked() {
                    action = Some(InspectorAction::FilterByTag(node.name.clone()));
                }
            });
        });

        // Recursively render child keywords if expanded
        if has_children && is_expanded {
            for child in &node.children {
                if let Some(sub_act) = Self::render_inspector_keyword_node(ui, child, vid, db, depth + 1) {
                    action = Some(sub_act);
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
