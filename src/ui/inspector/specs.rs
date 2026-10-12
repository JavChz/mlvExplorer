use crate::db::{Database, VideoRecord};
use crate::ui::theme::*;
use egui::{Color32, RichText, Ui};

pub struct TechnicalSpecs;

impl TechnicalSpecs {
    pub fn show(ui: &mut Ui, vid: &mut VideoRecord, db: &Database) {
        // Camera & Lens Metadata
        ui.group(|ui| {
            ui.label(RichText::new("CAMERA & OPTICS").strong().size(11.0).color(TEXT_SECONDARY));
            ui.add_space(4.0);

            egui::Grid::new("inspector_camera_grid").num_columns(2).striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                ui.label("Camera:");
                ui.label(if vid.camera_name.is_empty() { "Unknown" } else { &vid.camera_name });
                ui.end_row();

                if !vid.camera_serial.is_empty() {
                    ui.label("Serial:");
                    ui.label(&vid.camera_serial);
                    ui.end_row();
                }

                ui.label("Lens:");
                ui.label(if vid.lens_name.is_empty() { "Manual / Uncommunicated" } else { &vid.lens_name });
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

        // Technical Video Specifications
        ui.group(|ui| {
            ui.label(RichText::new("VIDEO SPECIFICATIONS").strong().size(11.0).color(TEXT_SECONDARY));
            ui.add_space(4.0);

            egui::Grid::new("inspector_specs_grid").num_columns(2).striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                ui.label("Resolution:");
                ui.label(format!("{} × {}", vid.width, vid.height));
                ui.end_row();

                ui.label("Aspect Stretch:");
                let cur_s = vid.stretch_factor;
                let s_label = match cur_s {
                    s if (s - 3.0).abs() < 0.05 => "3.0x (1x3 / 3x1 Anamorphic)",
                    s if (s - 2.0).abs() < 0.05 => "2.0x Anamorphic",
                    s if (s - 1.75).abs() < 0.05 => "1.75x Anamorphic",
                    s if (s - 1.5).abs() < 0.05 => "1.5x Anamorphic",
                    s if (s - 1.333).abs() < 0.05 => "1.33x Anamorphic",
                    _ => "1.0x (Square)",
                };
                egui::ComboBox::from_id_salt(format!("insp_stretch_{}", vid.id))
                    .selected_text(s_label)
                    .show_ui(ui, |ui| {
                        for (factor, label) in [
                            (1.0f32, "1.0x (1:1 Square)"),
                            (3.0f32, "3.0x (1x3 / 3x1 Anamorphic / EOS M Crop Mood)"),
                            (2.0f32, "2.0x (Standard 2x Anamorphic)"),
                            (1.75f32, "1.75x Anamorphic"),
                            (1.5f32, "1.5x Anamorphic"),
                            (1.333f32, "1.33x Anamorphic"),
                        ] {
                            let is_sel = (cur_s - factor).abs() < 0.02;
                            if ui.selectable_label(is_sel, label).clicked() {
                                vid.stretch_factor = factor;
                                let _ = db.set_stretch_factor(vid.id, factor);
                            }
                        }
                    });
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
                if vid.chunk_count > 1 {
                    ui.label(
                        RichText::new(format!("{} files (⚡ 5D3 Dual-Card Spanned)", vid.chunk_count))
                            .color(Color32::from_rgb(245, 158, 11))
                            .strong(),
                    );
                } else {
                    ui.label("1 file (Standard Single Card)");
                }
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
    }
}

pub struct LocationSection;

impl LocationSection {
    pub fn show(ui: &mut Ui, file_path: &str) {
        ui.group(|ui| {
            ui.label(RichText::new("LOCATION").strong().size(11.0).color(TEXT_SECONDARY));
            ui.label(RichText::new(file_path).monospace().small().color(TEXT_MUTED));
            ui.add_space(4.0);

            if ui.button("📂 Show in Explorer").clicked() {
                if let Some(parent) = std::path::Path::new(file_path).parent() {
                    let _ = open::that(parent);
                }
            }
        });
    }
}
