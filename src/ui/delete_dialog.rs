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

        let title = match self.mode {
            DeleteMode::RemoveFromCatalog => {
                if count <= 1 {
                    "🗑 Remove Clip from Catalog"
                } else {
                    "🗑 Remove Clips from Catalog"
                }
            }
            DeleteMode::MoveToTrash => {
                if count <= 1 {
                    "⚠️ Move Clip to Trash (Recycle Bin)"
                } else {
                    "⚠️ Move Clips to Trash (Recycle Bin)"
                }
            }
        };

        let mut is_open = self.is_open;
        let mut should_close = false;

        Window::new(RichText::new(title).strong())
            .open(&mut is_open)
            .resizable(false)
            .collapsible(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .fixed_size([460.0, 240.0])
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
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "This will only remove the metadata, ratings, and tags from MLV Explorer's library database.\nYour original RAW files on disk will NOT be touched or deleted."
                            )
                            .small()
                            .color(Color32::from_rgb(170, 180, 200))
                        );
                    }
                    DeleteMode::MoveToTrash => {
                        ui.label(
                            RichText::new(format!(
                                "⚠️ Are you sure you want to move {} clip{} to the Trash / Recycle Bin?",
                                count,
                                if count == 1 { "" } else { "s" }
                            ))
                            .strong()
                            .color(Color32::from_rgb(240, 100, 100))
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "This will move the actual MLV file(s) and any spanned companion slices (.M00, .M01) on disk directly into your operating system's Trash / Recycle Bin, and remove them from the catalog."
                            )
                            .small()
                            .color(Color32::from_rgb(230, 180, 180))
                        );
                    }
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // Preview clip names (up to 4, then summary)
                ui.label(RichText::new("Target clips:").small().strong());
                egui::ScrollArea::vertical().max_height(60.0).show(ui, |ui| {
                    for name in self.file_names.iter().take(6) {
                        ui.label(RichText::new(format!(" • {}", name)).small().monospace());
                    }
                    if self.file_names.len() > 6 {
                        ui.label(
                            RichText::new(format!("   ... and {} more", self.file_names.len() - 6))
                                .small()
                                .italics()
                        );
                    }
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let confirm_btn = match self.mode {
                            DeleteMode::RemoveFromCatalog => {
                                egui::Button::new(RichText::new("Remove from Catalog").strong())
                            }
                            DeleteMode::MoveToTrash => {
                                egui::Button::new(
                                    RichText::new("Move to Trash")
                                        .color(Color32::WHITE)
                                        .strong()
                                )
                                .fill(Color32::from_rgb(180, 40, 40))
                            }
                        };

                        if ui.add(confirm_btn).clicked() {
                            should_close = true;
                            result = Some(DeleteConfirmResult::Confirmed {
                                mode: self.mode.clone(),
                                target_indices: self.target_indices.clone(),
                            });
                        }

                        if ui.button("Cancel").clicked() {
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
