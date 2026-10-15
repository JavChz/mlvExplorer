use crate::db::{default_catalog_path, Database, Facets, FilterCriteria, PickFlag, VideoRecord};
use crate::gpu;
use crate::importer::{ImportMessage, ImportProgress, Importer};
use crate::launcher::ExternalLauncher;
use crate::mlv::{FrameSource, MlvClip};
use crate::playback::Playback;
use crate::thumbnails::ThumbnailManager;
use crate::ui::*;
use eframe::egui::{self, RichText};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum ViewMode {
    Grid,
    Player,
}

#[derive(Debug, Clone, Default)]
pub struct ClipSettingsClipboard {
    pub stretch_factor: Option<f32>,
    pub tags: Option<Vec<String>>,
    pub rating: Option<u8>,
    pub flag: Option<PickFlag>,
    pub notes: Option<String>,
    pub scene: Option<String>,
    pub shot: Option<String>,
    pub take: Option<String>,
    pub location: Option<String>,
    pub camera_unit: Option<String>,
}

pub struct MlvExplorerApp {
    config: crate::config::AppConfig,
    db: Database,
    view_mode: ViewMode,
    videos: Vec<VideoRecord>,
    selected_video_idx: Option<usize>,
    selected_indices: Vec<usize>,
    filter: FilterCriteria,
    facets: Facets,
    all_tags: Vec<String>,
    cameras: Vec<String>,
    lenses: Vec<String>,

    // Clipboard for copy/paste clip settings
    settings_clipboard: Option<ClipSettingsClipboard>,

    // Media & Playback
    thumb_mgr: ThumbnailManager,
    current_playback: Option<(i64, Playback)>, // (video_id, Playback)

    // UI Panels & Windows
    grid_view: GridView,
    player_view: PlayerView,
    inspector: Inspector,
    settings_dialog: SettingsDialog,
    delete_confirm_dialog: DeleteConfirmDialog,
    keyword_dialog: KeywordManagerDialog,
    ingest_dialog: IngestDialog,
    help_dialogs: HelpDialogs,
    update_manager: crate::updater::UpdateManager,

    // Background Importer
    import_tx: crossbeam_channel::Sender<ImportMessage>,
    import_rx: crossbeam_channel::Receiver<ImportMessage>,
    import_progress: Option<ImportProgress>,
    import_cancel_flag: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,

    // Status bar & notification
    gpu_info: String,
    status_bar_text: String,
}

impl MlvExplorerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Initialize GPU debayer shaders via wgpu
        let gpu_info = if let Some(render_state) = &cc.wgpu_render_state {
            gpu::init(render_state)
        } else {
            "Software Viewport".to_string()
        };

        // Load persistent application config
        let config = crate::config::AppConfig::load();

        // Apply theme according to configuration (Dark / Light)
        theme::apply_theme(&cc.egui_ctx, config.theme_mode);

        // Open SQLite catalog (last opened or default)
        let db_path = config.resolve_catalog_path();
        let db = Database::open(&db_path).unwrap_or_else(|_| {
            let def = default_catalog_path();
            Database::open(&def).expect("Failed to initialize default SQLite catalog")
        });

        let thumb_mgr = ThumbnailManager::new(db_path, cc.egui_ctx.clone());

        let (import_tx, import_rx) = crossbeam_channel::unbounded();

        let mut app = Self {
            config,
            db,
            view_mode: ViewMode::Grid,
            videos: Vec::new(),
            selected_video_idx: None,
            selected_indices: Vec::new(),
            filter: FilterCriteria::default(),
            facets: Facets::default(),
            all_tags: Vec::new(),
            cameras: Vec::new(),
            lenses: Vec::new(),
            settings_clipboard: None,
            thumb_mgr,
            current_playback: None,
            grid_view: GridView::default(),
            player_view: PlayerView::default(),
            inspector: Inspector::default(),
            settings_dialog: SettingsDialog::default(),
            delete_confirm_dialog: DeleteConfirmDialog::default(),
            keyword_dialog: KeywordManagerDialog::default(),
            ingest_dialog: IngestDialog::default(),
            help_dialogs: HelpDialogs::default(),
            update_manager: crate::updater::UpdateManager::default(),
            import_tx,
            import_rx,
            import_progress: None,
            import_cancel_flag: None,
            gpu_info,
            status_bar_text: "Ready. Drag and drop .MLV files or folders to import.".to_string(),
        };

        // If database was migrated from v1 (which had broken offsets), rescan all files
        if app.db.needs_rescan {
            app.rescan_all_files();
        }

        app.reload_catalog();
        app
    }

    /// Rescan all files in catalog to correct corrupted metadata from previous versions
    fn rescan_all_files(&mut self) {
        if let Ok(paths) = self.db.all_paths() {
            let mut count = 0;
            for p in &paths {
                if let Ok(clip) = MlvClip::open(p) {
                    let _ = self.db.upsert_video(&clip.meta);
                    count += 1;
                }
            }
            if count > 0 {
                self.status_bar_text = format!("Refreshed metadata for {} clips.", count);
            }
        }
    }

    /// Reload catalog data, facets, and filter criteria
    pub fn reload_catalog(&mut self) {
        if let Ok(videos) = self.db.get_videos(&self.filter) {
            self.videos = videos;
            if let Some(idx) = self.selected_video_idx {
                if idx >= self.videos.len() {
                    self.selected_video_idx = if self.videos.is_empty() { None } else { Some(0) };
                }
            }
            // Keep selected_indices valid within bounds
            self.selected_indices.retain(|&i| i < self.videos.len());
            if self.selected_indices.is_empty() {
                if let Some(idx) = self.selected_video_idx {
                    self.selected_indices.push(idx);
                }
            } else if self.selected_video_idx.is_none() {
                self.selected_video_idx = self.selected_indices.first().copied();
            }
        }

        self.refresh_facets();

        // Keep playback in sync with selected video
        self.sync_playback();
    }

    /// Refresh camera/lens facets and keyword tag list from database
    pub fn refresh_facets(&mut self) {
        if let Ok(facets) = self.db.facets(&self.filter) {
            self.cameras = facets.cameras.iter().map(|c| c.name.clone()).collect();
            self.lenses = facets.lenses.iter().map(|l| l.name.clone()).collect();
            self.facets = facets;
        }

        if let Ok(tags) = self.db.all_tag_names() {
            self.all_tags = tags;
        }
    }

    /// Synchronize active video playback engine with the selected video
    fn sync_playback(&mut self) {
        let Some(idx) = self.selected_video_idx else {
            self.current_playback = None;
            return;
        };

        let Some(vid) = self.videos.get(idx) else {
            self.current_playback = None;
            return;
        };

        let current_id = self.current_playback.as_ref().map(|(id, _)| *id);
        if current_id != Some(vid.id) {
            self.current_playback = None;
        }
    }

    /// Ensure active playback engine is loaded for current video
    fn ensure_playback(&mut self, ctx: &egui::Context) -> bool {
        let Some(idx) = self.selected_video_idx else { return false; };
        let Some(vid) = self.videos.get(idx) else { return false; };

        let needs_load = match &self.current_playback {
            Some((id, _)) => *id != vid.id,
            None => true,
        };

        if needs_load {
            match FrameSource::open(&vid.file_path) {
                Ok(source) => {
                    let pb = Playback::new(source, ctx.clone());
                    if let Ok(mut a) = pb.audio.lock() {
                        a.set_muted(self.player_view.is_muted);
                        a.set_volume(self.player_view.volume);
                        a.set_speed(self.player_view.playback_speed);
                    }
                    self.current_playback = Some((vid.id, pb));
                    self.player_view.current_frame = 0;
                    self.player_view.is_playing = false;
                    true
                }
                Err(e) => {
                    self.status_bar_text = format!("Error opening MLV: {e}");
                    false
                }
            }
        } else {
            true
        }
    }

    /// Start a background import of files or directories, skipping duplicates and tracking progress
    pub fn start_import(&mut self, paths: Vec<PathBuf>, ctx: &egui::Context) {
        let files = Importer::collect_mlv_files(paths);
        if files.is_empty() {
            self.status_bar_text = "No .MLV files found in selected path(s).".to_string();
            return;
        }

        let total = files.len();
        self.import_progress = Some(ImportProgress {
            current: 0,
            total,
            current_file: String::new(),
            imported: 0,
            duplicates: 0,
            failed: 0,
        });

        let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.import_cancel_flag = Some(cancel_flag.clone());

        Importer::start(
            files,
            self.db.path.clone(),
            self.import_tx.clone(),
            cancel_flag,
            ctx.clone(),
        );
    }

    /// Switch active SQLite catalog to a different database file
    pub fn switch_catalog(&mut self, new_path: PathBuf, ctx: &egui::Context) {
        match Database::open(&new_path) {
            Ok(db) => {
                self.db = db;
                self.config.last_catalog_path = Some(new_path.clone());
                self.config.save();
                self.thumb_mgr = ThumbnailManager::new(new_path.clone(), ctx.clone());
                self.current_playback = None;
                self.selected_video_idx = None;
                self.selected_indices.clear();
                self.reload_catalog();
                self.status_bar_text = format!("Switched to catalog: {}", new_path.display());
            }
            Err(e) => {
                self.status_bar_text = format!("Failed to open catalog {}: {e}", new_path.display());
            }
        }
    }

    /// Open a given clip file path in MLV App executable
    pub fn open_in_mlv_app(&mut self, file_path: &Path) {
        match ExternalLauncher::open_in_mlv_app(&self.db, self.config.mlv_app_path.as_ref(), file_path) {
            Ok(msg) => self.status_bar_text = msg,
            Err(e) => self.status_bar_text = e,
        }
    }

    /// Open multiple selected clips in a single MLV App instance / session playlist
    pub fn open_selection_in_mlv_app(&mut self, indices: &[usize]) {
        let paths: Vec<PathBuf> = indices
            .iter()
            .filter_map(|&idx| self.videos.get(idx))
            .map(|v| PathBuf::from(&v.file_path))
            .collect();

        if paths.is_empty() {
            self.status_bar_text = "No clips selected to open in MLV App.".to_string();
            return;
        }

        match ExternalLauncher::open_files_in_mlv_app(&self.db, self.config.mlv_app_path.as_ref(), &paths) {
            Ok(msg) => self.status_bar_text = msg,
            Err(e) => self.status_bar_text = e,
        }
    }

    /// Open all clips tagged with a specific keyword in a single MLV App instance / session playlist
    pub fn open_tag_in_mlv_app(&mut self, tag: &str) {
        let mut filter = FilterCriteria::default();
        filter.tag = Some(tag.to_string());
        match self.db.get_videos(&filter) {
            Ok(vids) => {
                let paths: Vec<PathBuf> = vids.into_iter().map(|v| PathBuf::from(v.file_path)).collect();
                if paths.is_empty() {
                    self.status_bar_text = format!("No clips found tagged with '{}'.", tag);
                } else {
                    match ExternalLauncher::open_files_in_mlv_app(&self.db, self.config.mlv_app_path.as_ref(), &paths) {
                        Ok(msg) => self.status_bar_text = format!("Keyword '{}': {}", tag, msg),
                        Err(e) => self.status_bar_text = e,
                    }
                }
            }
            Err(e) => self.status_bar_text = format!("Failed to query clips for keyword '{}': {}", tag, e),
        }
    }

    /// Open the entire catalog of clips in a single MLV App instance / session playlist
    pub fn open_catalog_in_mlv_app(&mut self) {
        match self.db.get_videos(&FilterCriteria::default()) {
            Ok(vids) => {
                let paths: Vec<PathBuf> = vids.into_iter().map(|v| PathBuf::from(v.file_path)).collect();
                if paths.is_empty() {
                    self.status_bar_text = "Catalog is empty. Import clips first.".to_string();
                } else {
                    match ExternalLauncher::open_files_in_mlv_app(&self.db, self.config.mlv_app_path.as_ref(), &paths) {
                        Ok(msg) => self.status_bar_text = format!("Catalog: {}", msg),
                        Err(e) => self.status_bar_text = e,
                    }
                }
            }
            Err(e) => self.status_bar_text = format!("Failed to query catalog clips: {}", e),
        }
    }

    /// Open all clips with a specific pick flag (e.g. Picks) in a single MLV App instance / session playlist
    pub fn open_flag_in_mlv_app(&mut self, flag: PickFlag) {
        let mut filter = FilterCriteria::default();
        filter.flag = Some(flag);
        match self.db.get_videos(&filter) {
            Ok(vids) => {
                let paths: Vec<PathBuf> = vids.into_iter().map(|v| PathBuf::from(v.file_path)).collect();
                if paths.is_empty() {
                    self.status_bar_text = format!("No {:?} clips found in catalog.", flag);
                } else {
                    match ExternalLauncher::open_files_in_mlv_app(&self.db, self.config.mlv_app_path.as_ref(), &paths) {
                        Ok(msg) => self.status_bar_text = format!("{:?}: {}", flag, msg),
                        Err(e) => self.status_bar_text = e,
                    }
                }
            }
            Err(e) => self.status_bar_text = format!("Failed to query {:?} clips: {}", flag, e),
        }
    }

    /// Open confirmation dialog to delete/remove specified indices
    pub fn prompt_delete(&mut self, mode: DeleteMode, indices: Vec<usize>) {
        if indices.is_empty() {
            return;
        }
        let targets: Vec<crate::ui::DeleteTargetClip> = indices
            .iter()
            .filter_map(|&idx| {
                self.videos.get(idx).map(|v| {
                    let main_path = PathBuf::from(&v.file_path);
                    let spanned_paths = crate::mlv::MlvClip::resolve_spanned_files(&main_path);
                    let files = spanned_paths
                        .into_iter()
                        .map(|p| {
                            let fname = p
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| v.file_name.clone());
                            let fpath = p.to_string_lossy().to_string();
                            (fname, fpath)
                        })
                        .collect();
                    crate::ui::DeleteTargetClip {
                        clip_name: v.file_name.clone(),
                        files,
                    }
                })
            })
            .collect();
        self.delete_confirm_dialog.open(mode, indices, targets);
    }

    /// Execute removal of clips from catalog
    pub fn execute_remove_from_catalog(&mut self, indices: &[usize]) {
        let video_ids: Vec<i64> = indices
            .iter()
            .filter_map(|&idx| self.videos.get(idx).map(|v| v.id))
            .collect();
        if video_ids.is_empty() {
            return;
        }

        match self.db.remove_videos(&video_ids) {
            Ok(count) => {
                self.status_bar_text = format!("Removed {} clip(s) from catalog.", count);
                self.selected_indices.clear();
                self.selected_video_idx = None;
                self.reload_catalog();
            }
            Err(e) => {
                self.status_bar_text = format!("Error removing clips from catalog: {e}");
            }
        }
    }

    /// Execute moving clips (including any spanned slices) to OS Trash/Recycle Bin and remove from catalog
    pub fn execute_move_to_trash(&mut self, indices: &[usize]) {
        let mut all_files_to_trash: Vec<PathBuf> = Vec::new();
        let mut video_ids: Vec<i64> = Vec::new();

        for &idx in indices {
            if let Some(vid) = self.videos.get(idx) {
                video_ids.push(vid.id);
                let main_path = PathBuf::from(&vid.file_path);
                for chunk_path in crate::mlv::MlvClip::resolve_spanned_files(&main_path) {
                    if chunk_path.exists() && !all_files_to_trash.contains(&chunk_path) {
                        all_files_to_trash.push(chunk_path);
                    }
                }
            }
        }

        let mut trash_errors = Vec::new();
        let mut trashed_count = 0;

        for path in &all_files_to_trash {
            match trash::delete(path) {
                Ok(_) => {
                    trashed_count += 1;
                }
                Err(e) => {
                    trash_errors.push(format!("{}: {e}", path.file_name().unwrap_or_default().to_string_lossy()));
                }
            }
        }

        // Always remove successfully processed video entries from catalog
        let _ = self.db.remove_videos(&video_ids);
        self.selected_indices.clear();
        self.selected_video_idx = None;
        self.reload_catalog();

        if trash_errors.is_empty() {
            self.status_bar_text = format!("Moved {} file(s) to Trash and removed from catalog.", trashed_count);
        } else {
            self.status_bar_text = format!(
                "Moved {} files to Trash with {} error(s): {}",
                trashed_count,
                trash_errors.len(),
                trash_errors.join(", ")
            );
        }
    }

    /// Copy settings from currently selected clip into clipboard
    pub fn copy_clip_settings(&mut self) {
        let Some(idx) = self.selected_video_idx else {
            self.status_bar_text = "Select a clip to copy settings.".to_string();
            return;
        };
        let Some(vid) = self.videos.get(idx) else { return; };

        self.settings_clipboard = Some(ClipSettingsClipboard {
            stretch_factor: Some(vid.stretch_factor),
            tags: Some(vid.tags.clone()),
            rating: Some(vid.rating),
            flag: Some(vid.flag),
            notes: Some(vid.notes.clone()),
            scene: vid.scene.clone(),
            shot: vid.shot.clone(),
            take: vid.take.clone(),
            location: vid.location.clone(),
            camera_unit: vid.camera_unit.clone(),
        });

        self.status_bar_text = format!("Copied settings from clip '{}'", vid.file_name);
    }

    /// Paste settings from clipboard to all currently selected clips
    pub fn paste_clip_settings(&mut self) {
        let Some(clip) = self.settings_clipboard.clone() else {
            self.status_bar_text = "Clip settings clipboard is empty. Copy settings first (Ctrl+C).".to_string();
            return;
        };

        let target_indices = if !self.selected_indices.is_empty() {
            self.selected_indices.clone()
        } else if let Some(idx) = self.selected_video_idx {
            vec![idx]
        } else {
            Vec::new()
        };

        if target_indices.is_empty() {
            self.status_bar_text = "No clip(s) selected to paste settings into.".to_string();
            return;
        }

        let count = target_indices.len();
        for &idx in &target_indices {
            if let Some(vid) = self.videos.get_mut(idx) {
                if let Some(stretch) = clip.stretch_factor {
                    vid.stretch_factor = stretch;
                    let _ = self.db.set_stretch_factor(vid.id, stretch);
                }
                if let Some(rating) = clip.rating {
                    vid.rating = rating;
                    let _ = self.db.set_rating(vid.id, rating);
                }
                if let Some(flag) = clip.flag {
                    vid.flag = flag;
                    let _ = self.db.set_flag(vid.id, flag);
                }
                if let Some(ref notes) = clip.notes {
                    vid.notes = notes.clone();
                    let _ = self.db.set_notes(vid.id, notes);
                }
                if clip.scene.is_some() || clip.shot.is_some() || clip.take.is_some() || clip.location.is_some() || clip.camera_unit.is_some() {
                    vid.scene = clip.scene.clone();
                    vid.shot = clip.shot.clone();
                    vid.take = clip.take.clone();
                    vid.location = clip.location.clone();
                    vid.camera_unit = clip.camera_unit.clone();
                    let _ = self.db.set_production_slate(
                        vid.id,
                        clip.scene.as_deref(),
                        clip.shot.as_deref(),
                        clip.take.as_deref(),
                        clip.location.as_deref(),
                        clip.camera_unit.as_deref(),
                    );
                }
                if let Some(ref tags) = clip.tags {
                    for tag in tags {
                        if !vid.tags.contains(tag) {
                            vid.tags.push(tag.clone());
                            let _ = self.db.add_tag(vid.id, tag);
                        }
                    }
                }
            }
        }

        self.refresh_facets();
        self.status_bar_text = format!("Pasted clip settings to {} selected clip(s).", count);
    }

    /// Navigate from Player view back to Grid view cleanly (pauses audio, exits fullscreen, resets state)
    pub fn navigate_to_grid(&mut self, ctx: &egui::Context) {
        self.view_mode = ViewMode::Grid;
        self.player_view.is_fullscreen = false;
        self.player_view.is_playing = false;
        if let Some((_, ref pb)) = self.current_playback {
            if let Ok(mut a) = pb.audio.lock() {
                a.pause();
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        ctx.request_repaint();
    }

    /// Navigate from Grid view to Cinema Player view for current selected clip
    pub fn navigate_to_player(&mut self, ctx: &egui::Context) {
        if self.selected_video_idx.is_some() {
            self.view_mode = ViewMode::Player;
            self.ensure_playback(ctx);
            ctx.request_repaint();
        }
    }

    /// Hierarchical Go Back navigation:
    /// 1. Close open modal dialogs
    /// 2. If in Player view, return to Grid view
    pub fn navigate_back(&mut self, ctx: &egui::Context) -> bool {
        if self.keyword_dialog.is_open {
            self.keyword_dialog.is_open = false;
            return true;
        }
        if self.settings_dialog.is_open {
            self.settings_dialog.is_open = false;
            return true;
        }
        if self.delete_confirm_dialog.is_open {
            self.delete_confirm_dialog.is_open = false;
            return true;
        }
        if self.help_dialogs.show_shortcuts || self.help_dialogs.show_about || self.help_dialogs.show_updater {
            self.help_dialogs.show_shortcuts = false;
            self.help_dialogs.show_about = false;
            self.help_dialogs.show_updater = false;
            return true;
        }
        if self.ingest_dialog.is_open {
            if !self.ingest_dialog.is_ingesting {
                self.ingest_dialog.is_open = false;
                return true;
            }
        }

        if self.view_mode == ViewMode::Player {
            self.navigate_to_grid(ctx);
            return true;
        }

        false
    }

    /// Go Forward navigation:
    /// If in Grid view and a clip is selected, navigate into Player view
    pub fn navigate_forward(&mut self, ctx: &egui::Context) -> bool {
        if self.view_mode == ViewMode::Grid && self.selected_video_idx.is_some() {
            self.navigate_to_player(ctx);
            return true;
        }
        false
    }

    /// Handle mouse side buttons (Back: Extra1, Forward: Extra2) and browser-style navigation keys (Alt+Left, Alt+Right, Esc)
    fn handle_navigation_input(&mut self, ctx: &egui::Context) {
        // 1. Mouse side buttons (Back: Extra1, Forward: Extra2)
        // Checked unconditionally so mouse side clicks work even when a text field has focus.
        let (mouse_back, mouse_forward) = ctx.input(|i| {
            let back = i.pointer.button_pressed(egui::PointerButton::Extra1)
                || i.raw.events.iter().any(|e| matches!(e, egui::Event::PointerButton { button: egui::PointerButton::Extra1, pressed: true, .. }));
            let fwd = i.pointer.button_pressed(egui::PointerButton::Extra2)
                || i.raw.events.iter().any(|e| matches!(e, egui::Event::PointerButton { button: egui::PointerButton::Extra2, pressed: true, .. }));
            (back, fwd)
        });

        if mouse_back {
            self.navigate_back(ctx);
            return;
        }

        if mouse_forward {
            self.navigate_forward(ctx);
            return;
        }

        // 2. Keyboard Back / Forward navigation (Alt+Left, Alt+Right, Escape)
        // Only handled when NOT typing into a text field.
        if !ctx.wants_keyboard_input() {
            let (alt_left, alt_right, escape) = ctx.input(|i| {
                let left = i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft);
                let right = i.modifiers.alt && i.key_pressed(egui::Key::ArrowRight);
                let esc = i.key_pressed(egui::Key::Escape);
                (left, right, esc)
            });

            if alt_left {
                self.navigate_back(ctx);
            } else if alt_right {
                self.navigate_forward(ctx);
            } else if escape {
                // If any modal is open, Escape closes it
                let has_modal = self.keyword_dialog.is_open
                    || self.settings_dialog.is_open
                    || self.delete_confirm_dialog.is_open
                    || (self.ingest_dialog.is_open && !self.ingest_dialog.is_ingesting)
                    || self.help_dialogs.show_shortcuts
                    || self.help_dialogs.show_about
                    || self.help_dialogs.show_updater;

                if has_modal {
                    self.navigate_back(ctx);
                }
            }
        }
    }

    /// Handle drag-and-drop file imports from OS
    fn handle_drag_and_drop(&mut self, ctx: &egui::Context) {
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            let paths: Vec<PathBuf> = dropped_files.into_iter().filter_map(|f| f.path).collect();
            if !paths.is_empty() {
                self.start_import(paths, ctx);
            }
        }
    }

    /// Global keyboard shortcuts
    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        // Suppress hotkeys when user is typing in a text field (Search, Scene, Location, Notes, Keywords, etc.)
        if ctx.wants_keyboard_input() {
            return;
        }

        let input = ctx.input(|i| i.clone());

        // Mode switching: G -> Grid, E / Enter -> Player, F -> Fullscreen Player
        if input.key_pressed(egui::Key::G) {
            self.navigate_to_grid(ctx);
        }
        if input.key_pressed(egui::Key::E) || (self.view_mode == ViewMode::Grid && input.key_pressed(egui::Key::Enter)) {
            self.navigate_to_player(ctx);
        }
        if input.key_pressed(egui::Key::F) {
            if self.view_mode == ViewMode::Grid {
                if self.selected_video_idx.is_some() {
                    self.view_mode = ViewMode::Player;
                    self.player_view.is_fullscreen = true;
                    self.player_view.show_overlay_controls = false;
                    self.ensure_playback(ctx);
                }
            }
        }

        // Ctrl+D / Cmd+D: Deselect all clips
        if (input.modifiers.command || input.modifiers.ctrl) && input.key_pressed(egui::Key::D) {
            self.selected_indices.clear();
            self.selected_video_idx = None;
        }

        // Ctrl+A / Cmd+A: Select all clips
        if (input.modifiers.command || input.modifiers.ctrl) && input.key_pressed(egui::Key::A) {
            self.selected_indices = (0..self.videos.len()).collect();
            if self.selected_video_idx.is_none() && !self.videos.is_empty() {
                self.selected_video_idx = Some(0);
            }
        }

        // Ctrl+C / Cmd+C: Copy clip settings
        if (input.modifiers.command || input.modifiers.ctrl) && input.key_pressed(egui::Key::C) {
            self.copy_clip_settings();
        }

        // Ctrl+V / Cmd+V: Paste clip settings
        if (input.modifiers.command || input.modifiers.ctrl) && input.key_pressed(egui::Key::V) {
            self.paste_clip_settings();
        }

        // Ctrl+I / Cmd+I: Ingest from SD / Card
        if (input.modifiers.command || input.modifiers.ctrl) && input.key_pressed(egui::Key::I) {
            self.ingest_dialog.open(&self.config, &self.db, ctx);
        }

        // Ctrl+M / Cmd+M: Open selected clip(s) in MLV App
        if (input.modifiers.command || input.modifiers.ctrl) && input.key_pressed(egui::Key::M) {
            let targets = if !self.selected_indices.is_empty() {
                self.selected_indices.clone()
            } else if let Some(idx) = self.selected_video_idx {
                vec![idx]
            } else {
                Vec::new()
            };
            if !targets.is_empty() {
                self.open_selection_in_mlv_app(&targets);
            }
        }

        // F1: Open shortcuts help
        if input.key_pressed(egui::Key::F1) {
            self.help_dialogs.show_shortcuts = true;
        }

        // Delete / Backspace: Remove selected clip(s) from catalog (with confirmation)
        if !self.delete_confirm_dialog.is_open && (input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace)) {
            let target_indices = if !self.selected_indices.is_empty() {
                self.selected_indices.clone()
            } else if let Some(idx) = self.selected_video_idx {
                vec![idx]
            } else {
                Vec::new()
            };
            if !target_indices.is_empty() {
                self.prompt_delete(DeleteMode::RemoveFromCatalog, target_indices);
            }
        }

        // Navigation in Grid (Left / Right / Up / Down)
        if self.view_mode == ViewMode::Grid && !self.videos.is_empty() {
            if input.key_pressed(egui::Key::ArrowRight) {
                let next_idx = match self.selected_video_idx {
                    Some(idx) if idx + 1 < self.videos.len() => Some(idx + 1),
                    Some(idx) => Some(idx),
                    None => Some(0),
                };
                if let Some(idx) = next_idx {
                    self.selected_video_idx = Some(idx);
                    self.selected_indices = vec![idx];
                }
            }
            if input.key_pressed(egui::Key::ArrowLeft) {
                let prev_idx = match self.selected_video_idx {
                    Some(idx) if idx > 0 => Some(idx - 1),
                    Some(idx) => Some(idx),
                    None => Some(0),
                };
                if let Some(idx) = prev_idx {
                    self.selected_video_idx = Some(idx);
                    self.selected_indices = vec![idx];
                }
            }
        }

        // Rating & Flags shortcuts on selected video(s)
        let targets = if !self.selected_indices.is_empty() {
            self.selected_indices.clone()
        } else if let Some(idx) = self.selected_video_idx {
            vec![idx]
        } else {
            Vec::new()
        };

        if !targets.is_empty() {
            let mut rating_to_set = None;
            if input.key_pressed(egui::Key::Num1) { rating_to_set = Some(1); }
            if input.key_pressed(egui::Key::Num2) { rating_to_set = Some(2); }
            if input.key_pressed(egui::Key::Num3) { rating_to_set = Some(3); }
            if input.key_pressed(egui::Key::Num4) { rating_to_set = Some(4); }
            if input.key_pressed(egui::Key::Num5) { rating_to_set = Some(5); }
            if input.key_pressed(egui::Key::Num0) { rating_to_set = Some(0); }

            if let Some(r) = rating_to_set {
                for &idx in &targets {
                    if let Some(video) = self.videos.get_mut(idx) {
                        video.rating = r;
                        let _ = self.db.set_rating(video.id, r);
                    }
                }
            }

            // Flags: P -> Pick, X -> Reject, U -> Unflag
            if input.key_pressed(egui::Key::P) {
                for &idx in &targets {
                    if let Some(video) = self.videos.get_mut(idx) {
                        video.flag = PickFlag::Picked;
                        let _ = self.db.set_flag(video.id, PickFlag::Picked);
                    }
                }
            }
            if input.key_pressed(egui::Key::X) {
                for &idx in &targets {
                    if let Some(video) = self.videos.get_mut(idx) {
                        video.flag = PickFlag::Rejected;
                        let _ = self.db.set_flag(video.id, PickFlag::Rejected);
                    }
                }
            }
            if input.key_pressed(egui::Key::U) {
                for &idx in &targets {
                    if let Some(video) = self.videos.get_mut(idx) {
                        video.flag = PickFlag::Unflagged;
                        let _ = self.db.set_flag(video.id, PickFlag::Unflagged);
                    }
                }
            }
        }
    }
}

impl eframe::App for MlvExplorerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll incoming import progress messages
        while let Ok(msg) = self.import_rx.try_recv() {
            match msg {
                ImportMessage::Progress { current, total, current_file, imported, duplicates, failed } => {
                    self.import_progress = Some(ImportProgress {
                        current,
                        total,
                        current_file,
                        imported,
                        duplicates,
                        failed,
                    });
                }
                ImportMessage::Complete { total: _, imported, duplicates, failed } => {
                    self.import_progress = None;
                    self.import_cancel_flag = None;
                    let mut summary = format!("Import complete: {} added", imported);
                    if duplicates > 0 {
                        summary.push_str(&format!(", {} duplicates skipped", duplicates));
                    }
                    if failed > 0 {
                        summary.push_str(&format!(", {} failed", failed));
                    }
                    self.status_bar_text = summary;
                    self.reload_catalog();
                }
                ImportMessage::Cancelled { total: _, imported, duplicates, failed: _ } => {
                    self.import_progress = None;
                    self.import_cancel_flag = None;
                    let mut summary = format!("Import cancelled: {} added", imported);
                    if duplicates > 0 {
                        summary.push_str(&format!(", {} duplicates skipped", duplicates));
                    }
                    self.status_bar_text = summary;
                    self.reload_catalog();
                }
            }
        }

        self.thumb_mgr.update(ctx);
        self.update_manager.update();
        match &self.update_manager.status {
            crate::updater::UpdateStatus::UpdateAvailable { latest_version, .. } => {
                if !self.status_bar_text.starts_with("Update available") {
                    self.status_bar_text = format!("Update available: v{latest_version}. Check Help menu.");
                }
            }
            crate::updater::UpdateStatus::UpToDate { current_version } => {
                if self.status_bar_text == "Checking for updates..." {
                    self.status_bar_text = format!("MLV Explorer is up to date (v{current_version}).");
                }
            }
            crate::updater::UpdateStatus::Error(e) => {
                if self.status_bar_text == "Checking for updates..." {
                    self.status_bar_text = format!("Update check failed: {e}");
                }
            }
            _ => {}
        }

        self.handle_drag_and_drop(ctx);
        self.handle_navigation_input(ctx);
        self.handle_keyboard_shortcuts(ctx);

        let has_modal_open = self.keyword_dialog.is_open
            || self.settings_dialog.is_open
            || self.delete_confirm_dialog.is_open
            || self.ingest_dialog.is_open
            || self.help_dialogs.show_shortcuts
            || self.help_dialogs.show_about
            || self.help_dialogs.show_updater;

        let is_fullscreen_player = self.view_mode == ViewMode::Player && self.player_view.is_fullscreen;

        // Sync fullscreen state with window viewport
        let was_fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        if was_fullscreen != is_fullscreen_player {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(is_fullscreen_player));
        }

        // 1. Studio Top Menu Bar (Native File, Edit, View, Preferences, Help + Import & Grid/Player on right)
        if !is_fullscreen_player {
            let mut menu_action = None;
            egui::TopBottomPanel::top("app_menu_bar")
                .exact_height(30.0)
                .show(ctx, |ui| {
                    let can_open_player = self.selected_video_idx.is_some();
                    let is_player_mode = self.view_mode == ViewMode::Player;
                    menu_action = TopBar::show_menu_bar(ui, is_player_mode, can_open_player);
                });

            if let Some(act) = menu_action {
                match act {
                    // File
                    TopBarAction::NewCatalog => {
                        if let Some(new_path) = rfd::FileDialog::new()
                            .set_title("Create New MLV Explorer Catalog")
                            .set_file_name("my_mlv_catalog.db")
                            .add_filter("SQLite Catalog (.db)", &["db", "sqlite"])
                            .save_file()
                        {
                            self.switch_catalog(new_path, ctx);
                        }
                    }
                    TopBarAction::OpenCatalog => {
                        if let Some(open_path) = rfd::FileDialog::new()
                            .set_title("Open MLV Explorer Catalog")
                            .add_filter("SQLite Catalog (.db)", &["db", "sqlite"])
                            .pick_file()
                        {
                            self.switch_catalog(open_path, ctx);
                        }
                    }
                    TopBarAction::ImportExistingCatalog => {
                        if let Some(import_path) = rfd::FileDialog::new()
                            .set_title("Import Another Catalog into Current Library")
                            .add_filter("Catalog (.db, .json)", &["db", "sqlite", "json"])
                            .pick_file()
                        {
                            let ext = import_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                            if ext == "json" {
                                match self.db.import_catalog_json(&import_path) {
                                    Ok(count) => {
                                        self.status_bar_text = format!("Merged {count} clips from JSON catalog");
                                        self.reload_catalog();
                                    }
                                    Err(e) => {
                                        self.status_bar_text = format!("Error importing JSON catalog: {e}");
                                    }
                                }
                            } else {
                                match self.db.import_from_catalog_db(&import_path) {
                                    Ok(added) => {
                                        self.status_bar_text = format!("Merged {added} clips from database");
                                        self.reload_catalog();
                                    }
                                    Err(e) => {
                                        self.status_bar_text = format!("Error importing database: {e}");
                                    }
                                }
                            }
                        }
                    }
                    TopBarAction::ExportCatalog => {
                        if let Some(export_path) = rfd::FileDialog::new()
                            .set_title("Export Catalog to JSON")
                            .set_file_name("mlv_catalog_export.json")
                            .add_filter("JSON Document", &["json"])
                            .save_file()
                        {
                            match self.db.export_catalog_json(&export_path) {
                                Ok(()) => {
                                    self.status_bar_text = format!("Exported catalog to {}", export_path.display());
                                }
                                Err(e) => {
                                    self.status_bar_text = format!("Error exporting catalog: {e}");
                                }
                            }
                        }
                    }
                    TopBarAction::BackupCatalog => {
                        let backup_dir = self.config.backup_folder.clone().unwrap_or_else(|| {
                            let def = self.db.path.parent().unwrap_or(Path::new(".")).join("backups");
                            let _ = std::fs::create_dir_all(&def);
                            def
                        });
                        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                        let orig_stem = self.db.path.file_stem().unwrap_or_default().to_string_lossy();
                        let backup_file = backup_dir.join(format!("{orig_stem}_backup_{timestamp}.db"));

                        match self.db.backup_database_to_file(&backup_file) {
                            Ok(_) => {
                                self.status_bar_text = format!("Created atomic backup: {}", backup_file.display());
                            }
                            Err(e) => {
                                self.status_bar_text = format!("Backup failed: {e}");
                            }
                        }
                    }
                    TopBarAction::IngestFromCard => {
                        self.ingest_dialog.open(&self.config, &self.db, ctx);
                    }
                    TopBarAction::ImportFiles => {
                        if let Some(files) = rfd::FileDialog::new()
                            .add_filter("Magic Lantern RAW Video", &["mlv", "MLV"])
                            .pick_files()
                        {
                            self.start_import(files, ctx);
                        }
                    }
                    TopBarAction::ImportFolder => {
                        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                            self.start_import(vec![folder], ctx);
                        }
                    }
                    TopBarAction::OpenSelectedInMlvApp => {
                        let targets = if !self.selected_indices.is_empty() {
                            self.selected_indices.clone()
                        } else if let Some(idx) = self.selected_video_idx {
                            vec![idx]
                        } else {
                            Vec::new()
                        };
                        if !targets.is_empty() {
                            self.open_selection_in_mlv_app(&targets);
                        }
                    }
                    TopBarAction::OpenCatalogInMlvApp => {
                        self.open_catalog_in_mlv_app();
                    }
                    TopBarAction::Exit => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }

                    // Edit
                    TopBarAction::ManageKeywords => {
                        self.keyword_dialog.open();
                    }
                    TopBarAction::CopyClipSettings => {
                        self.copy_clip_settings();
                    }
                    TopBarAction::PasteClipSettings => {
                        self.paste_clip_settings();
                    }
                    TopBarAction::SelectAll => {
                        self.selected_indices = (0..self.videos.len()).collect();
                        if self.selected_video_idx.is_none() && !self.videos.is_empty() {
                            self.selected_video_idx = Some(0);
                        }
                    }
                    TopBarAction::DeselectAll => {
                        self.selected_indices.clear();
                        self.selected_video_idx = None;
                    }
                    TopBarAction::RemoveFromCatalog => {
                        let targets = if !self.selected_indices.is_empty() {
                            self.selected_indices.clone()
                        } else if let Some(idx) = self.selected_video_idx {
                            vec![idx]
                        } else {
                            Vec::new()
                        };
                        if !targets.is_empty() {
                            self.prompt_delete(DeleteMode::RemoveFromCatalog, targets);
                        }
                    }
                    TopBarAction::MoveToTrash => {
                        let targets = if !self.selected_indices.is_empty() {
                            self.selected_indices.clone()
                        } else if let Some(idx) = self.selected_video_idx {
                            vec![idx]
                        } else {
                            Vec::new()
                        };
                        if !targets.is_empty() {
                            self.prompt_delete(DeleteMode::MoveToTrash, targets);
                        }
                    }

                    // View
                    TopBarAction::SwitchToGrid => {
                        self.navigate_to_grid(ctx);
                    }
                    TopBarAction::SwitchToPlayer => {
                        self.navigate_to_player(ctx);
                    }
                    TopBarAction::ToggleFullscreen => {
                        if self.view_mode == ViewMode::Player {
                            self.player_view.is_fullscreen = !self.player_view.is_fullscreen;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.player_view.is_fullscreen));
                        } else if self.selected_video_idx.is_some() {
                            self.view_mode = ViewMode::Player;
                            self.player_view.is_fullscreen = true;
                            self.player_view.show_overlay_controls = false;
                            self.ensure_playback(ctx);
                        }
                    }
                    TopBarAction::ZoomIn => {
                        self.grid_view.card_size = (self.grid_view.card_size + 30.0).min(480.0);
                    }
                    TopBarAction::ZoomOut => {
                        self.grid_view.card_size = (self.grid_view.card_size - 30.0).max(160.0);
                    }
                    TopBarAction::ZoomReset => {
                        self.grid_view.card_size = 260.0;
                    }

                    // Preferences
                    TopBarAction::SetTheme(mode) => {
                        self.config.theme_mode = mode;
                        self.config.save();
                        theme::apply_theme(ctx, mode);
                        ctx.request_repaint();
                    }
                    TopBarAction::OpenSettingsModal => {
                        self.settings_dialog.open(&self.config);
                    }

                    // Help
                    TopBarAction::OpenShortcutsModal => {
                        self.help_dialogs.show_shortcuts = true;
                    }
                    TopBarAction::CheckForUpdates => {
                        self.help_dialogs.show_updater = true;
                        self.update_manager.check_for_updates();
                        self.status_bar_text = "Checking for updates...".to_string();
                    }
                    TopBarAction::OpenAboutModal => {
                        self.help_dialogs.show_about = true;
                    }
                }
            }
        }

        // Import Progress Banner (shown below Top Bar when an import is actively running and not in fullscreen player)
        if !is_fullscreen_player {
            if let Some(prog) = &self.import_progress {
                egui::TopBottomPanel::top("import_progress_banner")
                    .exact_height(28.0)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            ui.spinner();
                            ui.label(RichText::new(format!("Importing ({}/{}):", prog.current, prog.total)).strong().size(FONT_SMALL).color(TEXT_PRIMARY));
                            ui.label(RichText::new(&prog.current_file).size(FONT_SMALL).color(ACCENT_HOVER));

                            let fraction = if prog.total > 0 { prog.current as f32 / prog.total as f32 } else { 0.0 };
                            ui.add(
                                egui::ProgressBar::new(fraction)
                                    .show_percentage()
                                    .animate(true)
                                    .desired_width(180.0),
                            );

                            ui.label(
                                RichText::new(format!("{} added  ·  {} duplicates skipped", prog.imported, prog.duplicates))
                                    .size(FONT_CAPTION)
                                    .color(TEXT_MUTED),
                            );

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let cancel_label = format!("{} Cancel", widgets::icon::X);
                                if ui.add(egui::Button::new(RichText::new(cancel_label).size(FONT_SMALL).color(FLAG_REJECTED))).clicked() {
                                    if let Some(flag) = &self.import_cancel_flag {
                                        flag.store(true, std::sync::atomic::Ordering::Relaxed);
                                    }
                                    self.status_bar_text = "Cancelling import...".to_string();
                                }
                            });
                        });
                    });
            }
        }

        // 2. Status Bar (Bottom)
        if !is_fullscreen_player {
            egui::TopBottomPanel::bottom("studio_status_bar")
                .exact_height(24.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        if let Some(prog) = &self.import_progress {
                            ui.spinner();
                            let fraction = if prog.total > 0 { prog.current as f32 / prog.total as f32 } else { 0.0 };
                            ui.add(
                                egui::ProgressBar::new(fraction)
                                    .animate(true)
                                    .desired_width(100.0),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "Importing {}/{} ({})  ·  {} skipped",
                                    prog.current, prog.total, prog.current_file, prog.duplicates
                                ))
                                .size(FONT_CAPTION)
                                .color(ACCENT_HOVER),
                            );
                        } else {
                            ui.label(RichText::new(&self.status_bar_text).size(FONT_CAPTION).color(TEXT_MUTED));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(4.0);
                            ui.label(RichText::new(&self.gpu_info).size(FONT_CAPTION).color(TEXT_MUTED));
                            ui.separator();
                            ui.label(RichText::new(format!("{} clips", self.videos.len())).size(FONT_CAPTION).color(TEXT_SECONDARY));
                        });
                    });
                });
        }

        // 3. Left Navigator Sidebar
        if !is_fullscreen_player {
            egui::SidePanel::left("studio_left_navigator")
                .default_width(220.0)
                .width_range(180.0..=320.0)
                .show(ctx, |ui| {
                    let mut open_kw_mgr = false;
                    let mut open_ingest = false;
                    if let Some(nav_act) = Navigator::show(ui, &mut self.filter, &self.facets, &self.db, &mut open_kw_mgr, &mut open_ingest) {
                        match nav_act {
                            NavigatorAction::FilterChanged => self.reload_catalog(),
                            NavigatorAction::OpenTagInMlvApp(tag) => self.open_tag_in_mlv_app(&tag),
                            NavigatorAction::OpenCatalogInMlvApp => self.open_catalog_in_mlv_app(),
                            NavigatorAction::OpenFlagInMlvApp(flag) => self.open_flag_in_mlv_app(flag),
                        }
                    }
                    if open_kw_mgr {
                        self.keyword_dialog.open();
                    }
                    if open_ingest {
                        self.ingest_dialog.open(&self.config, &self.db, ctx);
                    }
                });
        }

        // 4. Right Metadata Inspector Sidebar
        if !is_fullscreen_player {
            egui::SidePanel::right("studio_right_inspector")
                .default_width(280.0)
                .width_range(240.0..=380.0)
                .show(ctx, |ui| {
                    let sel_indices = if !self.selected_indices.is_empty() {
                        self.selected_indices.clone()
                    } else if let Some(idx) = self.selected_video_idx {
                        vec![idx]
                    } else {
                        Vec::new()
                    };
                    let selected_count = sel_indices.len();
                    let mut sel_vid = self.selected_video_idx.and_then(|idx| self.videos.get_mut(idx));
                    if let Some(act) = self.inspector.show(ui, sel_vid.as_deref_mut(), selected_count, &self.db, &self.facets) {
                        match act {
                            crate::ui::inspector::InspectorAction::OpenInMlvApp => {
                                self.open_selection_in_mlv_app(&sel_indices);
                            }
                            crate::ui::inspector::InspectorAction::BatchSetRating(rating) => {
                                for &idx in &sel_indices {
                                    if let Some(vid) = self.videos.get_mut(idx) {
                                        vid.rating = rating;
                                        let _ = self.db.set_rating(vid.id, rating);
                                    }
                                }
                                self.refresh_facets();
                                if self.filter.min_rating.is_some() {
                                    self.reload_catalog();
                                }
                            }
                            crate::ui::inspector::InspectorAction::BatchSetFlag(flag) => {
                                for &idx in &sel_indices {
                                    if let Some(vid) = self.videos.get_mut(idx) {
                                        vid.flag = flag;
                                        let _ = self.db.set_flag(vid.id, flag);
                                    }
                                }
                                self.refresh_facets();
                                if self.filter.flag.is_some() {
                                    self.reload_catalog();
                                }
                            }
                            crate::ui::inspector::InspectorAction::BatchAddTag(tag) => {
                                for &idx in &sel_indices {
                                    if let Some(vid) = self.videos.get_mut(idx) {
                                        if !vid.tags.contains(&tag) {
                                            vid.tags.push(tag.clone());
                                            let _ = self.db.add_tag(vid.id, &tag);
                                        }
                                    }
                                }
                                self.refresh_facets();
                                if self.filter.tag.is_some() {
                                    self.reload_catalog();
                                }
                            }
                            crate::ui::inspector::InspectorAction::BatchRemoveTag(tag) => {
                                for &idx in &sel_indices {
                                    if let Some(vid) = self.videos.get_mut(idx) {
                                        vid.tags.retain(|t| t != &tag);
                                        let _ = self.db.remove_tag(vid.id, &tag);
                                    }
                                }
                                self.refresh_facets();
                                if self.filter.tag.is_some() {
                                    self.reload_catalog();
                                }
                            }
                            crate::ui::inspector::InspectorAction::CreateKeywordWithParent { name, parent_id } => {
                                let _ = self.db.create_tag(&name, parent_id);
                                self.refresh_facets();
                            }
                            crate::ui::inspector::InspectorAction::OpenKeywordManager => {
                                self.keyword_dialog.open();
                            }
                            crate::ui::inspector::InspectorAction::FilterByTag(tag) => {
                                self.filter.tag = if self.filter.tag.as_ref() == Some(&tag) {
                                    None
                                } else {
                                    Some(tag)
                                };
                                self.reload_catalog();
                            }
                            crate::ui::inspector::InspectorAction::BatchSetNotes(notes) => {
                                for &idx in &sel_indices {
                                    if let Some(vid) = self.videos.get_mut(idx) {
                                        vid.notes = notes.clone();
                                        let _ = self.db.set_notes(vid.id, &notes);
                                    }
                                }
                            }
                            crate::ui::inspector::InspectorAction::BatchSetSlate { scene, shot, take, location, camera_unit } => {
                                for &idx in &sel_indices {
                                    if let Some(vid) = self.videos.get_mut(idx) {
                                        vid.scene = scene.clone();
                                        vid.shot = shot.clone();
                                        vid.take = take.clone();
                                        vid.location = location.clone();
                                        vid.camera_unit = camera_unit.clone();
                                        let _ = self.db.set_production_slate(
                                            vid.id,
                                            scene.as_deref(),
                                            shot.as_deref(),
                                            take.as_deref(),
                                            location.as_deref(),
                                            camera_unit.as_deref(),
                                        );
                                    }
                                }
                            }
                            crate::ui::inspector::InspectorAction::BatchSetStretch(stretch) => {
                                for &idx in &sel_indices {
                                    if let Some(vid) = self.videos.get_mut(idx) {
                                        vid.stretch_factor = stretch;
                                        let _ = self.db.set_stretch_factor(vid.id, stretch);
                                    }
                                }
                            }
                        }
                    }
                });
        }

        // 5. Central Workspace: Grid or Player View
        let central_frame = if is_fullscreen_player {
            egui::Frame::none().fill(egui::Color32::from_rgb(10, 10, 12))
        } else {
            egui::Frame::central_panel(&ctx.style())
        };
        egui::CentralPanel::default().frame(central_frame).show(ctx, |ui| {
            match self.view_mode {
                ViewMode::Grid => {
                    if TopBar::show_filter_bar(ui, &mut self.filter) {
                        self.reload_catalog();
                    }

                    if let Some(action) = self.grid_view.show(
                        ui,
                        &self.videos,
                        &self.selected_indices,
                        &mut self.thumb_mgr,
                        &mut self.filter.sort,
                        has_modal_open,
                    ) {
                        match action {
                            GridAction::Select { index, extend, toggle } => {
                                if extend {
                                    // Shift-click range selection
                                    let anchor = self.selected_video_idx.unwrap_or(index);
                                    let start = anchor.min(index);
                                    let end = anchor.max(index);
                                    self.selected_indices = (start..=end).collect();
                                    self.selected_video_idx = Some(index);
                                } else if toggle {
                                    // Ctrl/Cmd-click toggle selection
                                    if let Some(pos) = self.selected_indices.iter().position(|&i| i == index) {
                                        self.selected_indices.remove(pos);
                                        self.selected_video_idx = self.selected_indices.last().copied();
                                    } else {
                                        self.selected_indices.push(index);
                                        self.selected_video_idx = Some(index);
                                    }
                                } else {
                                    // Normal single click
                                    self.selected_indices = vec![index];
                                    self.selected_video_idx = Some(index);
                                }
                            }
                            GridAction::OpenPlayer(idx) => {
                                self.selected_video_idx = Some(idx);
                                self.selected_indices = vec![idx];
                                self.view_mode = ViewMode::Player;
                                self.ensure_playback(ctx);
                            }
                            GridAction::TogglePick(id) => {
                                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                                    let new_flag = if vid.flag == PickFlag::Picked { PickFlag::Unflagged } else { PickFlag::Picked };
                                    vid.flag = new_flag;
                                    let _ = self.db.set_flag(id, new_flag);
                                    let _ = self.reload_catalog();
                                }
                            }
                            GridAction::SetRating(id, rating) => {
                                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                                    vid.rating = rating;
                                    let _ = self.db.set_rating(id, rating);
                                }
                            }
                            GridAction::OpenInMlvApp(indices) => {
                                self.open_selection_in_mlv_app(&indices);
                            }
                            GridAction::RequestRemoveFromCatalog(indices) => {
                                self.prompt_delete(DeleteMode::RemoveFromCatalog, indices);
                            }
                            GridAction::RequestMoveToTrash(indices) => {
                                self.prompt_delete(DeleteMode::MoveToTrash, indices);
                            }
                            GridAction::BoxSelect(indices) => {
                                let modifiers = ui.input(|i| i.modifiers);
                                if modifiers.shift || modifiers.ctrl || modifiers.command {
                                    // Add to existing selection
                                    for idx in indices {
                                        if !self.selected_indices.contains(&idx) {
                                            self.selected_indices.push(idx);
                                        }
                                    }
                                } else {
                                    // Replace selection with box selection
                                    self.selected_indices = indices;
                                }
                                self.selected_video_idx = self.selected_indices.first().copied();
                            }
                            GridAction::DeselectAll => {
                                self.selected_indices.clear();
                                self.selected_video_idx = None;
                            }
                            GridAction::SortChanged => {
                                self.reload_catalog();
                            }
                        }
                    }
                }
                ViewMode::Player => {
                    let has_pb = self.ensure_playback(ctx);
                    if has_pb {
                        if let Some((_id, ref mut pb)) = self.current_playback {
                            if let Some(idx) = self.selected_video_idx {
                                if let Some(vid) = self.videos.get(idx) {
                                    if let Some(act) = self.player_view.show(ui, vid, pb) {
                                        match act {
                                            PlayerAction::BackToGrid => {
                                                self.navigate_to_grid(ctx);
                                            }
                                            PlayerAction::Seek(frame) => {
                                                pb.set_target(frame, self.player_view.is_looping);
                                                let time_sec = frame as f32 / vid.fps.max(1.0);
                                                if let Ok(mut a) = pb.audio.lock() {
                                                    a.seek(time_sec, self.player_view.is_playing);
                                                }
                                            }
                                            PlayerAction::SetRating(rating) => {
                                                let id = vid.id;
                                                let _ = self.db.set_rating(id, rating);
                                                self.reload_catalog();
                                            }
                                            PlayerAction::SetFlag(flag) => {
                                                let id = vid.id;
                                                let _ = self.db.set_flag(id, flag);
                                                self.reload_catalog();
                                            }
                                            PlayerAction::SetStretch(factor) => {
                                                let id = vid.id;
                                                let _ = self.db.set_stretch_factor(id, factor);
                                                self.reload_catalog();
                                            }
                                            PlayerAction::ToggleFullscreen => {
                                                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.player_view.is_fullscreen));
                                                ctx.request_repaint();
                                            }
                                            PlayerAction::OpenInMlvApp => {
                                                let path = PathBuf::from(&vid.file_path);
                                                self.open_in_mlv_app(&path);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        self.view_mode = ViewMode::Grid;
                    }
                }
            }

            if has_modal_open {
                ui.painter().rect_filled(ui.max_rect(), egui::Rounding::ZERO, egui::Color32::from_black_alpha(100));
            }
        });

        // 6. Settings & Catalog Manager Modal
        if let Some(act) = self.settings_dialog.show(ctx, &self.db, &mut self.config) {
            match act {
                SettingsAction::CatalogChanged => {
                    self.reload_catalog();
                }
                SettingsAction::SwitchCatalog(path) => {
                    self.switch_catalog(path, ctx);
                }
            }
        }

        // 7. Delete / Remove Confirmation Modal
        if let Some(res) = self.delete_confirm_dialog.show(ctx) {
            match res {
                DeleteConfirmResult::Confirmed { mode, target_indices } => {
                    match mode {
                        DeleteMode::RemoveFromCatalog => {
                            self.execute_remove_from_catalog(&target_indices);
                        }
                        DeleteMode::MoveToTrash => {
                            self.execute_move_to_trash(&target_indices);
                        }
                    }
                }
                DeleteConfirmResult::Cancelled => {}
            }
        }

        // 8. Keyword Manager Modal
        if let Some(KeywordManagerAction::Changed) = self.keyword_dialog.show(ctx, &self.db) {
            self.reload_catalog();
        }

        // 9. Help & Shortcuts Modals
        self.help_dialogs.show(ctx, &mut self.update_manager);

        // 10. SD / Card Ingestion Modal
        if let Some(IngestDialogAction::Finished(count)) = self.ingest_dialog.show(ctx, &self.db, &mut self.config) {
            if count > 0 {
                self.status_bar_text = format!("Ingested {} clip(s) from card.", count);
                self.reload_catalog();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use std::path::Path;

    #[test]
    fn test_navigation_hierarchy() {
        let ctx = egui::Context::default();
        let db = Database::open(Path::new(":memory:")).unwrap();
        let thumb_mgr = ThumbnailManager::new(PathBuf::from(":memory:"), ctx.clone());
        let (import_tx, import_rx) = crossbeam_channel::unbounded();

        let mut app = MlvExplorerApp {
            config: AppConfig::default(),
            db,
            view_mode: ViewMode::Grid,
            videos: Vec::new(),
            selected_video_idx: None,
            selected_indices: Vec::new(),
            filter: FilterCriteria::default(),
            facets: Facets::default(),
            all_tags: Vec::new(),
            cameras: Vec::new(),
            lenses: Vec::new(),
            settings_clipboard: None,
            thumb_mgr,
            current_playback: None,
            grid_view: GridView::default(),
            player_view: PlayerView::default(),
            inspector: Inspector::default(),
            settings_dialog: SettingsDialog::default(),
            delete_confirm_dialog: DeleteConfirmDialog::default(),
            keyword_dialog: KeywordManagerDialog::default(),
            ingest_dialog: IngestDialog::default(),
            help_dialogs: HelpDialogs::default(),
            update_manager: crate::updater::UpdateManager::default(),
            import_tx,
            import_rx,
            import_progress: None,
            import_cancel_flag: None,
            gpu_info: String::new(),
            status_bar_text: String::new(),
        };

        // Initially in Grid with no modals open, navigate_back returns false
        assert_eq!(app.view_mode, ViewMode::Grid);
        assert!(!app.navigate_back(&ctx));

        // When a modal is open, navigate_back closes it and stays on Grid
        app.keyword_dialog.is_open = true;
        assert!(app.navigate_back(&ctx));
        assert!(!app.keyword_dialog.is_open);
        assert_eq!(app.view_mode, ViewMode::Grid);

        // When in Player mode, navigate_back returns to Grid mode
        app.view_mode = ViewMode::Player;
        app.player_view.is_fullscreen = true;
        app.player_view.is_playing = true;
        assert!(app.navigate_back(&ctx));
        assert_eq!(app.view_mode, ViewMode::Grid);
        assert!(!app.player_view.is_fullscreen);
        assert!(!app.player_view.is_playing);

        // Forward navigation when video is selected
        app.selected_video_idx = Some(0);
        assert!(app.navigate_forward(&ctx));
        assert_eq!(app.view_mode, ViewMode::Player);

        // Back navigation returns to Grid
        assert!(app.navigate_back(&ctx));
        assert_eq!(app.view_mode, ViewMode::Grid);
    }
}
