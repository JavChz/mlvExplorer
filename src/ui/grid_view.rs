use crate::db::{PickFlag, VideoRecord};
use egui::{Color32, Pos2, Rect, Rounding, Stroke, Ui, Vec2};

pub struct GridView {
    pub card_size: f32, // Thumbnail card width (e.g. 200..400)
}

impl Default for GridView {
    fn default() -> Self {
        Self { card_size: 240.0 }
    }
}

pub enum GridAction {
    Select(usize),
    OpenPlayer(usize),
    TogglePick(i64),
    SetRating(i64, u8),
}

impl GridView {
    pub fn show(
        &mut self,
        ui: &mut Ui,
        videos: &[VideoRecord],
        selected_idx: Option<usize>,
    ) -> Option<GridAction> {
        let mut action = None;

        // Top bar for Grid Zoom
        ui.horizontal(|ui| {
            ui.label("Thumbnail Size:");
            ui.add(egui::Slider::new(&mut self.card_size, 160.0..=420.0).text("px"));
            ui.separator();
            ui.label(format!("Showing {} videos", videos.len()));
        });

        ui.separator();

        if videos.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.heading("No Magic Lantern Videos in Catalog");
                ui.label("Drag and drop .MLV files or folders here to quickly import them.");
                ui.add_space(20.0);
            });
            return None;
        }

        let card_w = self.card_size;
        let card_h = card_w * 0.75 + 70.0; // Aspect ratio for 16:9 thumb + metadata footer

        egui::ScrollArea::vertical().show(ui, |ui| {
            let available_width = ui.available_width();
            let columns = ((available_width / (card_w + 12.0)).floor() as usize).max(1);

            egui::Grid::new("video_cards_grid")
                .spacing([12.0, 12.0])
                .show(ui, |ui| {
                    for (i, vid) in videos.iter().enumerate() {
                        let is_selected = selected_idx == Some(i);

                        let card_rect = ui.allocate_exact_size(Vec2::new(card_w, card_h), egui::Sense::click());
                        let (rect, response) = (card_rect.0, card_rect.1);

                        // Click handling
                        if response.clicked() {
                            action = Some(GridAction::Select(i));
                        }
                        if response.double_clicked() {
                            action = Some(GridAction::OpenPlayer(i));
                        }

                        // Card background & borders
                        let bg_color = if is_selected {
                            Color32::from_rgb(45, 55, 75)
                        } else if response.hovered() {
                            Color32::from_rgb(38, 38, 42)
                        } else {
                            Color32::from_rgb(28, 28, 32)
                        };

                        let border_stroke = if is_selected {
                            Stroke::new(2.0, Color32::from_rgb(90, 150, 255))
                        } else {
                            Stroke::new(1.0, Color32::from_rgb(50, 50, 55))
                        };

                        let painter = ui.painter();
                        painter.rect(rect, Rounding::same(6.0), bg_color, border_stroke);

                        // Thumbnail Box (top 16:9 area)
                        let thumb_h = card_w * 0.5625; // 16:9
                        let thumb_rect = Rect::from_min_size(rect.min + Vec2::new(4.0, 4.0), Vec2::new(card_w - 8.0, thumb_h));
                        
                        // Placeholder dark slate background for thumbnail
                        painter.rect_filled(thumb_rect, Rounding::same(4.0), Color32::from_rgb(18, 18, 22));

                        // Thumbnail icon / text placeholder
                        let center = thumb_rect.center();
                        painter.text(
                            center,
                            egui::Align2::CENTER_CENTER,
                            "🎬 MLV RAW",
                            egui::FontId::proportional(14.0),
                            Color32::from_rgb(130, 140, 160),
                        );

                        // Duration Badge (bottom-right of thumbnail)
                        let dur_text = format!("{:.1}s", vid.duration_seconds);
                        let dur_badge_rect = Rect::from_min_size(
                            Pos2::new(thumb_rect.max.x - 52.0, thumb_rect.max.y - 20.0),
                            Vec2::new(48.0, 16.0),
                        );
                        painter.rect_filled(dur_badge_rect, Rounding::same(3.0), Color32::from_black_alpha(180));
                        painter.text(
                            dur_badge_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            dur_text,
                            egui::FontId::monospace(10.0),
                            Color32::WHITE,
                        );

                        // Pick / Flag Badge (top-left of thumbnail)
                        if vid.flag != PickFlag::Unflagged {
                            let flag_icon = if vid.flag == PickFlag::Picked { "🏳" } else { "❌" };
                            let flag_color = if vid.flag == PickFlag::Picked { Color32::WHITE } else { Color32::RED };
                            painter.text(
                                thumb_rect.min + Vec2::new(12.0, 12.0),
                                egui::Align2::CENTER_CENTER,
                                flag_icon,
                                egui::FontId::proportional(12.0),
                                flag_color,
                            );
                        }

                        // Metadata Footer
                        let mut cursor_y = thumb_rect.max.y + 6.0;

                        // File name
                        let fname_clipped = if vid.file_name.len() > 24 {
                            format!("{}...", &vid.file_name[..21])
                        } else {
                            vid.file_name.clone()
                        };
                        painter.text(
                            Pos2::new(rect.min.x + 8.0, cursor_y),
                            egui::Align2::LEFT_TOP,
                            fname_clipped,
                            egui::FontId::proportional(12.0),
                            Color32::WHITE,
                        );
                        cursor_y += 18.0;

                        // Specs line: Camera, Resolution, FPS
                        let specs_text = format!("{} • {}p • {:.0}fps", vid.camera_name, vid.height, vid.fps);
                        painter.text(
                            Pos2::new(rect.min.x + 8.0, cursor_y),
                            egui::Align2::LEFT_TOP,
                            specs_text,
                            egui::FontId::proportional(10.0),
                            Color32::from_rgb(160, 160, 170),
                        );
                        cursor_y += 16.0;

                        // Star ratings (★)
                        let mut stars_str = String::new();
                        for s in 1..=5 {
                            if s <= vid.rating {
                                stars_str.push('★');
                            } else {
                                stars_str.push('☆');
                            }
                        }
                        painter.text(
                            Pos2::new(rect.min.x + 8.0, cursor_y),
                            egui::Align2::LEFT_TOP,
                            stars_str,
                            egui::FontId::proportional(12.0),
                            Color32::from_rgb(255, 200, 50),
                        );

                        if (i + 1) % columns == 0 {
                            ui.end_row();
                        }
                    }
                });
        });

        action
    }
}
