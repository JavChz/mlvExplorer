use crate::db::Database;
use egui::{Color32, Context, Ui, Window};

pub struct CatalogModal {
    pub is_open: bool,
    pub status_message: Option<(String, bool)>, // (message, is_error)
}

impl Default for CatalogModal {
    fn default() -> Self {
        Self {
            is_open: false,
            status_message: None,
        }
    }
}

impl CatalogModal {
    pub fn show(&mut self, ctx: &Context, db: &Database) -> bool {
        let mut catalog_updated = false;

        if !self.is_open {
            return false;
        }

        Window::new("🗄 Catalog Backup & Import")
            .open(&mut self.is_open)
            .resizable(false)
            .collapsible(false)
            .fixed_size([460.0, 320.0])
            .show(ctx, |ui| {
                ui.heading("Catalog Management");
                ui.label("Safeguard your tags, ratings, and video library indices.");
                ui.separator();

                // 1. SQLite Live Database Backup
                ui.group(|ui| {
                    ui.label(egui::RichText::new("1. SQLite Binary Backup (.db)").strong());
                    ui.label("Creates an exact, atomic snapshot of the SQLite database file while running.");
                    if ui.button("💾 Backup SQLite Database...").clicked() {
                        if let Some(dest) = rfd::FileDialog::new()
                            .set_file_name("mlv_catalog_backup.db")
                            .add_filter("SQLite Database", &["db", "sqlite"])
                            .save_file()
                        {
                            match db.backup_database_to_file(&dest) {
                                Ok(_) => {
                                    self.status_message = Some((
                                        format!("Successfully saved SQLite backup to {}", dest.display()),
                                        false,
                                    ));
                                }
                                Err(e) => {
                                    self.status_message = Some((
                                        format!("Database backup failed: {e}"),
                                        true,
                                    ));
                                }
                            }
                        }
                    }
                });

                ui.add_space(8.0);

                // 2. JSON Portable Export & Import
                ui.group(|ui| {
                    ui.label(egui::RichText::new("2. Portable Catalog JSON (.json)").strong());
                    ui.label("Human-readable format for importing into another computer or library.");
                    
                    ui.horizontal(|ui| {
                        if ui.button("📤 Export Catalog JSON...").clicked() {
                            if let Some(dest) = rfd::FileDialog::new()
                                .set_file_name("mlv_catalog_export.json")
                                .add_filter("JSON File", &["json"])
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
                                            format!("Export failed: {e}"),
                                            true,
                                        ));
                                    }
                                }
                            }
                        }

                        if ui.button("📥 Import Catalog JSON...").clicked() {
                            if let Some(src) = rfd::FileDialog::new()
                                .add_filter("JSON File", &["json"])
                                .pick_file()
                            {
                                match db.import_catalog_json(&src) {
                                    Ok(count) => {
                                        self.status_message = Some((
                                            format!("Successfully imported {count} videos from JSON"),
                                            false,
                                        ));
                                        catalog_updated = true;
                                    }
                                    Err(e) => {
                                        self.status_message = Some((
                                            format!("Import failed: {e}"),
                                            true,
                                        ));
                                    }
                                }
                            }
                        }
                    });
                });

                ui.add_space(10.0);

                // Status message indicator
                if let Some((ref msg, is_err)) = self.status_message {
                    let color = if is_err { Color32::RED } else { Color32::from_rgb(100, 220, 120) };
                    ui.label(egui::RichText::new(msg).color(color));
                }
            });

        catalog_updated
    }
}
