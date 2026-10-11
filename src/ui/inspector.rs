use crate::db::{Database, PickFlag, VideoRecord};
use crate::ui::theme::*;
use egui::{Color32, RichText, Ui};

pub struct Inspector {
    pub new_tag_input: String,
}

impl Default for Inspector {
    fn default() -> Self {
        Self {
            new_tag_input: String::new(),
        }
    }
}

impl Inspector {
    pub fn show(
        &mut self,
        ui: &mut Ui,
        video: Option<&mut VideoRecord>,
        db: &Database,
        all_tags: &[String],
    ) {
        ui.label(RichText::new("METADATA & TAGS").extra_letter_spacing(1.2).strong().size(11.0).color(TEXT_MUTED));
        ui.add_space(4.0);
        ui.separator();

        let Some(vid) = video else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No clip selected").italics().color(TEXT_MUTED));
            });
            return;
        };

        egui::ScrollArea::vertical().show(ui, |ui| {
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

            ui.add_space(8.0);

            // Section 2: Tags
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
                        let _ = db.remove_tag(vid.id, &tag_name);
                        vid.tags.retain(|t| t != &tag_name);
                    }
                });

                ui.add_space(4.0);

                // Add Tag input
                ui.horizontal(|ui| {
                    let text_edit = ui.add(
                        egui::TextEdit::singleline(&mut self.new_tag_input)
                            .hint_text("New tag...")
                            .desired_width(120.0),
                    );
                    let add_clicked = ui.button("+ Add").clicked();
                    let enter_pressed = text_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    if (add_clicked || enter_pressed) && !self.new_tag_input.trim().is_empty() {
                        let clean_tag = self.new_tag_input.trim().to_string();
                        if !vid.tags.contains(&clean_tag) {
                            let _ = db.add_tag(vid.id, &clean_tag);
                            vid.tags.push(clean_tag);
                        }
                        self.new_tag_input.clear();
                    }
                });

                // Tag suggestions from existing catalog
                let unused_tags: Vec<&String> = all_tags.iter().filter(|t| !vid.tags.contains(t)).take(6).collect();
                if !unused_tags.is_empty() {
                    ui.add_space(2.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("Suggestions:").small().color(TEXT_MUTED));
                        for st in unused_tags {
                            if ui.small_button(format!("+{}", st)).clicked() {
                                let _ = db.add_tag(vid.id, st);
                                vid.tags.push(st.clone());
                            }
                        }
                    });
                }
            });

            ui.add_space(8.0);

            // Section 3: Notes
            ui.group(|ui| {
                ui.label(RichText::new("NOTES").strong().size(11.0).color(TEXT_SECONDARY));
                let notes_edit = ui.add(
                    egui::TextEdit::multiline(&mut vid.notes)
                        .hint_text("Shot description, take notes, scene...")
                        .desired_width(f32::INFINITY)
                        .desired_rows(3),
                );
                if notes_edit.changed() {
                    let _ = db.set_notes(vid.id, &vid.notes);
                }
            });

            ui.add_space(8.0);

            // Section 4: Camera & Shot Details
            ui.group(|ui| {
                ui.label(RichText::new("CAMERA & SHOT").strong().size(11.0).color(TEXT_SECONDARY));
                ui.add_space(4.0);

                egui::Grid::new("inspector_cam_grid").num_columns(2).striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                    ui.label("Camera:");
                    ui.label(RichText::new(&vid.camera_name).strong());
                    ui.end_row();

                    if !vid.camera_serial.is_empty() {
                        ui.label("Serial:");
                        ui.label(&vid.camera_serial);
                        ui.end_row();
                    }

                    ui.label("Lens:");
                    ui.label(&vid.lens_name);
                    ui.end_row();

                    if vid.focal_length_mm > 0 {
                        ui.label("Focal Length:");
                        ui.label(format!("{} mm", vid.focal_length_mm));
                        ui.end_row();
                    }

                    if vid.aperture_fstop > 0.0 {
                        ui.label("Aperture:");
                        ui.label(format!("f/{:.1}", vid.aperture_fstop));
                        ui.end_row();
                    }

                    ui.label("Shutter:");
                    ui.label(vid.shutter_str());
                    ui.end_row();

                    ui.label("ISO:");
                    ui.label(format!("{}", vid.iso));
                    ui.end_row();

                    if vid.white_balance_kelvin > 0 {
                        ui.label("White Balance:");
                        ui.label(format!("{} K", vid.white_balance_kelvin));
                        ui.end_row();
                    }
                });
            });

            ui.add_space(8.0);

            // Section 5: Technical Video Specifications
            ui.group(|ui| {
                ui.label(RichText::new("VIDEO SPECIFICATIONS").strong().size(11.0).color(TEXT_SECONDARY));
                ui.add_space(4.0);

                egui::Grid::new("inspector_specs_grid").num_columns(2).striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                    ui.label("Resolution:");
                    ui.label(format!("{} × {}", vid.width, vid.height));
                    ui.end_row();

                    ui.label("Frame Rate:");
                    ui.label(format!("{:.3} fps", vid.fps));
                    ui.end_row();

                    ui.label("Total Frames:");
                    ui.label(format!("{}", vid.frame_count));
                    ui.end_row();

                    ui.label("Duration:");
                    ui.label(format!("{:.2}s ({})", vid.duration_seconds, vid.duration_tc()));
                    ui.end_row();

                    ui.label("Format:");
                    ui.label(&vid.codec);
                    ui.end_row();

                    ui.label("Audio Track:");
                    ui.label(if vid.has_audio { "Yes (WAV/Linear PCM)" } else { "No Audio" });
                    ui.end_row();

                    ui.label("Spanned Chunks:");
                    ui.label(format!("{} file(s)", vid.chunk_count));
                    ui.end_row();

                    ui.label("File Size:");
                    ui.label(format!("{:.2} MB", vid.file_size as f64 / 1_048_576.0));
                    ui.end_row();

                    if let Some(ref d) = vid.date_recorded {
                        ui.label("Captured:");
                        ui.label(d);
                        ui.end_row();
                    }
                });
            });

            ui.add_space(8.0);

            // Section 6: File Location
            ui.group(|ui| {
                ui.label(RichText::new("LOCATION").strong().size(11.0).color(TEXT_SECONDARY));
                ui.label(RichText::new(&vid.file_path).monospace().small().color(TEXT_MUTED));
                ui.add_space(4.0);

                if ui.button("📂 Show in Explorer").clicked() {
                    if let Some(parent) = std::path::Path::new(&vid.file_path).parent() {
                        let _ = open::that(parent);
                    }
                }
            });
        });
    }
}
