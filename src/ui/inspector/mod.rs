pub mod sections;
pub mod specs;
pub mod types;

pub use sections::{NotesSection, OrganizationSection, ProductionSlateSection, TagsSection};
pub use specs::{LocationSection, TechnicalSpecs};
pub use types::{InspectorAction, Inspector};

use crate::db::{Database, VideoRecord};
use crate::ui::theme::*;
use egui::{Color32, RichText, Ui};

impl Inspector {
    pub fn show(
        &mut self,
        ui: &mut Ui,
        video: Option<&mut VideoRecord>,
        selected_count: usize,
        db: &Database,
        all_tags: &[String],
    ) -> Option<InspectorAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new("METADATA & TAGS").extra_letter_spacing(1.2).strong().size(11.0).color(TEXT_MUTED));
            if selected_count > 1 {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} clips selected", selected_count))
                            .small()
                            .strong()
                            .color(Color32::from_rgb(96, 165, 250))
                    );
                });
            }
        });
        ui.add_space(4.0);
        ui.separator();

        let Some(vid) = video else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No clip selected").italics().color(TEXT_MUTED));
            });
            return None;
        };

        if selected_count > 1 {
            ui.label(
                RichText::new("Batch Editing: Changes to rating, flags, tags, notes, slate, or stretch apply to all selected clips.")
                    .small()
                    .color(Color32::from_rgb(245, 158, 11))
            );
            ui.add_space(4.0);
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Organization & Quick Actions (Rating, Pick, MLVApp)
            if let Some(org_act) = OrganizationSection::show(ui, vid, db) {
                action = Some(org_act);
            }

            ui.add_space(8.0);

            // Cinema Production Slate (Scene, Shot, Take, Unit, Location)
            if let Some(slate_act) = ProductionSlateSection::show(ui, vid, db) {
                action = Some(slate_act);
            }

            ui.add_space(8.0);

            // Tags
            if let Some(tag_act) = TagsSection::show(ui, vid, db, all_tags, &mut self.new_tag_input) {
                action = Some(tag_act);
            }

            ui.add_space(8.0);

            // Notes
            if let Some(notes_act) = NotesSection::show(ui, vid, db) {
                action = Some(notes_act);
            }

            ui.add_space(8.0);

            // Camera, Optics & Technical Specs
            if let Some(specs_act) = TechnicalSpecs::show(ui, vid, db) {
                action = Some(specs_act);
            }

            ui.add_space(8.0);

            // File Location
            LocationSection::show(ui, &vid.file_path);
        });

        action
    }
}
