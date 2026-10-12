pub mod sections;
pub mod specs;
pub mod types;

pub use sections::{NotesSection, OrganizationSection, ProductionSlateSection, TagsSection};
pub use specs::{LocationSection, TechnicalSpecs};
pub use types::{InspectorAction, Inspector};

use crate::db::{Database, VideoRecord};
use crate::ui::theme::*;
use egui::{RichText, Ui};

impl Inspector {
    pub fn show(
        &mut self,
        ui: &mut Ui,
        video: Option<&mut VideoRecord>,
        db: &Database,
        all_tags: &[String],
    ) -> Option<InspectorAction> {
        let mut action = None;
        ui.label(RichText::new("METADATA & TAGS").extra_letter_spacing(1.2).strong().size(11.0).color(TEXT_MUTED));
        ui.add_space(4.0);
        ui.separator();

        let Some(vid) = video else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No clip selected").italics().color(TEXT_MUTED));
            });
            return None;
        };

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Organization & Quick Actions (Rating, Pick, MLVApp)
            if let Some(org_act) = OrganizationSection::show(ui, vid, db) {
                action = Some(org_act);
            }

            ui.add_space(8.0);

            // Cinema Production Slate (Scene, Shot, Take, Unit, Location)
            ProductionSlateSection::show(ui, vid, db);

            ui.add_space(8.0);

            // Tags
            TagsSection::show(ui, vid, db, all_tags, &mut self.new_tag_input);

            ui.add_space(8.0);

            // Notes
            NotesSection::show(ui, vid, db);

            ui.add_space(8.0);

            // Camera, Optics & Technical Specs
            TechnicalSpecs::show(ui, vid, db);

            ui.add_space(8.0);

            // File Location
            LocationSection::show(ui, &vid.file_path);
        });

        action
    }
}
