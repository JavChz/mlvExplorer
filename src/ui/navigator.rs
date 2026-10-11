use crate::db::{Facets, FilterCriteria, PickFlag};
use crate::ui::theme::*;
use egui::{CollapsingHeader, Color32, RichText, Ui};

pub struct Navigator;

impl Navigator {
    pub fn show(
        ui: &mut Ui,
        filter: &mut FilterCriteria,
        facets: &Facets,
    ) -> bool {
        let mut changed = false;

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Section: CATALOG
            ui.label(RichText::new("CATALOG").extra_letter_spacing(1.2).strong().size(11.0).color(TEXT_MUTED));
            ui.add_space(4.0);

            // All Clips
            let is_all = filter.flag.is_none() && filter.camera.is_none() && filter.lens.is_none() && filter.tag.is_none();
            let all_label = format!("📂 All Clips ({})", facets.total);
            if ui.selectable_label(is_all, all_label).clicked() {
                filter.flag = None;
                filter.camera = None;
                filter.lens = None;
                filter.tag = None;
                changed = true;
            }

            // Flagged / Picks
            let is_picked = filter.flag == Some(PickFlag::Picked);
            let pick_label = format!("🏳 Picks ({})", facets.picked);
            if ui.selectable_label(is_picked, RichText::new(pick_label).color(FLAG_PICKED)).clicked() {
                filter.flag = if is_picked { None } else { Some(PickFlag::Picked) };
                changed = true;
            }

            // Rejected
            let is_rejected = filter.flag == Some(PickFlag::Rejected);
            let rej_label = format!("❌ Rejected ({})", facets.rejected);
            if ui.selectable_label(is_rejected, RichText::new(rej_label).color(FLAG_REJECTED)).clicked() {
                filter.flag = if is_rejected { None } else { Some(PickFlag::Rejected) };
                changed = true;
            }

            // Unflagged
            let is_unflagged = filter.flag == Some(PickFlag::Unflagged);
            let unf_label = format!("⚪ Unflagged ({})", facets.unflagged);
            if ui.selectable_label(is_unflagged, unf_label).clicked() {
                filter.flag = if is_unflagged { None } else { Some(PickFlag::Unflagged) };
                changed = true;
            }

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(8.0);

            // Section: CAMERAS
            CollapsingHeader::new(RichText::new("CAMERAS").extra_letter_spacing(1.2).strong().size(11.0).color(TEXT_MUTED))
                .default_open(true)
                .show(ui, |ui| {
                    if facets.cameras.is_empty() {
                        ui.label(RichText::new("No cameras found").small().color(TEXT_MUTED));
                    }
                    for cam in &facets.cameras {
                        let is_sel = filter.camera.as_ref() == Some(&cam.name);
                        let label = format!("📷 {} ({})", cam.name, cam.count);
                        if ui.selectable_label(is_sel, label).clicked() {
                            filter.camera = if is_sel { None } else { Some(cam.name.clone()) };
                            changed = true;
                        }
                    }
                });

            ui.add_space(10.0);

            // Section: LENSES
            CollapsingHeader::new(RichText::new("LENSES").extra_letter_spacing(1.2).strong().size(11.0).color(TEXT_MUTED))
                .default_open(true)
                .show(ui, |ui| {
                    if facets.lenses.is_empty() {
                        ui.label(RichText::new("No lenses found").small().color(TEXT_MUTED));
                    }
                    for lens in &facets.lenses {
                        let is_sel = filter.lens.as_ref() == Some(&lens.name);
                        let label = format!("🔍 {} ({})", lens.name, lens.count);
                        if ui.selectable_label(is_sel, label).clicked() {
                            filter.lens = if is_sel { None } else { Some(lens.name.clone()) };
                            changed = true;
                        }
                    }
                });

            ui.add_space(10.0);

            // Section: TAGS
            CollapsingHeader::new(RichText::new("TAGS").extra_letter_spacing(1.2).strong().size(11.0).color(TEXT_MUTED))
                .default_open(true)
                .show(ui, |ui| {
                    if facets.tags.is_empty() {
                        ui.label(RichText::new("No tags created yet").small().color(TEXT_MUTED));
                    }
                    for tag in &facets.tags {
                        let is_sel = filter.tag.as_ref() == Some(&tag.name);
                        let label = format!("#{} ({})", tag.name, tag.count);
                        if ui.selectable_label(is_sel, RichText::new(label).color(Color32::from_rgb(96, 165, 250))).clicked() {
                            filter.tag = if is_sel { None } else { Some(tag.name.clone()) };
                            changed = true;
                        }
                    }
                });

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(8.0);

            // Section: SHORTCUTS CHEAT SHEET
            CollapsingHeader::new(RichText::new("KEYBOARD SHORTCUTS").extra_letter_spacing(1.2).strong().size(11.0).color(TEXT_MUTED))
                .default_open(false)
                .show(ui, |ui| {
                    egui::Grid::new("shortcuts_grid").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                        ui.label(RichText::new("G").monospace().strong());
                        ui.label("Grid View");
                        ui.end_row();

                        ui.label(RichText::new("E / Enter").monospace().strong());
                        ui.label("Player View");
                        ui.end_row();

                        ui.label(RichText::new("Space").monospace().strong());
                        ui.label("Play / Pause");
                        ui.end_row();

                        ui.label(RichText::new("Left / Right").monospace().strong());
                        ui.label("Step 1 Frame");
                        ui.end_row();

                        ui.label(RichText::new("1 – 5").monospace().strong());
                        ui.label("Star Rating");
                        ui.end_row();

                        ui.label(RichText::new("0").monospace().strong());
                        ui.label("Clear Rating");
                        ui.end_row();

                        ui.label(RichText::new("P").monospace().strong());
                        ui.label("Flag Pick");
                        ui.end_row();

                        ui.label(RichText::new("X").monospace().strong());
                        ui.label("Flag Reject");
                        ui.end_row();

                        ui.label(RichText::new("U").monospace().strong());
                        ui.label("Unflag");
                        ui.end_row();
                    });
                });
        });

        changed
    }
}
