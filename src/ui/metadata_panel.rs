use crate::db::{Database, PickFlag, VideoRecord};
use egui::{Color32, Ui};

pub struct MetadataPanel {
    pub new_tag_input: String,
}

impl Default for MetadataPanel {
    fn default() -> Self {
        Self {
            new_tag_input: String::new(),
        }
    }
}

impl MetadataPanel {
    pub fn show(
        &mut self,
        ui: &mut Ui,
        video: Option<&mut VideoRecord>,
        db: &Database,
    ) {
        ui.heading("Metadata & Organization");
        ui.separator();

        let Some(video) = video else {
            ui.label(egui::RichText::new("No video selected").italics().color(Color32::GRAY));
            return;
        };

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Rating and Flags
            ui.group(|ui| {
                ui.label(egui::RichText::new("Library Organization").strong());
                
                // Rating stars
                ui.horizontal(|ui| {
                    ui.label("Rating:");
                    for star in 1..=5 {
                        let star_str = if star <= video.rating { "★" } else { "☆" };
                        if ui.button(star_str).clicked() {
                            let new_rating = if video.rating == star { 0 } else { star };
                            video.rating = new_rating;
                            let _ = db.set_rating(video.id, new_rating);
                        }
                    }
                    if video.rating > 0 && ui.button("Clear").clicked() {
                        video.rating = 0;
                        let _ = db.set_rating(video.id, 0);
                    }
                });

                // Pick / Reject Flags
                ui.horizontal(|ui| {
                    ui.label("Flag:");
                    let is_picked = video.flag == PickFlag::Picked;
                    let is_rejected = video.flag == PickFlag::Rejected;
                    let is_unflagged = video.flag == PickFlag::Unflagged;

                    if ui.selectable_label(is_picked, "🏳 Pick").clicked() {
                        video.flag = PickFlag::Picked;
                        let _ = db.set_flag(video.id, PickFlag::Picked);
                    }
                    if ui.selectable_label(is_rejected, "❌ Reject").clicked() {
                        video.flag = PickFlag::Rejected;
                        let _ = db.set_flag(video.id, PickFlag::Rejected);
                    }
                    if ui.selectable_label(is_unflagged, "⚪ Unflag").clicked() {
                        video.flag = PickFlag::Unflagged;
                        let _ = db.set_flag(video.id, PickFlag::Unflagged);
                    }
                });
            });

            ui.add_space(8.0);

            // Tags Section
            ui.group(|ui| {
                ui.label(egui::RichText::new("Tags").strong());
                
                // Display current tags as chips
                ui.horizontal_wrapped(|ui| {
                    let mut tag_to_remove: Option<String> = None;
                    for tag in &video.tags {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("#{tag}")).color(Color32::from_rgb(100, 180, 255)));
                            if ui.small_button("×").clicked() {
                                tag_to_remove = Some(tag.clone());
                            }
                        });
                    }
                    if let Some(tag) = tag_to_remove {
                        let _ = db.remove_tag(video.id, &tag);
                        video.tags.retain(|t| t != &tag);
                    }
                });

                // Add new tag input
                ui.horizontal(|ui| {
                    let text_edit = ui.add(
                        egui::TextEdit::singleline(&mut self.new_tag_input)
                            .hint_text("Add tag...")
                            .desired_width(120.0),
                    );
                    let add_clicked = ui.button("+ Add").clicked();
                    let enter_pressed = text_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    if (add_clicked || enter_pressed) && !self.new_tag_input.trim().is_empty() {
                        let tag = self.new_tag_input.trim().to_string();
                        if !video.tags.contains(&tag) {
                            let _ = db.add_tag(video.id, &tag);
                            video.tags.push(tag);
                        }
                        self.new_tag_input.clear();
                    }
                });
            });

            ui.add_space(8.0);

            // Notes Section
            ui.group(|ui| {
                ui.label(egui::RichText::new("Notes").strong());
                let notes_edit = ui.add(
                    egui::TextEdit::multiline(&mut video.notes)
                        .hint_text("Add scene, take, or shot notes...")
                        .desired_width(f32::INFINITY)
                        .desired_rows(3),
                );
                if notes_edit.changed() {
                    let _ = db.set_notes(video.id, &video.notes);
                }
            });

            ui.add_space(8.0);

            // Video & Shot EXIF/MLV Details
            ui.group(|ui| {
                ui.label(egui::RichText::new("Camera & Shot Details").strong());
                egui::Grid::new("meta_camera_grid").num_columns(2).striped(true).show(ui, |ui| {
                    ui.label("Camera Model:");
                    ui.label(&video.camera_name);
                    ui.end_row();

                    if !video.camera_serial.is_empty() {
                        ui.label("Serial Number:");
                        ui.label(&video.camera_serial);
                        ui.end_row();
                    }

                    ui.label("Lens:");
                    ui.label(&video.lens_name);
                    ui.end_row();

                    if video.focal_length_mm > 0 {
                        ui.label("Focal Length:");
                        ui.label(format!("{} mm", video.focal_length_mm));
                        ui.end_row();
                    }

                    if video.aperture_fstop > 0.0 {
                        ui.label("Aperture:");
                        ui.label(format!("f/{:.1}", video.aperture_fstop));
                        ui.end_row();
                    }

                    ui.label("ISO:");
                    ui.label(format!("{}", video.iso));
                    ui.end_row();

                    if video.shutter_speed > 0 {
                        ui.label("Shutter Speed:");
                        ui.label(format!("1/{}s", 1_000_000 / video.shutter_speed.max(1)));
                        ui.end_row();
                    }

                    if video.white_balance_kelvin > 0 {
                        ui.label("White Balance:");
                        ui.label(format!("{} K", video.white_balance_kelvin));
                        ui.end_row();
                    }
                });
            });

            ui.add_space(8.0);

            // Video Specs
            ui.group(|ui| {
                ui.label(egui::RichText::new("Video Specifications").strong());
                egui::Grid::new("meta_video_specs_grid").num_columns(2).striped(true).show(ui, |ui| {
                    ui.label("Resolution:");
                    ui.label(format!("{} × {}", video.width, video.height));
                    ui.end_row();

                    ui.label("Frame Rate:");
                    ui.label(format!("{:.3} fps", video.fps));
                    ui.end_row();

                    ui.label("Total Frames:");
                    ui.label(format!("{}", video.frame_count));
                    ui.end_row();

                    ui.label("Duration:");
                    ui.label(format!("{:.2} s", video.duration_seconds));
                    ui.end_row();

                    ui.label("Bit Depth:");
                    ui.label(format!("{}-bit RAW", video.bits_per_pixel));
                    ui.end_row();

                    ui.label("File Size:");
                    ui.label(format!("{:.2} MB", video.file_size as f64 / 1_048_576.0));
                    ui.end_row();

                    if let Some(ref date) = video.date_recorded {
                        ui.label("Recorded:");
                        ui.label(date);
                        ui.end_row();
                    }
                });
            });

            ui.add_space(8.0);

            // File Location
            ui.group(|ui| {
                ui.label(egui::RichText::new("File Location").strong());
                ui.label(egui::RichText::new(&video.file_path).monospace().small());
                if ui.button("📂 Open Containing Folder").clicked() {
                    if let Some(parent) = std::path::Path::new(&video.file_path).parent() {
                        let _ = open::that(parent);
                    }
                }
            });
        });
    }
}
