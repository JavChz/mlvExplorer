pub mod sections;
pub mod specs;
pub mod types;

pub use sections::{NotesSection, OrganizationSection, ProductionSlateSection, TagsSection};
pub use specs::{LocationSection, TechnicalSpecs};
pub use types::{InspectorAction, Inspector};

use crate::db::{Database, VideoRecord};
use crate::ui::theme::*;
use crate::ui::widgets::icon;
use egui::{Align2, FontId, Pos2, RichText, Rounding, Sense, Stroke, Ui, Vec2};

impl Inspector {
    pub fn show(
        &mut self,
        ui: &mut Ui,
        video: Option<&mut VideoRecord>,
        selected_count: usize,
        db: &Database,
        facets: &crate::db::Facets,
    ) -> Option<InspectorAction> {
        let mut action = None;

        // Panel title bar
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), TOOLBAR_HEIGHT), Sense::hover());
        let painter = ui.painter();
        painter.text(
            Pos2::new(rect.min.x + PANEL_PADDING, rect.center().y),
            Align2::LEFT_CENTER,
            "Metadata",
            FontId::proportional(FONT_BODY + 0.5),
            TEXT_PRIMARY,
        );
        if selected_count > 1 {
            crate::ui::widgets::paint_badge(
                painter,
                Pos2::new(rect.max.x - PANEL_PADDING, rect.center().y),
                Align2::RIGHT_CENTER,
                &format!("{selected_count} selected"),
                FontId::proportional(FONT_CAPTION),
                egui::Color32::WHITE,
                ACCENT_DIM,
            );
        }
        painter.line_segment([rect.left_bottom(), rect.right_bottom()], Stroke::new(1.0_f32, BORDER_SUBTLE));

        let Some(vid) = video else {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(icon::FILM_STRIP).size(32.0).color(TEXT_DISABLED));
                ui.add_space(6.0);
                ui.label(RichText::new("No clip selected").color(TEXT_MUTED));
                ui.label(RichText::new("Select a clip to view its metadata").size(FONT_SMALL).color(TEXT_HINT));
            });
            return None;
        };

        if selected_count > 1 {
            egui::Frame::none()
                .fill(ACCENT_DIM.gamma_multiply(0.55))
                .inner_margin(egui::Margin::symmetric(PANEL_PADDING, 6.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(icon::INFO).color(ACCENT_HOVER));
                        ui.label(
                            RichText::new(format!(
                                "Batch edit: changes apply to all {selected_count} selected clips."
                            ))
                            .size(FONT_SMALL)
                            .color(TEXT_PRIMARY),
                        );
                    });
                });
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;

                if let Some(a) = OrganizationSection::show(ui, vid, db) {
                    action = Some(a);
                }
                if let Some(a) = ProductionSlateSection::show(ui, vid, db) {
                    action = Some(a);
                }
                if let Some(a) = TagsSection::show(ui, vid, db, facets, self) {
                    action = Some(a);
                }
                if let Some(a) = NotesSection::show(ui, vid, db) {
                    action = Some(a);
                }
                if let Some(a) = TechnicalSpecs::show(ui, vid, db) {
                    action = Some(a);
                }
                LocationSection::show(ui, &vid.file_path);

                // Bottom rule closing the last section
                let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
                ui.painter().rect_filled(r, Rounding::ZERO, BORDER_SUBTLE);
            });

        action
    }
}
