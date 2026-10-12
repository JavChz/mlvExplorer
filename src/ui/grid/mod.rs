pub mod card;
pub mod types;

pub use card::Card;
pub use types::{GridAction, GridView};

use crate::db::VideoRecord;
use crate::thumbnails::ThumbnailManager;
use crate::ui::theme::*;
use egui::{RichText, Ui};

impl GridView {
    pub fn show(
        &mut self,
        ui: &mut Ui,
        videos: &[VideoRecord],
        selected_idx: Option<usize>,
        thumb_mgr: &mut ThumbnailManager,
    ) -> Option<GridAction> {
        let mut action = None;

        // Top bar for Grid Zoom & Counter
        ui.horizontal(|ui| {
            ui.label(RichText::new("Thumbnail Zoom:").color(TEXT_SECONDARY));
            ui.add(egui::Slider::new(&mut self.card_size, 180.0..=440.0).show_value(false));

            ui.separator();
            ui.label(RichText::new(format!("Showing {} clips", videos.len())).color(TEXT_MUTED));
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        if videos.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.label(RichText::new("🎬").size(48.0));
                ui.add_space(8.0);
                ui.heading("No Magic Lantern Clips Matching Filter");
                ui.add_space(4.0);
                ui.label(RichText::new("Drag and drop .MLV files or folders anywhere onto the window.").color(TEXT_MUTED));
            });
            return None;
        }

        let card_w = self.card_size;
        let thumb_h = card_w * (9.0 / 16.0); // 16:9 thumbnail
        let footer_h = 68.0;
        let card_h = thumb_h + footer_h;

        egui::ScrollArea::vertical().show(ui, |ui| {
            let available_width = ui.available_width();
            let columns = ((available_width / (card_w + 12.0)).floor() as usize).max(1);

            egui::Grid::new("video_cards_grid")
                .spacing([12.0, 14.0])
                .show(ui, |ui| {
                    for (i, vid) in videos.iter().enumerate() {
                        let is_selected = selected_idx == Some(i);

                        if let Some(card_act) = Card::show(
                            ui,
                            i,
                            vid,
                            is_selected,
                            card_w,
                            card_h,
                            thumb_h,
                            thumb_mgr,
                        ) {
                            action = Some(card_act);
                        }

                        if (i + 1) % columns == 0 {
                            ui.end_row();
                        }
                    }
                });
        });

        action
    }
}
