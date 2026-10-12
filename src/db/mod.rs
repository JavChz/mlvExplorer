pub mod models;
pub mod queries;
pub mod schema;

pub use models::*;
pub use queries::Queries;

use rusqlite::{params, Connection, OptionalExtension, Result};
use std::path::{Path, PathBuf};

pub struct Database {
    conn: Connection,
    pub path: PathBuf,
    /// Set when an older catalog was migrated and metadata must be re-read.
    pub needs_rescan: bool,
}

/// Default catalog location: %APPDATA%/MLV Explorer/catalog.db (or ~/.local/share/...)
pub fn default_catalog_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_DATA_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("MLV Explorer");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("catalog.db")
}

pub fn open_connection(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let conn = open_connection(&path)?;
        let needs_rescan = schema::init(&conn)?;
        Ok(Self {
            conn,
            path,
            needs_rescan,
        })
    }

    #[inline]
    fn queries(&self) -> Queries<'_> {
        Queries::new(&self.conn)
    }

    #[inline]
    pub fn upsert_video(&self, meta: &crate::mlv::MlvMetadata) -> Result<i64> {
        self.queries().upsert_video(meta)
    }

    #[inline]
    pub fn is_file_imported<P: AsRef<Path>>(&self, path: P) -> bool {
        self.queries().is_file_imported(path)
    }

    #[inline]
    pub fn is_duplicate_clip(&self, meta: &crate::mlv::MlvMetadata) -> bool {
        self.queries().is_duplicate_clip(meta)
    }

    #[inline]
    pub fn insert_new_video(&self, meta: &crate::mlv::MlvMetadata) -> Result<Option<i64>> {
        self.queries().insert_new_video(meta)
    }

    #[inline]
    pub fn get_videos(&self, filter: &FilterCriteria) -> Result<Vec<VideoRecord>> {
        self.queries().get_videos(filter)
    }

    #[inline]
    pub fn set_stretch_factor(&self, video_id: i64, stretch: f32) -> Result<()> {
        self.queries().set_stretch_factor(video_id, stretch)
    }

    #[inline]
    pub fn all_paths(&self) -> Result<Vec<String>> {
        self.queries().all_paths()
    }

    #[inline]
    pub fn facets(&self) -> Result<Facets> {
        self.queries().facets()
    }

    #[inline]
    pub fn set_rating(&self, video_id: i64, rating: u8) -> Result<()> {
        self.queries().set_rating(video_id, rating)
    }

    #[inline]
    pub fn set_flag(&self, video_id: i64, flag: PickFlag) -> Result<()> {
        self.queries().set_flag(video_id, flag)
    }

    #[inline]
    pub fn set_notes(&self, video_id: i64, notes: &str) -> Result<()> {
        self.queries().set_notes(video_id, notes)
    }

    #[inline]
    pub fn set_production_slate(
        &self,
        video_id: i64,
        scene: Option<&str>,
        shot: Option<&str>,
        take: Option<&str>,
        location: Option<&str>,
        camera_unit: Option<&str>,
    ) -> Result<()> {
        self.queries().set_production_slate(video_id, scene, shot, take, location, camera_unit)
    }

    #[inline]
    pub fn add_tag(&self, video_id: i64, tag_name: &str) -> Result<()> {
        self.queries().add_tag(video_id, tag_name)
    }

    #[inline]
    pub fn remove_tag(&self, video_id: i64, tag_name: &str) -> Result<()> {
        self.queries().remove_tag(video_id, tag_name)
    }

    #[inline]
    pub fn all_tag_names(&self) -> Result<Vec<String>> {
        self.queries().all_tag_names()
    }

    #[inline]
    pub fn remove_video(&self, video_id: i64) -> Result<()> {
        self.queries().remove_video(video_id)
    }

    #[inline]
    pub fn remove_videos(&self, video_ids: &[i64]) -> Result<usize> {
        self.queries().remove_videos(video_ids)
    }

    #[inline]
    pub fn get_all_tags(&self) -> Result<Vec<TagRecord>> {
        self.queries().get_all_tags()
    }

    #[inline]
    pub fn backup_database_to_file<P: AsRef<Path>>(&self, dest_path: P) -> Result<()> {
        self.queries().backup_database_to_file(dest_path)
    }

    #[inline]
    pub fn export_catalog_json<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        self.queries().export_catalog_json(path)
    }

    #[inline]
    pub fn import_catalog_json<P: AsRef<Path>>(&self, path: P) -> Result<usize, String> {
        self.queries().import_catalog_json(path)
    }

    #[inline]
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        self.queries().get_setting(key)
    }

    #[inline]
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.queries().set_setting(key, value)
    }
}

// ---- thumbnail storage (used from worker threads with their own connection) ----

pub fn load_thumb(conn: &Connection, video_id: i64) -> Option<Vec<u8>> {
    conn.query_row("SELECT jpeg FROM thumbs WHERE video_id = ?1", params![video_id], |r| r.get(0))
        .optional()
        .ok()
        .flatten()
}

pub fn store_thumb(conn: &Connection, video_id: i64, jpeg: &[u8]) {
    let _ = conn.execute(
        "INSERT OR REPLACE INTO thumbs (video_id, jpeg) VALUES (?1, ?2)",
        params![video_id, jpeg],
    );
}

pub fn delete_thumb(conn: &Connection, video_id: i64) {
    let _ = conn.execute("DELETE FROM thumbs WHERE video_id = ?1", params![video_id]);
}
