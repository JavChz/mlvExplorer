use crate::db::Database;
use crate::ui::theme::*;
use egui::{Context, RichText, Window};

pub struct CatalogDialog {
    pub is_open: bool,
    pub status_message: Option<(String, bool)>, // (message, is_error)
}

impl Default for CatalogDialog {
    fn default() -> Self {
        Self {
            is_open: false,
            status_message: None,
        }
    }
}

impl CatalogDialog {
    pub fn show(&mut self, ctx: &Context, db: &Database) -> bool {
        let mut catalog_updated = false;

        if !self.is_open {
            return false;
        }

        Window::new(RichText::new("🗄 Catalog Backup & Migration").strong())
            .open(&mut self.is_open)
            .resizable(false)
            .collapsible(false)
            .fixed_size([500.0, 340.0])
            .show(ctx, |ui| {
                ui.label(RichText::new("Protect your ratings, custom tags, pick flags, and notes.").color(TEXT_SECONDARY));
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(8.0);

                // 1. Live SQLite Database Backup
                ui.group(|ui| {
                    ui.label(RichText::new("1. Native SQLite Binary Backup (.db)").strong());
                    ui.label(RichText::new("Takes a hot, atomic snapshot of your entire library catalog without locking or stopping playback.").small().color(TEXT_MUTED));
                    ui.add_space(4.0);

                    if ui.button("💾 Backup SQLite Database to Disk...").clicked() {
                        if let Some(dest) = rfd::FileDialog::new()
                            .set_file_name("mlv_catalog_backup.db")
                            .add_filter("SQLite Database", &["db", "sqlite"])
                            .save_file()
                        {
                            match db.backup_database_to_file(&dest) {
                                Ok(_) => {
                                    self.status_message = Some((
                                        format!("Snapshot saved successfully to {}", dest.display()),
                                        false,
                                    ));
                                }
                                Err(e) => {
                                    self.status_message = Some((
                                        format!("Database backup error: {e}"),
                                        true,
                                    ));
                                }
                            }
                        }
                    }
                });

                ui.add_space(8.0);

                // 2. Portable JSON Export & Import
                ui.group(|ui| {
                    ui.label(RichText::new("2. Portable Catalog JSON (.json)").strong());
                    ui.label(RichText::new("Export ratings and tags to portable JSON. Importing will merge tags/ratings by matching video paths.").small().color(TEXT_MUTED));
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        if ui.button("📤 Export Catalog JSON...").clicked() {
                            if let Some(dest) = rfd::FileDialog::new()
                                .set_file_name("mlv_catalog_export.json")
                                .add_filter("JSON Document", &["json"])
                                .save_file()
                            {
                                match db.export_catalog_json(&dest) {
                                    Ok(_) => {
                                        self.status_message = Some((
                                            format!("Exported catalog JSON to {}", dest.display()),
                                            false,
                                        ));
                                    }
                                    Err(e) => {
                                        self.status_message = Some((
                                            format!("Export error: {e}"),
                                            true,
                                        ));
                                    }
                                }
                            }
                        }

                        if ui.button("📥 Import & Merge JSON...").clicked() {
                            if let Some(src) = rfd::FileDialog::new()
                                .add_filter("JSON Document", &["json"])
                                .pick_file()
                            {
                                match db.import_catalog_json(&src) {
                                    Ok(count) => {
                                        self.status_message = Some((
                                            format!("Successfully merged {count} clips from JSON"),
                                            false,
                                        ));
                                        catalog_updated = true;
                                    }
                                    Err(e) => {
                                        self.status_message = Some((
                                            format!("Import error: {e}"),
                                            true,
                                        ));
                                    }
                                }
                            }
                        }
                    });
                });

                ui.add_space(10.0);

                // Status Message
                if let Some((ref msg, is_err)) = self.status_message {
                    let col = if is_err { FLAG_REJECTED } else { FLAG_PICKED };
                    ui.label(RichText::new(msg).color(col));
                }
            });

        catalog_updated
    }
}
