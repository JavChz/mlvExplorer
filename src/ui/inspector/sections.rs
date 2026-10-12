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
                    egui::Label::new(RichText::new(&vid.file_name).size(FONT_TITLE).strong().color(TEXT_PRIMARY))
                        .truncate(),
                );
                let mut summary = format!("{} × {}  ·  {}", vid.width, vid.height, widgets::format_fps(vid.fps));
                if !vid.camera_name.is_empty() {
                    summary = format!("{}  ·  {}", vid.camera_name, summary);
                }
                ui.add(egui::Label::new(RichText::new(summary).size(FONT_SMALL).color(TEXT_SECONDARY)).truncate());
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
        let trailing = vid.slate_label().map(|s| RichText::new(s).size(FONT_CAPTION).color(TEXT_MUTED).into());

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
        .fill(BG_SECTION)
        .stroke(Stroke::new(1.0_f32, BORDER_STRONG))
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(egui::Margin { left: 8.0, right: 5.0, top: 2.0, bottom: 2.0 })
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.horizontal(|ui| {
                ui.label(RichText::new(text).size(FONT_SMALL).color(TEXT_PRIMARY));
                let x = ui
                    .add(egui::Label::new(RichText::new(icon::X).size(10.0).color(TEXT_MUTED)).sense(Sense::click()))
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
        all_tags: &[String],
        new_tag_input: &mut String,
    ) -> Option<InspectorAction> {
        let mut action = None;
        let trailing = (!vid.tags.is_empty())
            .then(|| RichText::new(vid.tags.len().to_string()).size(FONT_CAPTION).color(TEXT_MUTED).into());

        widgets::section(ui, "Keywords", true, trailing, |ui| {
            // Existing keywords
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
                ui.add_space(6.0);
            }

            // Add keyword input (Enter to commit)
            let mut commit = false;
            ui.horizontal(|ui| {
                let add_w = CONTROL_HEIGHT + 2.0;
                let resp = ui.add_sized(
                    [ui.available_width() - add_w - ui.spacing().item_spacing.x, CONTROL_HEIGHT],
                    egui::TextEdit::singleline(new_tag_input)
                        .hint_text(widgets::hint("Add keyword…"))
                        .margin(egui::vec2(6.0, 3.0)),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    commit = true;
                    resp.request_focus();
                }
                let can_add = !new_tag_input.trim().is_empty();
                ui.add_enabled_ui(can_add, |ui| {
                    if widgets::icon_button(ui, icon::PLUS, false, "Add keyword [Enter]").clicked() {
                        commit = true;
                    }
                });
            });
            if commit && !new_tag_input.trim().is_empty() {
                let tag = new_tag_input.trim().to_string();
                if !vid.tags.contains(&tag) {
                    vid.tags.push(tag.clone());
                    let _ = db.add_tag(vid.id, &tag);
                    action = Some(InspectorAction::BatchAddTag(tag));
                }
                new_tag_input.clear();
            }

            // Recently used keywords (quick add)
            let unused: Vec<&String> = all_tags.iter().filter(|t| !vid.tags.contains(t)).take(8).collect();
            if !unused.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new("Recent").size(FONT_CAPTION).color(TEXT_MUTED));
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                    for t in unused {
                        let b = egui::Button::new(
                            RichText::new(format!("{} {}", icon::PLUS, t)).size(FONT_SMALL).color(TEXT_SECONDARY),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                        .rounding(egui::Rounding::same(10.0));
                        if ui.add(b).clicked() {
                            vid.tags.push(t.clone());
                            let _ = db.add_tag(vid.id, t);
                            action = Some(InspectorAction::BatchAddTag(t.clone()));
                        }
                    }
                });
            }
        });
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
