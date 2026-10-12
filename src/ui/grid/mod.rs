pub mod card;
pub mod types;

pub use card::Card;
pub use types::{GridAction, GridView};

use crate::db::{SortOrder, VideoRecord};
use crate::thumbnails::ThumbnailManager;
use crate::ui::theme::*;
use egui::{Color32, Pos2, RichText, Rounding, Stroke, Ui, Vec2};

struct DayGroup {
    day_key: String,
    title: String,
    indices: Vec<usize>,
    total_bytes: u64,
    total_duration_secs: f32,
}

fn format_day_info(date_str: Option<&str>) -> (String, String) {
    let Some(d) = date_str else {
        return ("0000-00-00".to_string(), "Undated / Unknown Date".to_string());
    };
    let clean = d.trim();
    if clean.len() < 10 {
        return ("0000-00-00".to_string(), "Undated / Unknown Date".to_string());
    }
    let date_part = &clean[..10];
    if let Ok(naive) = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
        let formatted = naive.format("%A, %B %e, %Y").to_string();
        (date_part.to_string(), formatted)
    } else {
        (date_part.to_string(), date_part.to_string())
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!("{:.0} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn format_duration(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    let m = s / 60;
    let rem = s % 60;
    format!("{:02}:{:02}", m, rem)
}

impl GridView {
    pub fn show(
        &mut self,
        ui: &mut Ui,
        videos: &[VideoRecord],
        selected_idx: Option<usize>,
        thumb_mgr: &mut ThumbnailManager,
        sort_order: &mut SortOrder,
    ) -> Option<GridAction> {
        let mut action = None;

        // Top toolbar for Sort, Grouping, Zoom & Library Stats
        ui.horizontal(|ui| {
            // Sort Selector
            ui.label(RichText::new("Sort by:").small().color(TEXT_SECONDARY));
            egui::ComboBox::from_id_salt("grid_sort_selector")
                .selected_text(RichText::new(sort_order.label()).small())
                .show_ui(ui, |ui| {
                    for opt in SortOrder::ALL {
                        if ui.selectable_value(sort_order, opt, opt.label()).clicked() {
                            action = Some(GridAction::SortChanged);
                        }
                    }
                });

            ui.separator();

            // Group by Day Toggle
            let group_btn = ui
                .selectable_label(self.group_by_day, RichText::new("📅 Group by Day").small())
                .on_hover_text("Group clips by calendar capture date with clear day section headers");
            if group_btn.clicked() {
                self.group_by_day = !self.group_by_day;
            }

            ui.separator();

            // Thumbnail Zoom Slider
            ui.label(RichText::new("Zoom:").small().color(TEXT_SECONDARY));
            ui.add(
                egui::Slider::new(&mut self.card_size, 180.0..=440.0)
                    .show_value(false)
            );

            // Right-aligned Total Stats
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let total_size: u64 = videos.iter().map(|v| v.file_size).sum();
                let total_dur: f32 = videos.iter().map(|v| v.duration_seconds).sum();
                ui.label(
                    RichText::new(format!(
                        "{} clips · {} · {}",
                        videos.len(),
                        format_bytes(total_size),
                        format_duration(total_dur)
                    ))
                    .small()
                    .color(TEXT_MUTED),
                );
            });
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
            return action;
        }

        let card_w = self.card_size;
        let thumb_h = card_w * (9.0 / 16.0); // 16:9 thumbnail
        let footer_h = 68.0;
        let card_h = thumb_h + footer_h;

        egui::ScrollArea::vertical().show(ui, |ui| {
            let available_width = ui.available_width();
            let columns = ((available_width / (card_w + 12.0)).floor() as usize).max(1);

            if self.group_by_day {
                // Group clips by calendar capture day
                let mut groups: Vec<DayGroup> = Vec::new();
                for (i, vid) in videos.iter().enumerate() {
                    let (key, title) = format_day_info(vid.date_recorded.as_deref());
                    if let Some(last) = groups.last_mut() {
                        if last.day_key == key {
                            last.indices.push(i);
                            last.total_bytes += vid.file_size;
                            last.total_duration_secs += vid.duration_seconds;
                            continue;
                        }
                    }
                    groups.push(DayGroup {
                        day_key: key,
                        title,
                        indices: vec![i],
                        total_bytes: vid.file_size,
                        total_duration_secs: vid.duration_seconds,
                    });
                }

                for (grp_idx, grp) in groups.iter().enumerate() {
                    // Day Section Header Banner
                    ui.add_space(6.0);
                    let (banner_rect, _) = ui.allocate_exact_size(Vec2::new(available_width, 28.0), egui::Sense::hover());
                    let painter = ui.painter();
                    painter.rect(
                        banner_rect,
                        Rounding::same(4.0),
                        Color32::from_rgb(22, 23, 29),
                        Stroke::new(1.0_f32, Color32::from_rgb(38, 42, 54)),
                    );

                    // Left title
                    painter.text(
                        Pos2::new(banner_rect.min.x + 10.0, banner_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        format!("📅 {}", grp.title),
                        egui::FontId::proportional(12.5),
                        Color32::from_rgb(243, 244, 246),
                    );

                    // Right badges
                    let grp_info = format!(
                        "{} clips · {} · {}",
                        grp.indices.len(),
                        format_bytes(grp.total_bytes),
                        format_duration(grp.total_duration_secs)
                    );
                    painter.text(
                        Pos2::new(banner_rect.max.x - 10.0, banner_rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        grp_info,
                        egui::FontId::proportional(11.0),
                        Color32::from_rgb(156, 163, 175),
                    );

                    ui.add_space(8.0);

                    // Grid for this day
                    egui::Grid::new(format!("day_grid_{}", grp_idx))
                        .spacing([12.0, 14.0])
                        .show(ui, |ui| {
                            for (c_idx, &i) in grp.indices.iter().enumerate() {
                                let vid = &videos[i];
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

                                if (c_idx + 1) % columns == 0 {
                                    ui.end_row();
                                }
                            }
                        });

                    ui.add_space(10.0);
                }
            } else {
                // Continuous flat grid
                egui::Grid::new("video_cards_grid_flat")
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
            }
        });

        action
    }
}
