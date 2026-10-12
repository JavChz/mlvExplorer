use crate::db::{Database, VideoRecord};
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{RichText, Ui, Vec2};

pub struct TechnicalSpecs;

impl TechnicalSpecs {
    pub fn show(ui: &mut Ui, vid: &mut VideoRecord, db: &Database) -> Option<crate::ui::inspector::InspectorAction> {
        let mut action = None;
        let dash = || RichText::new("—").color(TEXT_MUTED);

        // Camera & lens metadata
        widgets::section(ui, "Camera", true, None, |ui| {
            if vid.camera_name.is_empty() {
                widgets::prop_row(ui, "Camera", dash());
            } else {
                widgets::prop_row(ui, "Camera", vid.camera_name.as_str());
            }
            if !vid.camera_serial.is_empty() {
                widgets::prop_row(ui, "Serial #", vid.camera_serial.as_str());
            }
            if vid.lens_name.is_empty() {
                widgets::prop_row(ui, "Lens", RichText::new("Manual lens").color(TEXT_MUTED));
            } else {
                widgets::prop_row(ui, "Lens", vid.lens_name.as_str());
            }
            if vid.focal_length_mm > 0 {
                widgets::prop_row(ui, "Focal Length", format!("{} mm", vid.focal_length_mm));
            } else {
                widgets::prop_row(ui, "Focal Length", dash());
            }
            if vid.aperture_fstop > 0.0 {
                widgets::prop_row(ui, "Aperture", format!("f/{:.1}", vid.aperture_fstop));
            } else {
                widgets::prop_row(ui, "Aperture", dash());
            }
            widgets::prop_row(ui, "Shutter", vid.shutter_str());
            if vid.iso > 0 {
                widgets::prop_row(ui, "ISO", vid.iso.to_string());
            } else {
                widgets::prop_row(ui, "ISO", dash());
            }
            if vid.white_balance_kelvin > 0 {
                widgets::prop_row(ui, "White Balance", format!("{} K", vid.white_balance_kelvin));
            } else {
                widgets::prop_row(ui, "White Balance", dash());
            }
        });

        // Clip / media attributes
        widgets::section(ui, "Clip Attributes", true, None, |ui| {
            widgets::prop_row(ui, "Resolution", format!("{} × {}", vid.width, vid.height));

            widgets::field_row(ui, "Desqueeze", |ui| {
                let w = ui.available_width();
                if let Some(factor) = widgets::desqueeze_combo(ui, ("insp_desqueeze", vid.id), vid.stretch_factor, w - 8.0) {
                    vid.stretch_factor = factor;
                    let _ = db.set_stretch_factor(vid.id, factor);
                    action = Some(crate::ui::inspector::InspectorAction::BatchSetStretch(factor));
                }
            });

            widgets::prop_row(ui, "Frame Rate", widgets::format_fps(vid.fps));
            widgets::prop_row(ui, "Duration", widgets::timecode(vid.frame_count as usize, vid.fps));
            widgets::prop_row(ui, "Frames", vid.frame_count.to_string());
            widgets::prop_row(ui, "Bit Depth", format!("{}-bit", vid.bits_per_pixel));
            if !vid.codec.is_empty() {
                widgets::prop_row(ui, "Format", vid.codec.as_str());
            }
            if vid.has_audio {
                widgets::prop_row(ui, "Audio", "Linear PCM (WAV)");
            } else {
                widgets::prop_row(ui, "Audio", RichText::new("None").color(TEXT_MUTED));
            }
            if vid.chunk_count > 1 {
                widgets::field_row(ui, "Spanned Media", |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("{} Spanned ({} files)", icon::STACK, vid.chunk_count))
                                .color(WARNING),
                        )
                        .truncate(),
                    )
                    .on_hover_text(format!(
                        "Recording split across {} files (.MLV, .M00…)\nDynamically interleaved and unified as a single clip.",
                        vid.chunk_count
                    ));
                });
            } else {
                widgets::prop_row(ui, "Spanned Media", "Single file");
            }
            widgets::prop_row(ui, "File Size", widgets::format_bytes(vid.file_size));
            if let Some(ref d) = vid.date_recorded {
                widgets::prop_row(ui, "Date Recorded", d.as_str());
            }
        });
        action
    }
}

pub struct LocationSection;

impl LocationSection {
    pub fn show(ui: &mut Ui, file_path: &str) {
        widgets::section(ui, "File", true, None, |ui| {
            ui.add(
                egui::Label::new(RichText::new(file_path).monospace().size(FONT_CAPTION).color(TEXT_SECONDARY))
                    .wrap(),
            );
            ui.add_space(6.0);

            let label = if cfg!(target_os = "macos") {
                "Reveal in Finder"
            } else if cfg!(target_os = "windows") {
                "Show in Explorer"
            } else {
                "Show in File Manager"
            };
            let btn = egui::Button::new(widgets::icon_label(icon::FOLDER_OPEN, label))
                .min_size(Vec2::new(ui.available_width(), CONTROL_HEIGHT + 2.0));
            if ui.add(btn).clicked() {
                if let Some(parent) = std::path::Path::new(file_path).parent() {
                    let _ = open::that(parent);
                }
            }
        });
    }
}
