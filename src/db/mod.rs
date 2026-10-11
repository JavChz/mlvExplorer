pub mod models;

pub use models::*;
use rusqlite::{params, Connection, Result};
use std::path::Path;

pub struct Database {
    conn: Connection,
    db_path: String,
}

impl Database {
    /// Initialize or open the SQLite catalog database
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db_path = path.as_ref().to_string_lossy().to_string();
        let conn = Connection::open(path)?;

        // Enable WAL mode for high concurrent read/write performance
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        let db = Self { conn, db_path };
        db.init_schema()?;
        Ok(db)
    }

    /// Initialize tables and indexes
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS videos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL UNIQUE,
                file_name TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                fps REAL NOT NULL,
                frame_count INTEGER NOT NULL,
                duration_seconds REAL NOT NULL,
                bits_per_pixel INTEGER NOT NULL,
                camera_name TEXT NOT NULL,
                camera_serial TEXT NOT NULL,
                lens_name TEXT NOT NULL,
                focal_length_mm INTEGER NOT NULL,
                aperture_fstop REAL NOT NULL,
                iso INTEGER NOT NULL,
                shutter_speed INTEGER NOT NULL,
                white_balance_kelvin INTEGER NOT NULL,
                date_recorded TEXT,
                imported_at TEXT NOT NULL,
                rating INTEGER NOT NULL DEFAULT 0,
                flag INTEGER NOT NULL DEFAULT 0,
                notes TEXT NOT NULL DEFAULT '',
                color_label TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                color TEXT NOT NULL DEFAULT '#5B8DEF'
            );

            CREATE TABLE IF NOT EXISTS video_tags (
                video_id INTEGER NOT NULL,
                tag_id INTEGER NOT NULL,
                PRIMARY KEY (video_id, tag_id),
                FOREIGN KEY (video_id) REFERENCES videos(id) ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_videos_camera ON videos(camera_name);
            CREATE INDEX IF NOT EXISTS idx_videos_lens ON videos(lens_name);
            CREATE INDEX IF NOT EXISTS idx_videos_iso ON videos(iso);
            CREATE INDEX IF NOT EXISTS idx_videos_rating ON videos(rating);
            CREATE INDEX IF NOT EXISTS idx_videos_flag ON videos(flag);
            CREATE INDEX IF NOT EXISTS idx_videos_date ON videos(date_recorded);
            "#,
        )?;
        Ok(())
    }

    /// Insert or update video metadata in catalog
    pub fn upsert_video(&self, meta: &crate::mlv::MlvMetadata) -> Result<i64> {
        let file_name = Path::new(&meta.file_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| meta.file_path.clone());
        let imported_at = chrono::Utc::now().to_rfc3339();

        self.conn.execute(
            r#"
            INSERT INTO videos (
                file_path, file_name, file_size, width, height, fps,
                frame_count, duration_seconds, bits_per_pixel, camera_name,
                camera_serial, lens_name, focal_length_mm, aperture_fstop,
                iso, shutter_speed, white_balance_kelvin, date_recorded,
                imported_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
            ON CONFLICT(file_path) DO UPDATE SET
                file_size = excluded.file_size,
                width = excluded.width,
                height = excluded.height,
                fps = excluded.fps,
                frame_count = excluded.frame_count,
                duration_seconds = excluded.duration_seconds,
                bits_per_pixel = excluded.bits_per_pixel,
                camera_name = excluded.camera_name,
                camera_serial = excluded.camera_serial,
                lens_name = excluded.lens_name,
                focal_length_mm = excluded.focal_length_mm,
                aperture_fstop = excluded.aperture_fstop,
                iso = excluded.iso,
                shutter_speed = excluded.shutter_speed,
                white_balance_kelvin = excluded.white_balance_kelvin,
                date_recorded = excluded.date_recorded
            "#,
            params![
                meta.file_path,
                file_name,
                meta.file_size as i64,
                meta.width,
                meta.height,
                meta.fps(),
                meta.frame_count,
                meta.duration_seconds(),
                meta.bits_per_pixel,
                meta.camera_name,
                meta.camera_serial,
                meta.lens_name,
                meta.focal_length_mm,
                meta.aperture_fstop,
                meta.iso,
                meta.shutter_speed_microseconds,
                meta.white_balance_kelvin,
                meta.date_recorded,
                imported_at
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Query videos with flexible filters
    pub fn get_videos(&self, filter: &FilterCriteria) -> Result<Vec<VideoRecord>> {
        let mut query = String::from(
            r#"
            SELECT v.id, v.file_path, v.file_name, v.file_size, v.width, v.height,
                   v.fps, v.frame_count, v.duration_seconds, v.bits_per_pixel,
                   v.camera_name, v.camera_serial, v.lens_name, v.focal_length_mm,
                   v.aperture_fstop, v.iso, v.shutter_speed, v.white_balance_kelvin,
                   v.date_recorded, v.imported_at, v.rating, v.flag, v.notes, v.color_label,
                   GROUP_CONCAT(t.name) as tags
            FROM videos v
            LEFT JOIN video_tags vt ON v.id = vt.video_id
            LEFT JOIN tags t ON vt.tag_id = t.id
            WHERE 1=1
            "#,
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if !filter.search_text.trim().is_empty() {
            query.push_str(" AND (v.file_name LIKE ? OR v.camera_name LIKE ? OR v.lens_name LIKE ? OR v.notes LIKE ?)");
            let pattern = format!("%{}%", filter.search_text.trim());
            params.push(Box::new(pattern.clone()));
            params.push(Box::new(pattern.clone()));
            params.push(Box::new(pattern.clone()));
            params.push(Box::new(pattern));
        }

        if let Some(ref cam) = filter.camera {
            if !cam.is_empty() {
                query.push_str(" AND v.camera_name = ?");
                params.push(Box::new(cam.clone()));
            }
        }

        if let Some(ref lens) = filter.lens {
            if !lens.is_empty() {
                query.push_str(" AND v.lens_name = ?");
                params.push(Box::new(lens.clone()));
            }
        }

        if let Some(min_iso) = filter.min_iso {
            query.push_str(" AND v.iso >= ?");
            params.push(Box::new(min_iso));
        }

        if let Some(max_iso) = filter.max_iso {
            query.push_str(" AND v.iso <= ?");
            params.push(Box::new(max_iso));
        }

        if let Some(min_rating) = filter.min_rating {
            if min_rating > 0 {
                query.push_str(" AND v.rating >= ?");
                params.push(Box::new(min_rating as i64));
            }
        }

        if let Some(flag) = filter.flag {
            query.push_str(" AND v.flag = ?");
            params.push(Box::new(flag as i64));
        }

        query.push_str(" GROUP BY v.id ORDER BY v.date_recorded DESC, v.id DESC");

        let mut stmt = self.conn.prepare(&query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let tags_str: Option<String> = row.get(24)?;
            let tags = tags_str
                .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
                .unwrap_or_default();

            Ok(VideoRecord {
                id: row.get(0)?,
                file_path: row.get(1)?,
                file_name: row.get(2)?,
                file_size: row.get::<_, i64>(3)? as u64,
                width: row.get(4)?,
                height: row.get(5)?,
                fps: row.get(6)?,
                frame_count: row.get(7)?,
                duration_seconds: row.get(8)?,
                bits_per_pixel: row.get(9)?,
                camera_name: row.get(10)?,
                camera_serial: row.get(11)?,
                lens_name: row.get(12)?,
                focal_length_mm: row.get(13)?,
                aperture_fstop: row.get(14)?,
                iso: row.get(15)?,
                shutter_speed: row.get(16)?,
                white_balance_kelvin: row.get(17)?,
                date_recorded: row.get(18)?,
                imported_at: row.get(19)?,
                rating: row.get(20)?,
                flag: PickFlag::from(row.get::<_, i64>(21)?),
                notes: row.get(22)?,
                color_label: row.get(23)?,
                tags,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Update rating for video
    pub fn set_rating(&self, video_id: i64, rating: u8) -> Result<()> {
        self.conn.execute(
            "UPDATE videos SET rating = ?1 WHERE id = ?2",
            params![rating as i64, video_id],
        )?;
        Ok(())
    }

    /// Update pick flag for video
    pub fn set_flag(&self, video_id: i64, flag: PickFlag) -> Result<()> {
        self.conn.execute(
            "UPDATE videos SET flag = ?1 WHERE id = ?2",
            params![flag as i64, video_id],
        )?;
        Ok(())
    }

    /// Update notes
    pub fn set_notes(&self, video_id: i64, notes: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE videos SET notes = ?1 WHERE id = ?2",
            params![notes, video_id],
        )?;
        Ok(())
    }

    /// Add tag to video
    pub fn add_tag(&self, video_id: i64, tag_name: &str) -> Result<()> {
        let tag_name = tag_name.trim();
        if tag_name.is_empty() {
            return Ok(());
        }

        self.conn.execute(
            "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
            params![tag_name],
        )?;

        let tag_id: i64 = self.conn.query_row(
            "SELECT id FROM tags WHERE name = ?1",
            params![tag_name],
            |row| row.get(0),
        )?;

        self.conn.execute(
            "INSERT OR IGNORE INTO video_tags (video_id, tag_id) VALUES (?1, ?2)",
            params![video_id, tag_id],
        )?;

        Ok(())
    }

    /// Remove tag from video
    pub fn remove_tag(&self, video_id: i64, tag_name: &str) -> Result<()> {
        self.conn.execute(
            r#"
            DELETE FROM video_tags 
            WHERE video_id = ?1 
              AND tag_id = (SELECT id FROM tags WHERE name = ?2)
            "#,
            params![video_id, tag_name],
        )?;
        Ok(())
    }

    /// Get all unique cameras in the database
    pub fn get_cameras(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT DISTINCT camera_name FROM videos WHERE camera_name != '' ORDER BY camera_name")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    /// Get all unique lenses in the database
    pub fn get_lenses(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT DISTINCT lens_name FROM videos WHERE lens_name != '' ORDER BY lens_name")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    /// Get all tags
    pub fn get_all_tags(&self) -> Result<Vec<TagRecord>> {
        let mut stmt = self.conn.prepare("SELECT id, name, color FROM tags ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok(TagRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
            })
        })?;
        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    /// SQLite Online Backup: atomically back up the database to a destination file
    pub fn backup_database_to_file<P: AsRef<Path>>(&self, dest_path: P) -> Result<()> {
        let mut dest_conn = Connection::open(dest_path)?;
        let backup = rusqlite::backup::Backup::new(&self.conn, &mut dest_conn)?;
        backup.run_to_completion(5, std::time::Duration::from_millis(100), None)?;
        Ok(())
    }

    /// Export entire catalog (metadata, ratings, tags, notes) to a JSON file
    pub fn export_catalog_json<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let videos = self.get_videos(&FilterCriteria::default()).map_err(|e| e.to_string())?;
        let tags = self.get_all_tags().map_err(|e| e.to_string())?;

        let backup = CatalogBackup {
            exported_at: chrono::Utc::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            videos,
            tags,
        };

        let json = serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Import catalog JSON into current database (merging or updating)
    pub fn import_catalog_json<P: AsRef<Path>>(&self, path: P) -> Result<usize, String> {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let backup: CatalogBackup = serde_json::from_str(&content).map_err(|e| e.to_string())?;

        let mut count = 0;
        for vid in &backup.videos {
            let meta = crate::mlv::MlvMetadata {
                file_path: vid.file_path.clone(),
                file_size: vid.file_size,
                width: vid.width,
                height: vid.height,
                raw_width: vid.width,
                raw_height: vid.height,
                frame_count: vid.frame_count,
                bits_per_pixel: vid.bits_per_pixel,
                camera_name: vid.camera_name.clone(),
                camera_serial: vid.camera_serial.clone(),
                lens_name: vid.lens_name.clone(),
                focal_length_mm: vid.focal_length_mm,
                aperture_fstop: vid.aperture_fstop,
                iso: vid.iso,
                shutter_speed_microseconds: vid.shutter_speed,
                white_balance_kelvin: vid.white_balance_kelvin,
                date_recorded: vid.date_recorded.clone(),
                ..Default::default()
            };

            let row_id = self.upsert_video(&meta).map_err(|e| e.to_string())?;
            let _ = self.set_rating(row_id, vid.rating);
            let _ = self.set_flag(row_id, vid.flag);
            let _ = self.set_notes(row_id, &vid.notes);

            for t in &vid.tags {
                let _ = self.add_tag(row_id, t);
            }
            count += 1;
        }

        Ok(count)
    }
}
