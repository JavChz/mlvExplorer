use crate::db::{FilterCriteria, PickFlag, SortOrder};
use crate::ui::theme::*;
use egui::{RichText, Ui};

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
            // Brand Logo & Name
            ui.label(RichText::new("🎬 MLV EXPLORER").strong().size(14.0).color(ACCENT_BLUE));
            ui.separator();

            // Mode Switcher Pills (Lightroom Library vs Develop/Loupe)
            let grid_active = !is_player_mode;
            let player_active = is_player_mode;

            if ui.selectable_label(grid_active, "▦ Grid [G]").clicked() {
                action = Some(TopBarAction::SwitchToGrid);
            }

            ui.add_enabled_ui(can_open_player, |ui| {
                if ui.selectable_label(player_active, "▶ Player [E]").clicked() {
                    action = Some(TopBarAction::SwitchToPlayer);
                }
            });

            ui.separator();

            // Search Bar
            ui.label(RichText::new("🔍").color(TEXT_MUTED));
            let search_edit = ui.add(
                egui::TextEdit::singleline(&mut filter.search_text)
                    .hint_text("Search name, camera, lens, tag...")
                    .desired_width(170.0),
            );
            if search_edit.changed() {
                filter_changed = true;
            }

            if !filter.search_text.is_empty() && ui.small_button("✖").clicked() {
                filter.search_text.clear();
                filter_changed = true;
            }

            ui.separator();

            // Sort Selector
            ui.label(RichText::new("Sort:").color(TEXT_SECONDARY));
            egui::ComboBox::from_id_salt("sort_order_combo")
                .selected_text(filter.sort.label())
                .show_ui(ui, |ui| {
                    for sort_opt in SortOrder::ALL {
                        if ui.selectable_value(&mut filter.sort, sort_opt, sort_opt.label()).clicked() {
                            filter_changed = true;
                        }
                    }
                });

            ui.separator();

            // Rating Filter Quick Toggles (All, 1+, 2+, 3+, 4+, 5+)
            ui.label(RichText::new("Rating:").color(TEXT_SECONDARY));
            let r_all = filter.min_rating.is_none();
            if ui.selectable_label(r_all, "All").clicked() {
                filter.min_rating = None;
                filter_changed = true;
            }
            for r in 1..=5 {
                let is_sel = filter.min_rating == Some(r);
                let label = format!("{}★+", r);
                if ui.selectable_label(is_sel, RichText::new(label).color(if is_sel { ACCENT_GOLD } else { TEXT_PRIMARY })).clicked() {
                    filter.min_rating = if is_sel { None } else { Some(r) };
                    filter_changed = true;
                }
            }

            ui.separator();

            // Pick / Reject Flags
            ui.label(RichText::new("Flag:").color(TEXT_SECONDARY));
            let flag_all = filter.flag.is_none();
            if ui.selectable_label(flag_all, "All").clicked() {
                filter.flag = None;
                filter_changed = true;
            }
            let is_pick = filter.flag == Some(PickFlag::Picked);
            if ui.selectable_label(is_pick, RichText::new("🏳 Pick").color(FLAG_PICKED)).clicked() {
                filter.flag = if is_pick { None } else { Some(PickFlag::Picked) };
                filter_changed = true;
            }
            let is_rej = filter.flag == Some(PickFlag::Rejected);
            if ui.selectable_label(is_rej, RichText::new("❌ Reject").color(FLAG_REJECTED)).clicked() {
                filter.flag = if is_rej { None } else { Some(PickFlag::Rejected) };
                filter_changed = true;
            }

            // Right-aligned actions: Import & Catalog
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                    if ui.button(RichText::new("Reset Filter").color(TEXT_MUTED)).clicked() {
                        *filter = FilterCriteria::default();
                        filter_changed = true;
                    }
                }
            });
        });

        (filter_changed, action)
    }
}
