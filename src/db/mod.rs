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
    pub fn create_tag(&self, name: &str, parent_id: Option<i64>) -> Result<i64> {
        self.queries().create_tag(name, parent_id)
    }

    #[inline]
    pub fn set_tag_parent(&self, tag_id: i64, parent_id: Option<i64>) -> Result<()> {
        self.queries().set_tag_parent(tag_id, parent_id)
    }

    #[inline]
    pub fn rename_tag(&self, old_name: &str, new_name: &str) -> Result<()> {
        self.queries().rename_tag(old_name, new_name)
    }

    #[inline]
    pub fn delete_keyword(&self, tag_name: &str) -> Result<()> {
        self.queries().delete_keyword(tag_name)
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
    pub fn get_tag_tree(&self) -> Result<Vec<TagNode>> {
        self.queries().get_tag_tree()
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
    pub fn backup_to_folder<P: AsRef<Path>>(&self, folder: P) -> Result<PathBuf, String> {
        self.queries().backup_to_folder(folder)
    }

    #[inline]
    pub fn import_from_catalog_db<P: AsRef<Path>>(&self, source_path: P) -> Result<usize, String> {
        self.queries().import_from_catalog_db(source_path)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mlv::MlvMetadata;

    #[test]
    fn test_backup_and_import() {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_dir = std::env::temp_dir().join(format!("mlv_explorer_test_{unique_id}"));
        std::fs::create_dir_all(&test_dir).unwrap();

        let db_path_1 = test_dir.join("cat1.db");
        let db1 = Database::open(&db_path_1).unwrap();

        let mut meta = MlvMetadata::default();
        meta.file_path = "C:/test/M01_0001.MLV".to_string();
        meta.camera_name = "Canon EOS 5D Mark III".to_string();
        meta.lens_name = "EF 50mm f/1.2L USM".to_string();
        meta.width = 1920;
        meta.height = 1080;
        meta.fps_num = 24000;
        meta.fps_den = 1000;
        meta.frame_count = 100;

        let row_id = db1.upsert_video(&meta).unwrap();
        db1.set_rating(row_id, 4).unwrap();
        db1.add_tag(row_id, "B-Roll").unwrap();
        db1.set_notes(row_id, "Sunny morning test").unwrap();

        // 1. Test backup creation
        let backup_dir = test_dir.join("backups");
        std::fs::create_dir_all(&backup_dir).unwrap();
        let backup_file = db1.backup_to_folder(&backup_dir).unwrap();
        assert!(backup_file.exists(), "Backup file should be created");

        // 2. Test opening a second catalog and importing from cat1
        let db_path_2 = test_dir.join("cat2.db");
        let db2 = Database::open(&db_path_2).unwrap();
        let imported_count = db2.import_from_catalog_db(&db_path_1).unwrap();
        assert_eq!(imported_count, 1);

        let videos = db2.get_videos(&FilterCriteria::default()).unwrap();
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].rating, 4);
        assert_eq!(videos[0].notes, "Sunny morning test");
        assert!(videos[0].tags.contains(&"B-Roll".to_string()));

        // Clean up
        let _ = std::fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_keyword_hierarchy_and_filtering() {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_dir = std::env::temp_dir().join(format!("mlv_tag_test_{unique_id}"));
        std::fs::create_dir_all(&test_dir).unwrap();

        let db_path = test_dir.join("test_tags.db");
        let db = Database::open(&db_path).unwrap();

        // Create hierarchy: Location -> Europe -> France
        let loc_id = db.create_tag("Location", None).unwrap();
        let eur_id = db.create_tag("Europe", Some(loc_id)).unwrap();
        let _fra_id = db.create_tag("France", Some(eur_id)).unwrap();

        let mut meta = MlvMetadata::default();
        meta.file_path = "C:/test/clip_france.MLV".to_string();
        meta.width = 1920;
        meta.height = 1080;
        meta.fps_num = 24000;
        meta.fps_den = 1000;
        meta.frame_count = 50;

        let row_id = db.upsert_video(&meta).unwrap();
        // Assign the leaf child tag "France" to the video
        db.add_tag(row_id, "France").unwrap();

        // 1. Verify tree structure
        let tree = db.get_tag_tree().unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "Location");
        assert_eq!(tree[0].total_count, 1); // Aggregated from grandchild
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].name, "Europe");
        assert_eq!(tree[0].children[0].total_count, 1);
        assert_eq!(tree[0].children[0].children[0].name, "France");
        assert_eq!(tree[0].children[0].children[0].direct_count, 1);

        // 2. Verify filtering by Ancestor ("Location") finds the video tagged with "France"
        let mut filter = FilterCriteria::default();
        filter.tag = Some("Location".to_string());
        let res_ancestor = db.get_videos(&filter).unwrap();
        assert_eq!(res_ancestor.len(), 1, "Filtering by Location should match clips tagged with France");

        // 3. Verify filtering by intermediate Parent ("Europe") finds the video
        filter.tag = Some("Europe".to_string());
        let res_parent = db.get_videos(&filter).unwrap();
        assert_eq!(res_parent.len(), 1, "Filtering by Europe should match clips tagged with France");

        // 4. Verify filtering by direct Tag ("France")
        filter.tag = Some("France".to_string());
        let res_leaf = db.get_videos(&filter).unwrap();
        assert_eq!(res_leaf.len(), 1);

        // 5. Test set_tag_parent under root tag (loc_id has NULL parent_id)
        let paris_id = db.create_tag("Paris", None).unwrap();
        db.set_tag_parent(paris_id, Some(loc_id)).unwrap();

        let tags = db.get_all_tags().unwrap();
        let paris_tag = tags.iter().find(|t| t.id == paris_id).unwrap();
        assert_eq!(paris_tag.parent_id, Some(loc_id));

        // 6. Test promoting back to root
        db.set_tag_parent(paris_id, None).unwrap();
        let tags = db.get_all_tags().unwrap();
        let paris_tag = tags.iter().find(|t| t.id == paris_id).unwrap();
        assert_eq!(paris_tag.parent_id, None);

        // 7. Test cycle prevention (Location cannot have Paris as parent if Paris is under Location)
        db.set_tag_parent(paris_id, Some(loc_id)).unwrap();
        db.set_tag_parent(loc_id, Some(paris_id)).unwrap();
        let tags = db.get_all_tags().unwrap();
        let loc_tag = tags.iter().find(|t| t.id == loc_id).unwrap();
        assert_eq!(loc_tag.parent_id, None);

        // Clean up
        let _ = std::fs::remove_dir_all(&test_dir);
    }
}

