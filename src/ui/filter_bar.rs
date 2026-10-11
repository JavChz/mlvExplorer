use crate::db::{FilterCriteria, PickFlag};
use egui::Ui;

pub struct FilterBar;

impl FilterBar {
    pub fn show(
        ui: &mut Ui,
        filter: &mut FilterCriteria,
        cameras: &[String],
        lenses: &[String],
    ) -> bool {
        let mut changed = false;

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("🔍 Search:").strong());
            let text_res = ui.add(
                egui::TextEdit::singleline(&mut filter.search_text)
                    .hint_text("File name, camera, lens, notes...")
                    .desired_width(180.0),
            );
            if text_res.changed() {
                changed = true;
            }

            ui.separator();

            // Camera Filter Dropdown
            ui.label("Camera:");
            let cam_selected_label = filter
                .camera
                .clone()
                .unwrap_or_else(|| "All Cameras".to_string());
            egui::ComboBox::from_id_salt("cam_filter")
                .selected_text(cam_selected_label)
                .show_ui(ui, |ui| {
                    if ui.selectable_value(&mut filter.camera, None, "All Cameras").clicked() {
                        changed = true;
                    }
                    for cam in cameras {
                        if ui.selectable_value(&mut filter.camera, Some(cam.clone()), cam).clicked() {
                            changed = true;
                        }
                    }
                });

            // Lens Filter Dropdown
            ui.label("Lens:");
            let lens_selected_label = filter
                .lens
                .clone()
                .unwrap_or_else(|| "All Lenses".to_string());
            egui::ComboBox::from_id_salt("lens_filter")
                .selected_text(lens_selected_label)
                .show_ui(ui, |ui| {
                    if ui.selectable_value(&mut filter.lens, None, "All Lenses").clicked() {
                        changed = true;
                    }
                    for lens in lenses {
                        if ui.selectable_value(&mut filter.lens, Some(lens.clone()), lens).clicked() {
                            changed = true;
                        }
                    }
                });

            ui.separator();

            // Rating Filter
            ui.label("Rating:");
            let current_rating = filter.min_rating.unwrap_or(0);
            for r in 0..=5 {
                let text = if r == 0 { "All".to_string() } else { format!("{}★+", r) };
                let is_selected = (r == 0 && filter.min_rating.is_none()) || filter.min_rating == Some(r);
                if ui.selectable_label(is_selected, text).clicked() {
                    filter.min_rating = if r == 0 { None } else { Some(r) };
                    changed = true;
                }
            }

            ui.separator();

            // Flag Filter
            ui.label("Flag:");
            let flags = [
                ("All", None),
                ("🏳 Picked", Some(PickFlag::Picked)),
                ("❌ Rejected", Some(PickFlag::Rejected)),
                ("⚪ Unflagged", Some(PickFlag::Unflagged)),
            ];
            for (label, flag_val) in flags {
                let is_sel = filter.flag == flag_val;
                if ui.selectable_label(is_sel, label).clicked() {
                    filter.flag = flag_val;
                    changed = true;
                }
            }

            ui.separator();

            // Reset filters button
            if ui.button("✖ Reset").clicked() {
                *filter = FilterCriteria::default();
                changed = true;
            }
        });

        changed
    }
}
