use crate::db::{PickFlag, VideoRecord};
use crate::thumbnails::ThumbnailManager;
use crate::ui::theme::*;
use egui::{Color32, Pos2, Rect, RichText, Rounding, Stroke, Ui, Vec2};

pub enum GridAction {
    Select(usize),
    OpenPlayer(usize),
    TogglePick(i64),
    SetRating(i64, u8),
}

pub struct GridView {
    pub card_size: f32,
}

impl Default for GridView {
    fn default() -> Self {
        Self { card_size: 260.0 }
    }
}

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

                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(card_w, card_h),
                            egui::Sense::click(),
                        );

                        if response.clicked() {
                            action = Some(GridAction::Select(i));
                        }
                        if response.double_clicked() {
                            action = Some(GridAction::OpenPlayer(i));
                        }

                        let painter = ui.painter();

                        // Card background & borders
                        let bg_color = if is_selected {
                            BG_CARD_SELECTED
                        } else if response.hovered() {
                            BG_CARD_HOVER
                        } else {
                            BG_CARD
                        };

                        let border_stroke = if is_selected {
                            Stroke::new(2.0_f32, BORDER_SELECTED)
                        } else {
                            Stroke::new(1.0_f32, BORDER_SUBTLE)
                        };

                        painter.rect(rect, Rounding::same(6.0), bg_color, border_stroke);

                        // 1. Thumbnail Area
                        let thumb_rect = Rect::from_min_size(
                            rect.min + Vec2::new(4.0, 4.0),
                            Vec2::new(card_w - 8.0, thumb_h),
                        );

                        // Fetch or request thumbnail
                        let thumb_tex = thumb_mgr.get_or_request(vid.id, &vid.file_path);

                        if let Some(tex) = thumb_tex {
                            // Paint real MLV frame 0 decoded thumbnail
                            painter.image(
                                tex.id(),
                                thumb_rect,
                                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        } else {
                            // Dark slate placeholder while decoding
                            painter.rect_filled(thumb_rect, Rounding::same(4.0), Color32::from_rgb(18, 18, 22));
                            painter.text(
                                thumb_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "⏳ Loading RAW...",
                                egui::FontId::proportional(12.0),
                                TEXT_MUTED,
                            );
                        }

                        // 2. Badges over thumbnail
                        // Duration pill (bottom-right)
                        let dur_str = vid.duration_tc();
                        let badge_w = 54.0;
                        let badge_rect = Rect::from_min_size(
                            Pos2::new(thumb_rect.max.x - badge_w - 4.0, thumb_rect.max.y - 18.0),
                            Vec2::new(badge_w, 15.0),
                        );
                        painter.rect_filled(badge_rect, Rounding::same(3.0), Color32::from_black_alpha(200));
                        painter.text(
                            badge_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            dur_str,
                            egui::FontId::monospace(9.5),
                            TEXT_PRIMARY,
                        );

                        // Codec / Bit-depth pill (bottom-left)
                        let bpp_str = format!("{}-bit", vid.bits_per_pixel);
                        let bpp_rect = Rect::from_min_size(
                            Pos2::new(thumb_rect.min.x + 4.0, thumb_rect.max.y - 18.0),
                            Vec2::new(38.0, 15.0),
                        );
                        painter.rect_filled(bpp_rect, Rounding::same(3.0), Color32::from_black_alpha(200));
                        painter.text(
                            bpp_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            bpp_str,
                            egui::FontId::proportional(9.5),
                            Color32::from_rgb(147, 197, 253),
                        );

                        // Pick / Flag Icon (top-left)
                        if vid.flag != PickFlag::Unflagged {
                            let (flag_icon, flag_color) = match vid.flag {
                                PickFlag::Picked => ("🏳", FLAG_PICKED),
                                PickFlag::Rejected => ("❌", FLAG_REJECTED),
                                PickFlag::Unflagged => ("", Color32::TRANSPARENT),
                            };
                            let flag_rect = Rect::from_min_size(
                                thumb_rect.min + Vec2::new(4.0, 4.0),
                                Vec2::new(18.0, 18.0),
                            );
                            painter.rect_filled(flag_rect, Rounding::same(3.0), Color32::from_black_alpha(180));
                            painter.text(
                                flag_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                flag_icon,
                                egui::FontId::proportional(11.0),
                                flag_color,
                            );
                        }

                        // 3. Card Footer (File metadata & rating)
                        let mut cursor_y = thumb_rect.max.y + 6.0;

                        // Filename
                        let fname_clipped = if vid.file_name.len() > 26 {
                            format!("{}...", &vid.file_name[..23])
                        } else {
                            vid.file_name.clone()
                        };
                        painter.text(
                            Pos2::new(rect.min.x + 8.0, cursor_y),
                            egui::Align2::LEFT_TOP,
                            fname_clipped,
                            egui::FontId::proportional(12.5),
                            TEXT_PRIMARY,
                        );
                        cursor_y += 18.0;

                        // Camera & Specs Line
                        let specs_str = format!("{} · {}p · {:.0}fps", vid.camera_name, vid.height, vid.fps);
                        let specs_clipped = if specs_str.len() > 30 {
                            format!("{}...", &specs_str[..27])
                        } else {
                            specs_str
                        };
                        painter.text(
                            Pos2::new(rect.min.x + 8.0, cursor_y),
                            egui::Align2::LEFT_TOP,
                            specs_clipped,
                            egui::FontId::proportional(10.5),
                            TEXT_MUTED,
                        );
                        cursor_y += 16.0;

                        // Star ratings (★) clickable or interactive
                        let mut star_x = rect.min.x + 8.0;
                        for s in 1..=5 {
                            let is_lit = s <= vid.rating;
                            let star_char = if is_lit { "★" } else { "☆" };
                            let star_col = if is_lit { ACCENT_GOLD } else { TEXT_MUTED };

                            let star_rect = Rect::from_min_size(Pos2::new(star_x, cursor_y), Vec2::new(14.0, 14.0));
                            if ui.rect_contains_pointer(star_rect) && ui.input(|inp| inp.pointer.primary_clicked()) {
                                let new_r = if vid.rating == s { 0 } else { s };
                                action = Some(GridAction::SetRating(vid.id, new_r));
                            }

                            painter.text(
                                star_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                star_char,
                                egui::FontId::proportional(13.0),
                                star_col,
                            );
                            star_x += 16.0;
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
