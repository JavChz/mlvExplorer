use crate::db::{Database, TagNode, TagRecord};
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{Color32, Context, FontId, Layout, Pos2, Rect, RichText, Rounding, Stroke, Vec2, Window};

pub enum KeywordManagerAction {
    Changed,
}

#[derive(Default)]
pub struct KeywordManagerDialog {
    pub is_open: bool,
    pub status_message: Option<(String, bool)>, // (msg, is_error)
    new_tag_name: String,
    new_tag_parent_id: Option<i64>,
    editing_tag_id: Option<i64>,
    editing_tag_name: String,
    focus_rename_input: bool,
    adding_child_parent_id: Option<i64>,
    adding_child_name: String,
    focus_child_input: bool,
    reparenting_tag_id: Option<i64>,
    search_query: String,
    // Drag and drop state
    dragged_tag_id: Option<i64>,
    dragged_tag_name: Option<String>,
}

impl KeywordManagerDialog {
    pub fn open(&mut self) {
        self.is_open = true;
        self.status_message = None;
        self.new_tag_name.clear();
        self.new_tag_parent_id = None;
        self.editing_tag_id = None;
        self.editing_tag_name.clear();
        self.focus_rename_input = false;
        self.adding_child_parent_id = None;
        self.adding_child_name.clear();
        self.focus_child_input = false;
        self.reparenting_tag_id = None;
        self.search_query.clear();
        self.dragged_tag_id = None;
        self.dragged_tag_name = None;
    }

    pub fn show(&mut self, ctx: &Context, db: &Database) -> Option<KeywordManagerAction> {
        if !self.is_open {
            return None;
        }

        let mut action = None;
        let mut is_open = self.is_open;
        let mut should_close = false;
        let title = widgets::icon_label(icon::TAG, "Keyword Taxonomy Manager");

        Window::new(RichText::new(title).strong().size(FONT_BODY))
            .open(&mut is_open)
            .resizable(true)
            .collapsible(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .default_size([640.0, 560.0])
            .min_size([500.0, 420.0])
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 8.0);

                // Notification Banner
                if let Some((ref msg, is_err)) = self.status_message {
                    let col = if is_err { FLAG_REJECTED } else { FLAG_PICKED };
                    let bg = if is_err { Color32::from_rgb(48, 22, 22) } else { Color32::from_rgb(22, 42, 28) };
                    egui::Frame::none()
                        .fill(bg)
                        .stroke(Stroke::new(1.0_f32, col))
                        .rounding(Rounding::same(ROUNDING))
                        .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let glyph = if is_err { icon::WARNING_CIRCLE } else { icon::CHECK_CIRCLE };
                                ui.label(RichText::new(glyph).size(FONT_BODY).color(col));
                                ui.label(RichText::new(msg).size(FONT_SMALL).color(col));
                            });
                        });
                }

                // ── Top Header / Quick Add Keyword Card ──────────────────────────
                let tree_for_combo = db.get_tag_tree().unwrap_or_default();
                let flat_options = crate::db::flatten_tree_with_connectors(&tree_for_combo);

                egui::Frame::none()
                    .fill(bg_card(ui))
                    .stroke(Stroke::new(1.0_f32, border_subtle(ui)))
                    .rounding(Rounding::same(ROUNDING + 1.0))
                    .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
                            ui.label(RichText::new("Add Keyword:").strong().size(FONT_SMALL).color(text_primary(ui)));
                            let name_resp = ui.add(
                                egui::TextEdit::singleline(&mut self.new_tag_name)
                                    .hint_text("Enter name... [Enter]")
                                    .desired_width(155.0),
                            );

                            let enter_pressed = (name_resp.lost_focus() || name_resp.has_focus())
                                && ui.input(|i| i.key_pressed(egui::Key::Enter));

                            let current_parent_label = self
                                .new_tag_parent_id
                                .and_then(|pid| flat_options.iter().find(|t| t.id == pid))
                                .map(|t| t.name.as_str())
                                .unwrap_or("Root (Top Level)");

                            ui.label(RichText::new("Under:").size(FONT_CAPTION).color(text_muted(ui)));
                            egui::ComboBox::from_id_salt("new_keyword_parent_combo")
                                .selected_text(RichText::new(current_parent_label).size(FONT_SMALL))
                                .width(175.0)
                                .show_ui(ui, |ui| {
                                    if ui.selectable_label(self.new_tag_parent_id.is_none(), "Root (Top Level)").clicked() {
                                        self.new_tag_parent_id = None;
                                    }
                                    for opt in &flat_options {
                                        let is_sel = self.new_tag_parent_id == Some(opt.id);
                                        if ui.selectable_label(is_sel, &opt.display_label).clicked() {
                                            self.new_tag_parent_id = Some(opt.id);
                                        }
                                    }
                                });

                            let can_add = !self.new_tag_name.trim().is_empty();
                            let mut commit_new = enter_pressed && can_add;
                            ui.add_enabled_ui(can_add, |ui| {
                                if widgets::primary_button(ui, widgets::icon_label(icon::PLUS, "Create")).clicked() {
                                    commit_new = true;
                                }
                            });

                            if commit_new {
                                let name = self.new_tag_name.trim().to_string();
                                match db.create_tag(&name, self.new_tag_parent_id) {
                                    Ok(_) => {
                                        self.status_message = Some((format!("Created keyword '{}'", name), false));
                                        self.new_tag_name.clear();
                                        action = Some(KeywordManagerAction::Changed);
                                        if enter_pressed {
                                            name_resp.request_focus();
                                        }
                                    }
                                    Err(e) => {
                                        self.status_message = Some((format!("Error adding keyword: {e}"), true));
                                    }
                                }
                            }
                        });
                    });

                // ── Search & Hierarchy Sub-header ───────────────────────────────
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Hierarchy Tree").strong().size(FONT_BODY).color(text_primary(ui)));
                    ui.label(RichText::new("(Drag handle to re-parent · Hover card to nest)").size(FONT_CAPTION).color(text_muted(ui)));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !self.search_query.is_empty() {
                            if widgets::icon_button(ui, icon::X, false, "Clear search").clicked() {
                                self.search_query.clear();
                            }
                        }
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text("Filter keywords...")
                                .desired_width(140.0),
                        );
                    });
                });

                // ── Drop to Root Dropzone (Visible when dragging any keyword) ───
                if let Some(dragged_id) = self.dragged_tag_id {
                    let root_drop = ui.allocate_response(
                        Vec2::new(ui.available_width(), 32.0),
                        egui::Sense::hover(),
                    );
                    let is_hovered = root_drop.hovered();
                    let stroke = if is_hovered { Stroke::new(2.0_f32, ACCENT_HOVER) } else { Stroke::new(1.0_f32, border_strong(ui)) };
                    let fill = if is_hovered {
                        if ui.visuals().dark_mode { ACCENT_DIM } else { Color32::from_rgb(220, 234, 254) }
                    } else {
                        bg_card(ui)
                    };

                    ui.painter().rect(root_drop.rect, Rounding::same(ROUNDING), fill, stroke);
                    let text = if let Some(ref name) = self.dragged_tag_name {
                        format!("📥 Drop here to move '{}' to Top-Level (Root)", name)
                    } else {
                        "📥 Drop here to make Top-Level (Root)".to_string()
                    };
                    ui.painter().text(
                        root_drop.rect.center(),
                        egui::Align2::CENTER_CENTER,
                        text,
                        FontId::proportional(FONT_SMALL),
                        if is_hovered {
                            if ui.visuals().dark_mode { Color32::WHITE } else { ACCENT }
                        } else {
                            text_primary(ui)
                        },
                    );

                    if ui.input(|i| i.pointer.any_released()) && is_hovered {
                        match db.set_tag_parent(dragged_id, None) {
                            Ok(_) => {
                                if let Some(ref name) = self.dragged_tag_name {
                                    self.status_message = Some((format!("Promoted '{name}' to top-level root."), false));
                                }
                                action = Some(KeywordManagerAction::Changed);
                                ui.ctx().request_repaint();
                            }
                            Err(e) => {
                                self.status_message = Some((format!("Error moving keyword to root: {e}"), true));
                            }
                        }
                        self.dragged_tag_id = None;
                        self.dragged_tag_name = None;
                    }
                }

                // ── Scrollable Keyword Tree ─────────────────────────────────────
                egui::Frame::none()
                    .fill(bg_field(ui))
                    .stroke(Stroke::new(1.0_f32, border_subtle(ui)))
                    .rounding(Rounding::same(ROUNDING))
                    .inner_margin(egui::Margin::symmetric(8.0, 8.0))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(360.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let Ok(tree) = db.get_tag_tree() else {
                                    ui.label("Failed to load keyword hierarchy.");
                                    return;
                                };

                                let Ok(all_tags) = db.get_all_tags() else {
                                    return;
                                };

                                if tree.is_empty() {
                                    ui.add_space(24.0);
                                    ui.vertical_centered(|ui| {
                                        ui.label(RichText::new(icon::TAG).size(32.0).color(text_muted(ui)));
                                        ui.add_space(4.0);
                                        ui.label(RichText::new("No keywords in catalog yet.").italics().size(FONT_BODY).color(text_muted(ui)));
                                        ui.label(RichText::new("Add your first keyword using the field above.").size(FONT_CAPTION).color(text_muted(ui)));
                                    });
                                    ui.add_space(24.0);
                                } else {
                                    for root in &tree {
                                        self.render_tree_node(ui, root, &all_tags, &tree, db, 0, None, &mut action);
                                        ui.add_space(4.0);
                                    }
                                }
                            });
                    });

                // Clear drag state if mouse was released anywhere
                if ui.input(|i| i.pointer.any_released()) {
                    self.dragged_tag_id = None;
                    self.dragged_tag_name = None;
                }

                // Floating Drag Preview Badge following mouse cursor
                if let (Some(_drag_id), Some(ref drag_name)) = (self.dragged_tag_id, &self.dragged_tag_name) {
                    if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
                        let painter = ctx.layer_painter(egui::LayerId::new(
                            egui::Order::Tooltip,
                            egui::Id::new("keyword_drag_ghost"),
                        ));
                        let ghost_rect = Rect::from_min_size(pos + Vec2::new(14.0, 10.0), Vec2::new(140.0, 24.0));
                        painter.rect(
                            ghost_rect,
                            Rounding::same(ROUNDING),
                            ACCENT_DIM,
                            Stroke::new(1.0_f32, ACCENT_HOVER),
                        );
                        painter.text(
                            ghost_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("Moving '{}'...", drag_name),
                            FontId::proportional(FONT_SMALL),
                            Color32::WHITE,
                        );
                        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
                    }
                }

                // ── Footer ──────────────────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.label(RichText::new("💡 Tips: Drag by grip handle to nest. Click ➕ to add child, or ✏ to rename.").size(FONT_CAPTION).color(text_muted(ui)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::primary_button(ui, "Done").clicked() {
                            should_close = true;
                        }
                    });
                });
            });

        self.is_open = is_open && !should_close;
        action
    }

    fn render_tree_node(
        &mut self,
        ui: &mut egui::Ui,
        node: &TagNode,
        all_tags: &[TagRecord],
        full_tree: &[TagNode],
        db: &Database,
        depth: usize,
        parent_y_center: Option<f32>,
        action: &mut Option<KeywordManagerAction>,
    ) {
        let is_filter_match = self.search_query.is_empty()
            || node.name.to_lowercase().contains(&self.search_query.to_lowercase());

        let has_matching_child = !self.search_query.is_empty()
            && self.has_child_matching(node, &self.search_query.to_lowercase());

        if !self.search_query.is_empty() && !is_filter_match && !has_matching_child {
            return;
        }

        let is_being_dragged = self.dragged_tag_id == Some(node.id);
        let has_children = !node.children.is_empty();
        let is_root = depth == 0;

        let row_height = if is_root { 32.0 } else { 28.0 };
        let indent_per_level = 24.0_f32;
        let indent_offset = depth as f32 * indent_per_level;

        let available_w = (ui.available_width() - 4.0).max(160.0);
        let (full_row_rect, row_resp) = ui.allocate_exact_size(
            Vec2::new(available_w, row_height),
            egui::Sense::hover(),
        );

        // Visual Tree Connecting Guide Lines
        if depth > 0 {
            if let Some(parent_y) = parent_y_center {
                let stem_x = full_row_rect.min.x + indent_offset - (indent_per_level * 0.5);
                let row_y = full_row_rect.center().y;

                // Vertical line coming down from parent
                ui.painter().line_segment(
                    [Pos2::new(stem_x, parent_y), Pos2::new(stem_x, row_y)],
                    Stroke::new(1.0_f32, border_strong(ui)),
                );
                // Horizontal branch elbow reaching into card
                ui.painter().line_segment(
                    [Pos2::new(stem_x, row_y), Pos2::new(stem_x + 10.0, row_y)],
                    Stroke::new(1.0_f32, border_strong(ui)),
                );
            }
        }

        // Sub-rect for the interactive card (offset by tree indent)
        let card_rect = Rect::from_min_max(
            Pos2::new(full_row_rect.min.x + indent_offset, full_row_rect.min.y),
            full_row_rect.max,
        );

        // Check if this node is a valid drop target
        let can_be_drop_target = if let Some(dragged_id) = self.dragged_tag_id {
            if dragged_id == node.id {
                false
            } else if let Some(dragged_node) = Self::find_node_in_tree(full_tree, dragged_id) {
                // Cannot drop dragged node onto itself or any of its own descendants (would create a cycle)
                !Self::is_descendant_of(dragged_node, node.id)
            } else {
                true
            }
        } else {
            false
        };

        let is_drop_hovered = can_be_drop_target && card_rect.contains(ui.input(|i| i.pointer.latest_pos().unwrap_or(Pos2::ZERO)));

        // Handle drop event on this node
        if is_drop_hovered && ui.input(|i| i.pointer.any_released()) {
            if let Some(dragged_id) = self.dragged_tag_id {
                match db.set_tag_parent(dragged_id, Some(node.id)) {
                    Ok(_) => {
                        let name = self.dragged_tag_name.clone().unwrap_or_else(|| "Keyword".to_string());
                        self.status_message = Some((
                            format!("Nested '{name}' under '{}'", node.name),
                            false,
                        ));
                        *action = Some(KeywordManagerAction::Changed);
                        ui.ctx().request_repaint();
                    }
                    Err(e) => {
                        self.status_message = Some((
                            format!("Error moving keyword: {e}"),
                            true,
                        ));
                    }
                }
                self.dragged_tag_id = None;
                self.dragged_tag_name = None;
            }
        }

        // Card styling: root categories vs sub-keywords
        let is_dark = ui.visuals().dark_mode;
        let bg = if is_being_dragged {
            if is_dark { ACCENT_DIM } else { Color32::from_rgb(220, 234, 254) }
        } else if is_drop_hovered {
            if is_dark { Color32::from_rgb(34, 62, 102) } else { Color32::from_rgb(195, 220, 255) }
        } else if is_root {
            if row_resp.hovered() { bg_card_hover(ui) } else { bg_card(ui) }
        } else {
            if row_resp.hovered() {
                bg_card_hover(ui)
            } else if is_dark {
                Color32::from_rgb(30, 30, 33)
            } else {
                Color32::from_rgb(248, 250, 252)
            }
        };

        let stroke = if is_drop_hovered {
            Stroke::new(2.0_f32, ACCENT_HOVER)
        } else if is_being_dragged {
            Stroke::new(1.5_f32, ACCENT)
        } else if is_root {
            Stroke::new(1.0_f32, border_strong(ui))
        } else {
            Stroke::new(1.0_f32, border_subtle(ui))
        };

        ui.painter().rect(card_rect, Rounding::same(ROUNDING), bg, stroke);

        // Drop target banner indicator
        if is_drop_hovered {
            ui.painter().text(
                Pos2::new(card_rect.max.x - 12.0, card_rect.center().y),
                egui::Align2::RIGHT_CENTER,
                format!("Nest inside '{}' ↵", node.name),
                FontId::proportional(FONT_CAPTION),
                ACCENT_HOVER,
            );
        }

        // Card Inner Content
        let mut row_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(card_rect.shrink(3.0))
                .layout(Layout::left_to_right(egui::Align::Center)),
        );

        // 1. Grip Drag Handle (vector painted dots with grab cursor)
        let handle_resp = widgets::drag_handle(&mut row_ui, is_being_dragged);
        if handle_resp.drag_started() {
            self.dragged_tag_id = Some(node.id);
            self.dragged_tag_name = Some(node.name.clone());
        }

        // 2. Icon (Folder/Tags for categories, Tag for leaves)
        let (icon_glyph, icon_color) = if is_root && has_children {
            (icon::FOLDER_SIMPLE, RATING)
        } else if is_root {
            (icon::TAG, ACCENT)
        } else {
            (icon::TAG, text_secondary(ui))
        };
        row_ui.label(RichText::new(icon_glyph).size(FONT_BODY).color(icon_color));

        // 3. Name or Inline Rename TextEdit
        if self.editing_tag_id == Some(node.id) {
            let edit = row_ui.add(
                egui::TextEdit::singleline(&mut self.editing_tag_name)
                    .desired_width(140.0),
            );
            if self.focus_rename_input {
                edit.request_focus();
                self.focus_rename_input = false;
            }
            let enter_pressed = (edit.lost_focus() || edit.has_focus())
                && row_ui.input(|i| i.key_pressed(egui::Key::Enter));
            let esc_pressed = row_ui.input(|i| i.key_pressed(egui::Key::Escape));

            let mut save_rename = enter_pressed;
            if widgets::icon_button(&mut row_ui, icon::CHECK, false, "Save name [Enter]").clicked() {
                save_rename = true;
            }
            if esc_pressed || widgets::icon_button(&mut row_ui, icon::X, false, "Cancel [Esc]").clicked() {
                self.editing_tag_id = None;
            } else if save_rename {
                if !self.editing_tag_name.trim().is_empty() {
                    let _ = db.rename_tag(&node.name, &self.editing_tag_name);
                    self.status_message = Some((format!("Renamed to '{}'", self.editing_tag_name.trim()), false));
                    *action = Some(KeywordManagerAction::Changed);
                }
                self.editing_tag_id = None;
            }
        } else {
            let text_weight = if is_root {
                RichText::new(&node.name).strong().size(FONT_BODY).color(text_primary(ui))
            } else {
                RichText::new(&node.name).size(FONT_BODY).color(text_primary(ui))
            };
            row_ui.label(text_weight);

            // Clip count pill badge
            let count_text = if node.children.is_empty() {
                format!("{} clips", node.direct_count)
            } else {
                format!("{} direct · {} total", node.direct_count, node.total_count)
            };
            row_ui.label(RichText::new(count_text).size(FONT_CAPTION).color(text_muted(ui)));
        }

        // 4. Action Buttons aligned to the right inside row
        row_ui.with_layout(Layout::right_to_left(egui::Align::Center), |row_ui| {
            // Delete button
            if widgets::icon_button(row_ui, icon::TRASH, false, "Delete keyword").clicked() {
                if let Err(e) = db.delete_keyword(&node.name) {
                    self.status_message = Some((format!("Error deleting: {e}"), true));
                } else {
                    self.status_message = Some((format!("Deleted keyword '{}'", node.name), false));
                    *action = Some(KeywordManagerAction::Changed);
                }
            }

            // Rename button
            if self.editing_tag_id != Some(node.id) {
                if widgets::icon_button(row_ui, icon::PENCIL_SIMPLE, false, "Rename keyword").clicked() {
                    self.editing_tag_id = Some(node.id);
                    self.editing_tag_name = node.name.clone();
                    self.focus_rename_input = true;
                }
            }

            // Quick Add Child under this node (opens inline input directly below this node)
            let is_adding_child_here = self.adding_child_parent_id == Some(node.id);
            if widgets::icon_button(row_ui, icon::PLUS, is_adding_child_here, "Add child keyword under this").clicked() {
                if is_adding_child_here {
                    self.adding_child_parent_id = None;
                } else {
                    self.adding_child_parent_id = Some(node.id);
                    self.adding_child_name.clear();
                    self.focus_child_input = true;
                }
            }

            // Quick Move to Root (Un-nest) if currently nested
            if node.parent_id.is_some() {
                if widgets::icon_button(row_ui, icon::ARROW_UP_RIGHT, false, "Promote to Root (Un-nest)").clicked() {
                    match db.set_tag_parent(node.id, None) {
                        Ok(_) => {
                            self.status_message = Some((format!("Promoted '{}' to top-level root.", node.name), false));
                            *action = Some(KeywordManagerAction::Changed);
                            ui.ctx().request_repaint();
                        }
                        Err(e) => {
                            self.status_message = Some((format!("Error promoting keyword: {e}"), true));
                        }
                    }
                }
            }

            // Reparent dropdown selector (hierarchical with tree connector lines)
            let is_reparenting = self.reparenting_tag_id == Some(node.id);
            if is_reparenting {
                let flat_opts = crate::db::flatten_tree_with_connectors(full_tree);
                egui::ComboBox::from_id_salt(("reparent_combo", node.id))
                    .selected_text(RichText::new("Parent...").size(FONT_CAPTION))
                    .width(160.0)
                    .show_ui(row_ui, |ui| {
                        if ui.selectable_label(node.parent_id.is_none(), "Root (Top Level)").clicked() {
                            match db.set_tag_parent(node.id, None) {
                                Ok(_) => {
                                    *action = Some(KeywordManagerAction::Changed);
                                    ui.ctx().request_repaint();
                                }
                                Err(e) => {
                                    self.status_message = Some((format!("Error setting root: {e}"), true));
                                }
                            }
                            self.reparenting_tag_id = None;
                        }
                        for opt in &flat_opts {
                            let is_disallowed = if opt.id == node.id {
                                true
                            } else if let Some(curr_node) = Self::find_node_in_tree(full_tree, node.id) {
                                Self::is_descendant_of(curr_node, opt.id)
                            } else {
                                false
                            };
                            if !is_disallowed {
                                let is_curr = node.parent_id == Some(opt.id);
                                if ui.selectable_label(is_curr, &opt.display_label).clicked() {
                                    match db.set_tag_parent(node.id, Some(opt.id)) {
                                        Ok(_) => {
                                            *action = Some(KeywordManagerAction::Changed);
                                            ui.ctx().request_repaint();
                                        }
                                        Err(e) => {
                                            self.status_message = Some((format!("Error changing parent: {e}"), true));
                                        }
                                    }
                                    self.reparenting_tag_id = None;
                                }
                            }
                        }
                    });
            } else {
                if widgets::icon_button(row_ui, icon::TREE_STRUCTURE, false, "Change Parent").clicked() {
                    self.reparenting_tag_id = Some(node.id);
                }
            }
        });

        let current_y_center = full_row_rect.center().y;

        // 4b. Inline "Add Child Keyword" Row directly below this parent node
        if self.adding_child_parent_id == Some(node.id) {
            let child_depth = depth + 1;
            let child_indent = child_depth as f32 * indent_per_level;
            let (child_full_rect, _) = ui.allocate_exact_size(
                Vec2::new(available_w, 28.0),
                egui::Sense::hover(),
            );
            let stem_x = child_full_rect.min.x + child_indent - (indent_per_level * 0.5);
            let row_y = child_full_rect.center().y;

            ui.painter().line_segment(
                [Pos2::new(stem_x, current_y_center), Pos2::new(stem_x, row_y)],
                Stroke::new(1.0_f32, ACCENT),
            );
            ui.painter().line_segment(
                [Pos2::new(stem_x, row_y), Pos2::new(stem_x + 10.0, row_y)],
                Stroke::new(1.0_f32, ACCENT),
            );

            let child_card_rect = Rect::from_min_max(
                Pos2::new(child_full_rect.min.x + child_indent, child_full_rect.min.y),
                child_full_rect.max,
            );
            ui.painter().rect(
                child_card_rect,
                Rounding::same(ROUNDING),
                bg_card(ui),
                Stroke::new(1.0_f32, ACCENT),
            );

            let mut child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(child_card_rect.shrink(3.0))
                    .layout(Layout::left_to_right(egui::Align::Center)),
            );
            child_ui.spacing_mut().item_spacing.x = 6.0;
            child_ui.add_space(4.0);
            child_ui.label(RichText::new(icon::TAG).size(FONT_SMALL).color(ACCENT));

            let child_edit = child_ui.add(
                egui::TextEdit::singleline(&mut self.adding_child_name)
                    .hint_text(format!("New child under '{}'… [Enter]", node.name))
                    .desired_width(180.0),
            );
            if self.focus_child_input {
                child_edit.request_focus();
                self.focus_child_input = false;
            }

            let enter_pressed = (child_edit.lost_focus() || child_edit.has_focus())
                && child_ui.input(|i| i.key_pressed(egui::Key::Enter));
            let esc_pressed = child_ui.input(|i| i.key_pressed(egui::Key::Escape));

            let can_save_child = !self.adding_child_name.trim().is_empty();
            let mut commit_child = enter_pressed && can_save_child;

            child_ui.add_enabled_ui(can_save_child, |ui| {
                if widgets::icon_button(ui, icon::CHECK, true, "Create child keyword [Enter]").clicked() {
                    commit_child = true;
                }
            });
            if esc_pressed || widgets::icon_button(&mut child_ui, icon::X, false, "Cancel [Esc]").clicked() {
                self.adding_child_parent_id = None;
                self.adding_child_name.clear();
            } else if commit_child {
                let child_name = self.adding_child_name.trim().to_string();
                match db.create_tag(&child_name, Some(node.id)) {
                    Ok(_) => {
                        self.status_message = Some((
                            format!("Created child keyword '{}' under '{}'", child_name, node.name),
                            false,
                        ));
                        self.adding_child_name.clear();
                        self.adding_child_parent_id = None;
                        *action = Some(KeywordManagerAction::Changed);
                    }
                    Err(e) => {
                        self.status_message = Some((format!("Error creating child keyword: {e}"), true));
                    }
                }
            }
        }

        // 5. Recursively render child nodes with current card center as guide stem
        for child in &node.children {
            self.render_tree_node(ui, child, all_tags, full_tree, db, depth + 1, Some(current_y_center), action);
        }
    }

    /// Check if target_id is equal to dragged_node.id or is one of dragged_node's descendants
    fn is_descendant_of(dragged_node: &TagNode, target_id: i64) -> bool {
        if dragged_node.id == target_id {
            return true;
        }
        for child in &dragged_node.children {
            if Self::is_descendant_of(child, target_id) {
                return true;
            }
        }
        false
    }

    /// Recursively find a node by its ID in the full tree
    fn find_node_in_tree<'a>(nodes: &'a [TagNode], id: i64) -> Option<&'a TagNode> {
        for n in nodes {
            if n.id == id {
                return Some(n);
            }
            if let Some(found) = Self::find_node_in_tree(&n.children, id) {
                return Some(found);
            }
        }
        None
    }

    fn has_child_matching(&self, node: &TagNode, query: &str) -> bool {
        for child in &node.children {
            if child.name.to_lowercase().contains(query) || self.has_child_matching(child, query) {
                return true;
            }
        }
        false
    }
}
