use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use crate::updater::{UpdateManager, UpdateStatus, GITHUB_REPO};
use egui::{Context, RichText, Window};

pub struct HelpDialogs {
    pub show_shortcuts: bool,
    pub show_about: bool,
    pub show_updater: bool,
    shortcuts_filter: String,
}

impl Default for HelpDialogs {
    fn default() -> Self {
        Self {
            show_shortcuts: false,
            show_about: false,
            show_updater: false,
            shortcuts_filter: String::new(),
        }
    }
}

enum DialogAction {
    Check,
    Cancel,
    Download {
        version: String,
        name: String,
        url: String,
    },
}

struct ShortcutItem {
    key: &'static str,
    context: &'static str,
    desc: &'static str,
}

const SHORTCUTS: &[ShortcutItem] = &[
    // Navigation & Views
    ShortcutItem { key: "G", context: "Global", desc: "Switch to Contact Sheet Grid View" },
    ShortcutItem { key: "E", context: "Global", desc: "Switch to Cinema Player View" },
    ShortcutItem { key: "F", context: "Player", desc: "Toggle Borderless Cinema Fullscreen" },
    ShortcutItem { key: "Esc", context: "Any", desc: "Exit Fullscreen / Return to Grid View" },
    
    // Player Controls
    ShortcutItem { key: "Space", context: "Player", desc: "Play / Pause playback" },
    ShortcutItem { key: "Left / Right", context: "Player", desc: "Step 1 frame backward / forward" },
    ShortcutItem { key: "M", context: "Player", desc: "Toggle Audio Mute" },

    // Selection & Rating
    ShortcutItem { key: "1 – 5", context: "Grid / Player", desc: "Set Star Rating (1 to 5 stars)" },
    ShortcutItem { key: "0", context: "Grid / Player", desc: "Clear Star Rating" },
    ShortcutItem { key: "P", context: "Grid / Player", desc: "Flag selected clip(s) as Picked" },
    ShortcutItem { key: "X", context: "Grid / Player", desc: "Flag selected clip(s) as Rejected" },
    ShortcutItem { key: "U", context: "Grid / Player", desc: "Unflag / Reset to Neutral" },

    // Multi-Selection & Clipboard
    ShortcutItem { key: "Ctrl + A", context: "Grid", desc: "Select All clips in current view" },
    ShortcutItem { key: "Ctrl + D", context: "Grid", desc: "Deselect All clips" },
    ShortcutItem { key: "Ctrl + C", context: "Grid / Player", desc: "Copy Clip Settings (Slate, Anamorphic, Tags, Ratings)" },
    ShortcutItem { key: "Ctrl + V", context: "Grid / Player", desc: "Paste Clip Settings to Selected clip(s)" },
    ShortcutItem { key: "Shift + Click", context: "Grid", desc: "Continuous range selection" },
    ShortcutItem { key: "Ctrl + Click", context: "Grid", desc: "Toggle clip selection" },

    // Management
    ShortcutItem { key: "Ctrl + O", context: "Global", desc: "Import .MLV Files" },
    ShortcutItem { key: "Ctrl + Shift + O", context: "Global", desc: "Import Directory of .MLV Files" },
    ShortcutItem { key: "Del / Backspace", context: "Grid", desc: "Remove selected clip(s) from Catalog" },
    ShortcutItem { key: "Shift + Del", context: "Grid", desc: "Move selected clip(s) and spanned slices to Trash" },
];

impl HelpDialogs {
    pub fn show(&mut self, ctx: &Context, updater: &mut UpdateManager) {
        // 1. Shortcuts Dialog
        if self.show_shortcuts {
            let mut is_open = self.show_shortcuts;
            let mut should_close = false;
            let title = widgets::icon_label(icon::KEYBOARD, "Keyboard Shortcuts");
            Window::new(RichText::new(title).strong().size(FONT_BODY))
                .open(&mut is_open)
                .resizable(true)
                .collapsible(false)
                .pivot(egui::Align2::CENTER_CENTER)
                .default_pos(ctx.screen_rect().center())
                .default_size([520.0, 420.0])
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);

                    // Filter search
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Search:").size(FONT_SMALL).color(text_muted(ui)));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.shortcuts_filter)
                                .hint_text("Filter shortcuts...")
                                .desired_width(180.0),
                        );
                        if !self.shortcuts_filter.is_empty() {
                            if widgets::icon_button(ui, icon::X, false, "Clear").clicked() {
                                self.shortcuts_filter.clear();
                            }
                        }
                    });

                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("shortcuts_table")
                            .num_columns(3)
                            .spacing([14.0, 6.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label(RichText::new("Shortcut").strong().size(FONT_SMALL).color(text_primary(ui)));
                                ui.label(RichText::new("Context").strong().size(FONT_SMALL).color(text_muted(ui)));
                                ui.label(RichText::new("Action").strong().size(FONT_SMALL).color(text_primary(ui)));
                                ui.end_row();

                                let term = self.shortcuts_filter.to_lowercase();
                                for s in SHORTCUTS {
                                    if !term.is_empty()
                                        && !s.key.to_lowercase().contains(&term)
                                        && !s.context.to_lowercase().contains(&term)
                                        && !s.desc.to_lowercase().contains(&term)
                                    {
                                        continue;
                                    }

                                    ui.label(
                                        RichText::new(s.key)
                                            .strong()
                                            .size(FONT_BODY)
                                            .color(ACCENT_HOVER),
                                    );
                                    ui.label(RichText::new(s.context).size(FONT_CAPTION).color(text_muted(ui)));
                                    ui.label(RichText::new(s.desc).size(FONT_SMALL).color(text_primary(ui)));
                                    ui.end_row();
                                }
                            });
                    });

                    ui.add_space(8.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::button(ui, "Close").clicked() {
                            should_close = true;
                        }
                    });
                });
            self.show_shortcuts = is_open && !should_close;
        }

        // 2. About Dialog
        if self.show_about {
            let mut is_open = self.show_about;
            let mut should_close = false;
            let title = widgets::icon_label(icon::INFO, "About MLV Explorer");
            Window::new(RichText::new(title).strong().size(FONT_BODY))
                .open(&mut is_open)
                .resizable(false)
                .collapsible(false)
                .pivot(egui::Align2::CENTER_CENTER)
                .default_pos(ctx.screen_rect().center())
                .fixed_size([460.0, 360.0])
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 8.0);

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icon::FILM_STRIP).size(32.0).color(ACCENT));
                        ui.vertical(|ui| {
                            ui.label(RichText::new("MLV Explorer").strong().size(18.0).color(text_primary(ui)));
                            ui.label(
                                RichText::new(format!("Version {} (Alpha)", env!("CARGO_PKG_VERSION")))
                                    .size(FONT_CAPTION)
                                    .color(text_muted(ui)),
                            );
                        });
                    });

                    ui.separator();

                    ui.label(
                        RichText::new(
                            "A photo catalog-style browser, player, and organizer for Magic Lantern RAW Video (.mlv) files, written in Rust with egui and SQLite.",
                        )
                        .size(FONT_BODY)
                        .color(text_primary(ui)),
                    );

                    ui.add_space(4.0);
                    ui.label(RichText::new("Upstream Projects & Credits:").strong().size(FONT_SMALL).color(text_primary(ui)));
                    ui.label(RichText::new("• The Magic Lantern Project (Format specification & reverse engineering)").size(FONT_CAPTION).color(text_secondary(ui)));
                    ui.label(RichText::new("• MLV App (Ilia3101 & contributors - dual-card spanning & matrix standards)").size(FONT_CAPTION).color(text_secondary(ui)));
                    ui.label(RichText::new("• Phosphor Icons (Tobias Fried & Helena Zhang)").size(FONT_CAPTION).color(text_secondary(ui)));
                    ui.label(RichText::new("• egui / eframe (Emil Ernerfeldt)").size(FONT_CAPTION).color(text_secondary(ui)));
                    ui.label(RichText::new("• rodio (RustAudio community)").size(FONT_CAPTION).color(text_secondary(ui)));

                    ui.separator();

                    ui.label(
                        RichText::new("License: GNU General Public License v3.0 or later (GPL-3.0-or-later)")
                            .size(FONT_CAPTION)
                            .color(text_muted(ui)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::primary_button(ui, "Close").clicked() {
                            should_close = true;
                        }
                    });
                });
            self.show_about = is_open && !should_close;
        }

        // 3. Software Update Dialog
        if self.show_updater {
            let mut is_open = self.show_updater;
            let mut should_close = false;
            let mut dialog_action = None;
            let title = widgets::icon_label(icon::ARROWS_CLOCKWISE, "Software Update");

            Window::new(RichText::new(title).strong().size(FONT_BODY))
                .open(&mut is_open)
                .resizable(true)
                .collapsible(false)
                .pivot(egui::Align2::CENTER_CENTER)
                .default_pos(ctx.screen_rect().center())
                .default_size([500.0, 360.0])
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 10.0);

                    match &updater.status {
                        UpdateStatus::Idle => {
                            ui.label("Click Check for Updates to query the latest release.");
                            if widgets::primary_button(ui, "Check Now").clicked() {
                                dialog_action = Some(DialogAction::Check);
                            }
                        }
                        UpdateStatus::Checking => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(
                                    RichText::new("Checking GitHub for the latest release...")
                                        .size(FONT_BODY)
                                        .color(text_primary(ui)),
                                );
                            });
                            ui.add_space(8.0);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if widgets::button(ui, "Cancel").clicked() {
                                    dialog_action = Some(DialogAction::Cancel);
                                    should_close = true;
                                }
                            });
                        }
                        UpdateStatus::UpToDate { current_version } => {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(icon::CHECK_CIRCLE).size(28.0).color(FLAG_PICKED));
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("MLV Explorer is up to date!")
                                            .strong()
                                            .size(FONT_TITLE)
                                            .color(text_primary(ui)),
                                    );
                                    ui.label(
                                        RichText::new(format!("Version v{} is currently the newest version.", current_version))
                                            .size(FONT_CAPTION)
                                            .color(text_muted(ui)),
                                    );
                                });
                            });

                            ui.separator();
                            ui.label(
                                RichText::new("You have the latest version with all recent features, optimizations, and bug fixes.")
                                    .size(FONT_SMALL)
                                    .color(text_secondary(ui)),
                            );

                            ui.add_space(8.0);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if widgets::primary_button(ui, "Close").clicked() {
                                    should_close = true;
                                }
                                if widgets::button(ui, "Check Again").clicked() {
                                    dialog_action = Some(DialogAction::Check);
                                }
                            });
                        }
                        UpdateStatus::UpdateAvailable {
                            current_version,
                            latest_version,
                            release_title,
                            release_notes,
                            release_url,
                            target_asset_name,
                            target_download_url,
                            target_asset_size,
                        } => {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(icon::DOWNLOAD_SIMPLE).size(28.0).color(ACCENT));
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(format!("A new version is available: v{}", latest_version))
                                            .strong()
                                            .size(FONT_TITLE)
                                            .color(text_primary(ui)),
                                    );
                                    ui.label(
                                        RichText::new(format!("Current version: v{} ➔ New version: v{}", current_version, latest_version))
                                            .size(FONT_CAPTION)
                                            .color(text_muted(ui)),
                                    );
                                });
                            });

                            if !release_title.is_empty() && release_title != latest_version {
                                ui.label(RichText::new(release_title).strong().size(FONT_BODY).color(text_primary(ui)));
                            }

                            if let Some(asset_name) = target_asset_name {
                                let size_str = target_asset_size.map(|sz| {
                                    let mb = sz as f64 / (1024.0 * 1024.0);
                                    format!(" ({:.1} MB)", mb)
                                }).unwrap_or_default();
                                ui.label(
                                    RichText::new(format!("Package: {}{}", asset_name, size_str))
                                        .size(FONT_CAPTION)
                                        .color(ACCENT_HOVER),
                                );
                            }

                            ui.label(RichText::new("Release Notes:").strong().size(FONT_SMALL).color(text_primary(ui)));
                            egui::ScrollArea::vertical()
                                .max_height(160.0)
                                .show(ui, |ui| {
                                    let notes = if release_notes.trim().is_empty() {
                                        "No detailed release notes provided."
                                    } else {
                                        release_notes.as_str()
                                    };
                                    ui.label(RichText::new(notes).size(FONT_SMALL).color(text_secondary(ui)));
                                });

                            ui.separator();
                            ui.horizontal(|ui| {
                                if let (Some(asset_name), Some(url)) = (target_asset_name, target_download_url) {
                                    if widgets::primary_button(ui, "Download Update").clicked() {
                                        dialog_action = Some(DialogAction::Download {
                                            version: latest_version.clone(),
                                            name: asset_name.clone(),
                                            url: url.clone(),
                                        });
                                    }
                                }
                                if widgets::button(ui, "View on GitHub").clicked() {
                                    let _ = open::that(release_url);
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if widgets::button(ui, "Later").clicked() {
                                        should_close = true;
                                    }
                                });
                            });
                        }
                        UpdateStatus::Downloading {
                            latest_version,
                            asset_name,
                            bytes_downloaded,
                            total_bytes,
                        } => {
                            ui.label(
                                RichText::new(format!("Downloading MLV Explorer v{}...", latest_version))
                                    .strong()
                                    .size(FONT_TITLE)
                                    .color(text_primary(ui)),
                            );
                            ui.label(RichText::new(asset_name).size(FONT_CAPTION).color(text_muted(ui)));

                            let (fraction, progress_str) = match total_bytes {
                                Some(total) if *total > 0 => {
                                    let f = (*bytes_downloaded as f32 / *total as f32).clamp(0.0, 1.0);
                                    let dl_mb = *bytes_downloaded as f64 / (1024.0 * 1024.0);
                                    let tot_mb = *total as f64 / (1024.0 * 1024.0);
                                    (f, format!("{:.1} / {:.1} MB ({:.0}%)", dl_mb, tot_mb, f * 100.0))
                                }
                                _ => {
                                    let dl_mb = *bytes_downloaded as f64 / (1024.0 * 1024.0);
                                    (0.0, format!("{:.1} MB downloaded", dl_mb))
                                }
                            };

                            ui.add(egui::ProgressBar::new(fraction).text(progress_str));

                            ui.add_space(8.0);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if widgets::button(ui, "Cancel").clicked() {
                                    dialog_action = Some(DialogAction::Cancel);
                                }
                            });
                        }
                        UpdateStatus::Downloaded {
                            latest_version,
                            asset_name: _,
                            file_path,
                        } => {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(icon::CHECK_CIRCLE).size(28.0).color(FLAG_PICKED));
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("Download Completed!")
                                            .strong()
                                            .size(FONT_TITLE)
                                            .color(text_primary(ui)),
                                    );
                                    ui.label(
                                        RichText::new(format!("v{} is ready to use.", latest_version))
                                            .size(FONT_CAPTION)
                                            .color(text_muted(ui)),
                                    );
                                });
                            });

                            ui.separator();
                            ui.label(RichText::new(format!("Saved to: {}", file_path.display())).size(FONT_CAPTION).color(text_secondary(ui)));

                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if widgets::primary_button(ui, "Open File").clicked() {
                                    let _ = open::that(file_path);
                                }
                                if let Some(parent) = file_path.parent() {
                                    if widgets::button(ui, "Open Folder").clicked() {
                                        let _ = open::that(parent);
                                    }
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if widgets::button(ui, "Done").clicked() {
                                        should_close = true;
                                    }
                                });
                            });
                        }
                        UpdateStatus::Error(err) => {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(icon::WARNING).size(28.0).color(FLAG_REJECTED));
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new("Update Check Failed")
                                            .strong()
                                            .size(FONT_TITLE)
                                            .color(FLAG_REJECTED),
                                    );
                                    ui.label(RichText::new(err).size(FONT_CAPTION).color(text_muted(ui)));
                                });
                            });

                            ui.separator();
                            ui.horizontal(|ui| {
                                if widgets::button(ui, "Try Again").clicked() {
                                    dialog_action = Some(DialogAction::Check);
                                }
                                if widgets::button(ui, "Open Releases on GitHub").clicked() {
                                    let _ = open::that(format!("https://github.com/{}/releases", GITHUB_REPO));
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if widgets::button(ui, "Close").clicked() {
                                        should_close = true;
                                    }
                                });
                            });
                        }
                    }
                });

            if let Some(action) = dialog_action {
                match action {
                    DialogAction::Check => updater.check_for_updates(),
                    DialogAction::Cancel => updater.cancel(),
                    DialogAction::Download { version, name, url } => {
                        updater.download_update(version, name, url);
                    }
                }
            }

            self.show_updater = is_open && !should_close;
        }
    }
}
