use crate::db::{Facets, FilterCriteria, PickFlag};
use crate::ui::theme::*;
use egui::{CollapsingHeader, Color32, Rect, RichText, Rounding, Stroke, Ui, Vec2};

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
            if Self::nav_row(ui, is_all, "📂", "All Clips", facets.total, None) {
                filter.flag = None;
                filter.camera = None;
                filter.lens = None;
                filter.tag = None;
                changed = true;
            }

            // Flagged / Picks
            let is_picked = filter.flag == Some(PickFlag::Picked);
            if Self::nav_row(ui, is_picked, "🏳", "Picks", facets.picked, Some(FLAG_PICKED)) {
                filter.flag = if is_picked { None } else { Some(PickFlag::Picked) };
                changed = true;
            }

            // Rejected
            let is_rejected = filter.flag == Some(PickFlag::Rejected);
            if Self::nav_row(ui, is_rejected, "❌", "Rejected", facets.rejected, Some(FLAG_REJECTED)) {
                filter.flag = if is_rejected { None } else { Some(PickFlag::Rejected) };
                changed = true;
            }

            // Unflagged
            let is_unflagged = filter.flag == Some(PickFlag::Unflagged);
            if Self::nav_row(ui, is_unflagged, "⚪", "Unflagged", facets.unflagged, None) {
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
                        if Self::nav_row(ui, is_sel, "📷", &cam.name, cam.count, None) {
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
                        if Self::nav_row(ui, is_sel, "🔍", &lens.name, lens.count, None) {
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
                        if Self::nav_row(ui, is_sel, "#", &tag.name, tag.count, Some(Color32::from_rgb(96, 165, 250))) {
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

                        ui.label(RichText::new("Home / End").monospace().strong());
                        ui.label("Start / End Frame");
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

    /// Renders a sleek studio row with icon, title, and right-aligned badge pill
    fn nav_row(
        ui: &mut Ui,
        is_selected: bool,
        icon: &str,
        label: &str,
        count: usize,
        text_color: Option<Color32>,
    ) -> bool {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), egui::Sense::click());
        let painter = ui.painter();

        // Row background
        if is_selected {
            painter.rect_filled(rect, Rounding::same(4.0), BG_CARD_SELECTED);
            painter.rect_stroke(rect, Rounding::same(4.0), Stroke::new(1.0_f32, BORDER_SELECTED));
        } else if response.hovered() {
            painter.rect_filled(rect, Rounding::same(4.0), BG_CARD_HOVER);
        }

        // Left Icon + Label
        let label_col = text_color.unwrap_or(if is_selected { TEXT_PRIMARY } else { Color32::from_rgb(210, 215, 225) });
        let text_y = rect.center().y;
        
        let mut display_label = label;
        if display_label.len() > 18 {
            display_label = &display_label[..18];
        }

        let full_text = format!("{} {}", icon, display_label);
        painter.text(
            egui::pos2(rect.min.x + 8.0, text_y),
            egui::Align2::LEFT_CENTER,
            full_text,
            egui::FontId::proportional(12.0),
            label_col,
        );

        // Right Badge Pill
        let count_str = format!("{}", count);
        let badge_w = (count_str.len() as f32 * 7.5 + 12.0).max(20.0);
        let badge_rect = Rect::from_center_size(
            egui::pos2(rect.max.x - badge_w * 0.5 - 6.0, text_y),
            Vec2::new(badge_w, 16.0),
        );

        let badge_bg = if is_selected {
            Color32::from_rgb(45, 80, 140)
        } else {
            Color32::from_rgb(28, 28, 36)
        };
        painter.rect_filled(badge_rect, Rounding::same(8.0), badge_bg);
        painter.text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            count_str,
            egui::FontId::monospace(10.0),
            if is_selected { Color32::WHITE } else { TEXT_SECONDARY },
        );

        response.clicked()
    }
}
