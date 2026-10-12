use super::types::InspectorAction;
use crate::db::{Database, PickFlag, VideoRecord};
use crate::ui::theme::*;
use egui::{Color32, RichText, Ui};

pub struct OrganizationSection;

impl OrganizationSection {
    pub fn show(
        ui: &mut Ui,
        vid: &mut VideoRecord,
        db: &Database,
    ) -> Option<InspectorAction> {
        let mut action = None;

        // Quick Launch MLVApp Button
        ui.vertical_centered_justified(|ui| {
            if ui.button(RichText::new("🎬 Open in MLVApp").strong().color(Color32::from_rgb(243, 244, 246)))
                .on_hover_text("Open this clip directly in MLV App for advanced grading & audio export")
                .clicked()
            {
                action = Some(InspectorAction::OpenInMlvApp);
            }
        });
        ui.add_space(4.0);

        // Section 1: Library Organization (Rating & Flags)
        ui.group(|ui| {
            ui.label(RichText::new("ORGANIZATION").strong().size(11.0).color(TEXT_SECONDARY));
            ui.add_space(4.0);

            // Star Ratings
            ui.horizontal(|ui| {
                ui.label("Rating:");
                for star in 1..=5 {
                    let is_lit = star <= vid.rating;
                    let star_str = if is_lit { "★" } else { "☆" };
                    let star_col = if is_lit { ACCENT_GOLD } else { TEXT_MUTED };
                    if ui.button(RichText::new(star_str).color(star_col).size(14.0)).clicked() {
                        let new_r = if vid.rating == star { 0 } else { star };
                        vid.rating = new_r;
                        let _ = db.set_rating(vid.id, new_r);
                    }
                }
                if vid.rating > 0 && ui.small_button("Clear").clicked() {
                    vid.rating = 0;
                    let _ = db.set_rating(vid.id, 0);
                }
            });

            // Pick / Reject Flags
            ui.horizontal(|ui| {
                ui.label("Flag:");
                let is_picked = vid.flag == PickFlag::Picked;
                if ui.selectable_label(is_picked, RichText::new("🏳 Pick").color(FLAG_PICKED)).clicked() {
                    vid.flag = if is_picked { PickFlag::Unflagged } else { PickFlag::Picked };
                    let _ = db.set_flag(vid.id, vid.flag);
                }

                let is_rej = vid.flag == PickFlag::Rejected;
                if ui.selectable_label(is_rej, RichText::new("❌ Reject").color(FLAG_REJECTED)).clicked() {
                    vid.flag = if is_rej { PickFlag::Unflagged } else { PickFlag::Rejected };
                    let _ = db.set_flag(vid.id, vid.flag);
                }

                let is_unf = vid.flag == PickFlag::Unflagged;
                if ui.selectable_label(is_unf, "⚪ Unflag").clicked() {
                    vid.flag = PickFlag::Unflagged;
                    let _ = db.set_flag(vid.id, PickFlag::Unflagged);
                }
            });
        });

        action
    }
}

pub struct TagsSection;

impl TagsSection {
    pub fn show(
        ui: &mut Ui,
        vid: &mut VideoRecord,
        db: &Database,
        all_tags: &[String],
        new_tag_input: &mut String,
    ) {
        ui.group(|ui| {
            ui.label(RichText::new("TAGS").strong().size(11.0).color(TEXT_SECONDARY));
            ui.add_space(4.0);

            // Existing tags wrapped
            ui.horizontal_wrapped(|ui| {
                let mut to_remove = None;
                for t in &vid.tags {
                    ui.horizontal(|ui| {
                        let pill = RichText::new(format!("#{t}")).color(Color32::from_rgb(96, 165, 250)).size(11.5);
                        ui.label(pill);
                        if ui.small_button("×").clicked() {
                            to_remove = Some(t.clone());
                        }
                    });
                }
                if let Some(tag_name) = to_remove {
                    vid.tags.retain(|t| t != &tag_name);
                    let _ = db.remove_tag(vid.id, &tag_name);
                }
            });

            ui.add_space(4.0);

            // Add tag input
            ui.horizontal(|ui| {
                let text_resp = ui.add(
                    egui::TextEdit::singleline(new_tag_input)
                        .hint_text("Add tag...")
                        .desired_width(120.0),
                );
                let enter_pressed = text_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button("+").clicked() || enter_pressed) && !new_tag_input.trim().is_empty() {
                    let tag = new_tag_input.trim().to_string();
                    if !vid.tags.contains(&tag) {
                        vid.tags.push(tag.clone());
                        let _ = db.add_tag(vid.id, &tag);
                    }
                    new_tag_input.clear();
                }
            });

            // Quick suggestions
            let unused_tags: Vec<&String> = all_tags.iter().filter(|t| !vid.tags.contains(t)).take(6).collect();
            if !unused_tags.is_empty() {
                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("Suggestions:").small().color(TEXT_MUTED));
                    for t in unused_tags {
                        if ui.small_button(format!("+{t}")).clicked() {
                            vid.tags.push(t.clone());
                            let _ = db.add_tag(vid.id, t);
                        }
                    }
                });
            }
        });
    }
}

pub struct NotesSection;

impl NotesSection {
    pub fn show(ui: &mut Ui, vid: &mut VideoRecord, db: &Database) {
        ui.group(|ui| {
            ui.label(RichText::new("NOTES").strong().size(11.0).color(TEXT_SECONDARY));
            ui.add_space(4.0);
            let response = ui.add(
                egui::TextEdit::multiline(&mut vid.notes)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .hint_text("Add clip notes, director comments..."),
            );
            if response.changed() {
                let _ = db.set_notes(vid.id, &vid.notes);
            }
        });
    }
}

pub struct ProductionSlateSection;

impl ProductionSlateSection {
    pub fn show(ui: &mut Ui, vid: &mut VideoRecord, db: &Database) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🎬 PRODUCTION SLATE").strong().size(11.0).color(Color32::from_rgb(245, 158, 11)));
                if let Some(slate) = vid.slate_label() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(slate).small().color(Color32::from_rgb(147, 197, 253)));
                    });
                }
            });
            ui.add_space(4.0);

            let mut changed = false;

            // Row 1: Scene & Shot
            ui.horizontal(|ui| {
                ui.label(RichText::new("Scene:").small().color(TEXT_SECONDARY));
                let mut scene = vid.scene.clone().unwrap_or_default();
                if ui.add_sized([64.0, 18.0], egui::TextEdit::singleline(&mut scene).hint_text("12A")).changed() {
                    vid.scene = if scene.trim().is_empty() { None } else { Some(scene) };
                    changed = true;
                }

                ui.add_space(6.0);
                ui.label(RichText::new("Shot:").small().color(TEXT_SECONDARY));
                let mut shot = vid.shot.clone().unwrap_or_default();
                if ui.add_sized([54.0, 18.0], egui::TextEdit::singleline(&mut shot).hint_text("2")).changed() {
                    vid.shot = if shot.trim().is_empty() { None } else { Some(shot) };
                    changed = true;
                }
            });

            // Row 2: Take & Unit / Camera
            ui.horizontal(|ui| {
                ui.label(RichText::new("Take:").small().color(TEXT_SECONDARY));
                let mut take = vid.take.clone().unwrap_or_default();
                if ui.add_sized([64.0, 18.0], egui::TextEdit::singleline(&mut take).hint_text("1")).changed() {
                    vid.take = if take.trim().is_empty() { None } else { Some(take) };
                    changed = true;
                }

                ui.add_space(6.0);
                ui.label(RichText::new("Unit:").small().color(TEXT_SECONDARY));
                let mut unit = vid.camera_unit.clone().unwrap_or_default();
                if ui.add_sized([54.0, 18.0], egui::TextEdit::singleline(&mut unit).hint_text("A-Cam")).changed() {
                    vid.camera_unit = if unit.trim().is_empty() { None } else { Some(unit) };
                    changed = true;
                }
            });

            // Row 3: Location
            ui.horizontal(|ui| {
                ui.label(RichText::new("Location:").small().color(TEXT_SECONDARY));
                let mut loc = vid.location.clone().unwrap_or_default();
                if ui.add(egui::TextEdit::singleline(&mut loc).hint_text("Stage B / Exterior Woods").desired_width(f32::INFINITY)).changed() {
                    vid.location = if loc.trim().is_empty() { None } else { Some(loc) };
                    changed = true;
                }
            });

            if changed {
                let _ = db.set_production_slate(
                    vid.id,
                    vid.scene.as_deref(),
                    vid.shot.as_deref(),
                    vid.take.as_deref(),
                    vid.location.as_deref(),
                    vid.camera_unit.as_deref(),
                );
            }
        });
    }
}
