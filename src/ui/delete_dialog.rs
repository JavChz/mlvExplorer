use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{Color32, Context, RichText, Window};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeleteMode {
    RemoveFromCatalog,
    MoveToTrash,
}

pub struct DeleteConfirmDialog {
    pub is_open: bool,
    pub mode: DeleteMode,
    pub target_indices: Vec<usize>,
    pub file_names: Vec<String>,
}

impl Default for DeleteConfirmDialog {
    fn default() -> Self {
        Self {
            is_open: false,
            mode: DeleteMode::RemoveFromCatalog,
            target_indices: Vec::new(),
            file_names: Vec::new(),
        }
    }
}

pub enum DeleteConfirmResult {
    Confirmed {
        mode: DeleteMode,
        target_indices: Vec<usize>,
    },
    Cancelled,
}

impl DeleteConfirmDialog {
    pub fn open(&mut self, mode: DeleteMode, indices: Vec<usize>, file_names: Vec<String>) {
        self.mode = mode;
        self.target_indices = indices;
        self.file_names = file_names;
        self.is_open = true;
    }

    pub fn show(&mut self, ctx: &Context) -> Option<DeleteConfirmResult> {
        if !self.is_open {
            return None;
        }

        let mut result = None;
        let count = self.target_indices.len();

        let (title_glyph, title_text) = match self.mode {
            DeleteMode::RemoveFromCatalog => {
                if count <= 1 {
                    (icon::PROHIBIT, "Remove Clip from Catalog")
                } else {
                    (icon::PROHIBIT, "Remove Clips from Catalog")
                }
            }
            DeleteMode::MoveToTrash => {
                if count <= 1 {
                    (icon::TRASH, "Move Clip to Trash")
                } else {
                    (icon::TRASH, "Move Clips to Trash")
                }
            }
        };

        let title = widgets::icon_label(title_glyph, title_text);

        let mut is_open = self.is_open;
        let mut should_close = false;

        Window::new(RichText::new(title).strong().size(FONT_BODY))
            .open(&mut is_open)
            .resizable(false)
            .collapsible(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .fixed_size([460.0, 220.0])
            .show(ctx, |ui| {
                match self.mode {
                    DeleteMode::RemoveFromCatalog => {
                        ui.label(
                            RichText::new(format!(
                                "Are you sure you want to remove {} clip{} from the catalog?",
                                count,
                                if count == 1 { "" } else { "s" }
                            ))
                            .strong()
                            .color(text_primary(ui))
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "This will only remove the metadata, ratings, and tags from the catalog database.\nYour original RAW files on disk will NOT be deleted or moved."
                            )
                            .size(FONT_SMALL)
                            .color(text_secondary(ui))
                        );
                    }
                    DeleteMode::MoveToTrash => {
                        ui.label(
                            RichText::new(format!(
                                "Are you sure you want to move {} clip{} to the Trash / Recycle Bin?",
                                count,
                                if count == 1 { "" } else { "s" }
                            ))
                            .strong()
                            .color(FLAG_REJECTED)
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "This will move the actual MLV file(s) and any spanned companion slices (.M00, .M01) on disk directly into your operating system's Trash."
                            )
                            .size(FONT_SMALL)
                            .color(text_secondary(ui))
                        );
                    }
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Preview clip names (up to 4)
                ui.label(RichText::new("Target clips:").size(FONT_CAPTION).color(text_muted(ui)));
                egui::ScrollArea::vertical().max_height(60.0).show(ui, |ui| {
                    for name in self.file_names.iter().take(6) {
                        ui.label(RichText::new(format!("  • {}", name)).size(FONT_SMALL).monospace().color(text_primary(ui)));
                    }
                    if self.file_names.len() > 6 {
                        ui.label(
                            RichText::new(format!("    … and {} more", self.file_names.len() - 6))
                                .size(FONT_CAPTION)
                                .color(text_muted(ui))
                        );
                    }
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let confirm_btn = match self.mode {
                            DeleteMode::RemoveFromCatalog => {
                                egui::Button::new(RichText::new("Remove from Catalog").strong())
                                    .min_size(egui::Vec2::new(0.0, CONTROL_HEIGHT))
                            }
                            DeleteMode::MoveToTrash => {
                                egui::Button::new(
                                    RichText::new("Move to Trash")
                                        .color(Color32::WHITE)
                                        .strong()
                                )
                                .fill(DANGER_FILL)
                                .min_size(egui::Vec2::new(0.0, CONTROL_HEIGHT))
                            }
                        };

                        if ui.add(confirm_btn).clicked() {
                            should_close = true;
                            result = Some(DeleteConfirmResult::Confirmed {
                                mode: self.mode.clone(),
                                target_indices: self.target_indices.clone(),
                            });
                        }

                        if widgets::button(ui, "Cancel").clicked() {
                            should_close = true;
                            result = Some(DeleteConfirmResult::Cancelled);
                        }
                    });
                });
            });

        self.is_open = is_open && !should_close;
        result
    }
}
