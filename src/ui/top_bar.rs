use crate::db::{FilterCriteria, PickFlag, SortOrder};
use crate::ui::theme::*;
use egui::{Color32, RichText, Rounding, Stroke, Ui};

pub enum TopBarAction {
    ImportFiles,
    ImportFolder,
    OpenCatalogModal,
    SwitchToGrid,
    SwitchToPlayer,
}

pub struct TopBar;

impl TopBar {
    pub fn show(
        ui: &mut Ui,
        filter: &mut FilterCriteria,
        is_player_mode: bool,
        can_open_player: bool,
        _cameras: &[String],
        _lenses: &[String],
    ) -> (bool, Option<TopBarAction>) {
        let mut filter_changed = false;
        let mut action = None;

        ui.horizontal(|ui| {
            // Studio Brand Logo & Title
            ui.add_space(4.0);
            ui.label(RichText::new("🎬 MLV EXPLORER").strong().size(13.5).color(Color32::from_rgb(147, 197, 253)));
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            // Workspace Mode Switcher (Photo browser Library / Loupe tabs)
            let grid_active = !is_player_mode;
            let player_active = is_player_mode;

            let grid_bg = if grid_active { Color32::from_rgb(45, 60, 85) } else { Color32::TRANSPARENT };
            let grid_stroke = if grid_active { Stroke::new(1.0_f32, BORDER_SELECTED) } else { Stroke::NONE };
            let grid_btn = egui::Button::new(RichText::new(" ▦ GRID [G] ").strong().size(12.0).color(if grid_active { Color32::WHITE } else { TEXT_SECONDARY }))
                .fill(grid_bg)
                .stroke(grid_stroke)
                .rounding(Rounding::same(4.0));

            if ui.add(grid_btn).clicked() {
                action = Some(TopBarAction::SwitchToGrid);
            }

            ui.add_enabled_ui(can_open_player, |ui| {
                let player_bg = if player_active { Color32::from_rgb(45, 60, 85) } else { Color32::TRANSPARENT };
                let player_stroke = if player_active { Stroke::new(1.0_f32, BORDER_SELECTED) } else { Stroke::NONE };
                let player_btn = egui::Button::new(RichText::new(" ▶ PLAYER [E] ").strong().size(12.0).color(if player_active { Color32::WHITE } else { TEXT_SECONDARY }))
                    .fill(player_bg)
                    .stroke(player_stroke)
                    .rounding(Rounding::same(4.0));

                if ui.add(player_btn).clicked() {
                    action = Some(TopBarAction::SwitchToPlayer);
                }
            });

            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            // Search Input Field
            ui.label(RichText::new("🔍").size(12.0).color(TEXT_MUTED));
            let search_edit = ui.add(
                egui::TextEdit::singleline(&mut filter.search_text)
                    .hint_text("Search name, camera, lens, tag...")
                    .desired_width(180.0),
            );
            if search_edit.changed() {
                filter_changed = true;
            }

            if !filter.search_text.is_empty() && ui.small_button("×").clicked() {
                filter.search_text.clear();
                filter_changed = true;
            }

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Sort Selector
            ui.label(RichText::new("Sort:").small().color(TEXT_SECONDARY));
            egui::ComboBox::from_id_salt("sort_order_combo")
                .selected_text(RichText::new(filter.sort.label()).small())
                .show_ui(ui, |ui| {
                    for sort_opt in SortOrder::ALL {
                        if ui.selectable_value(&mut filter.sort, sort_opt, sort_opt.label()).clicked() {
                            filter_changed = true;
                        }
                    }
                });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Rating Filter Quick Toggles (All, 1★+, 2★+, 3★+, 4★+, 5★)
            ui.label(RichText::new("Rating:").small().color(TEXT_SECONDARY));
            let r_all = filter.min_rating.is_none();
            if ui.selectable_label(r_all, RichText::new("All").small()).clicked() {
                filter.min_rating = None;
                filter_changed = true;
            }
            for r in 1..=5 {
                let is_sel = filter.min_rating == Some(r);
                let label = format!("{}★+", r);
                if ui.selectable_label(is_sel, RichText::new(label).small().color(if is_sel { ACCENT_GOLD } else { TEXT_PRIMARY })).clicked() {
                    filter.min_rating = if is_sel { None } else { Some(r) };
                    filter_changed = true;
                }
            }

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Pick / Reject Flags
            ui.label(RichText::new("Flag:").small().color(TEXT_SECONDARY));
            let flag_all = filter.flag.is_none();
            if ui.selectable_label(flag_all, RichText::new("All").small()).clicked() {
                filter.flag = None;
                filter_changed = true;
            }
            let is_pick = filter.flag == Some(PickFlag::Picked);
            if ui.selectable_label(is_pick, RichText::new("🏳 Pick").small().color(FLAG_PICKED)).clicked() {
                filter.flag = if is_pick { None } else { Some(PickFlag::Picked) };
                filter_changed = true;
            }
            let is_rej = filter.flag == Some(PickFlag::Rejected);
            if ui.selectable_label(is_rej, RichText::new("❌ Reject").small().color(FLAG_REJECTED)).clicked() {
                filter.flag = if is_rej { None } else { Some(PickFlag::Rejected) };
                filter_changed = true;
            }

            // Right-aligned actions: Import & Catalog
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);
                if ui.button("🗄 Catalog Backup").clicked() {
                    action = Some(TopBarAction::OpenCatalogModal);
                }

                if ui.button("📁 Import Folder").clicked() {
                    action = Some(TopBarAction::ImportFolder);
                }

                if ui.button("📥 Import Files").clicked() {
                    action = Some(TopBarAction::ImportFiles);
                }

                if filter.is_filtered() {
                    if ui.button(RichText::new("Reset Filter").small().color(TEXT_MUTED)).clicked() {
                        *filter = FilterCriteria::default();
                        filter_changed = true;
                    }
                }
            });
        });

        (filter_changed, action)
    }
}
