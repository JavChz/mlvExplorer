use rusqlite::{params, Connection, Result};

pub const SCHEMA_VERSION: i64 = 4;

/// Initialize database tables and run additive migrations
pub fn init(conn: &Connection) -> Result<bool> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS videos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL UNIQUE,
            file_name TEXT NOT NULL,
            file_size INTEGER NOT NULL DEFAULT 0,
            width INTEGER NOT NULL DEFAULT 0,
            height INTEGER NOT NULL DEFAULT 0,
            fps REAL NOT NULL DEFAULT 0,
            frame_count INTEGER NOT NULL DEFAULT 0,
            duration_seconds REAL NOT NULL DEFAULT 0,
            bits_per_pixel INTEGER NOT NULL DEFAULT 0,
            camera_name TEXT NOT NULL DEFAULT '',
            camera_serial TEXT NOT NULL DEFAULT '',
            lens_name TEXT NOT NULL DEFAULT '',
            focal_length_mm INTEGER NOT NULL DEFAULT 0,
            aperture_fstop REAL NOT NULL DEFAULT 0,
            iso INTEGER NOT NULL DEFAULT 0,
            shutter_speed INTEGER NOT NULL DEFAULT 0,
            white_balance_kelvin INTEGER NOT NULL DEFAULT 0,
            date_recorded TEXT,
            imported_at TEXT NOT NULL DEFAULT '',
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
            PRIMARY KEY (video_id, tag_id)
        );
        CREATE TABLE IF NOT EXISTS thumbs (
            video_id INTEGER PRIMARY KEY,
            jpeg BLOB NOT NULL
        );
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_videos_camera ON videos(camera_name);
        CREATE INDEX IF NOT EXISTS idx_videos_lens ON videos(lens_name);
        CREATE INDEX IF NOT EXISTS idx_videos_rating ON videos(rating);
        CREATE INDEX IF NOT EXISTS idx_videos_flag ON videos(flag);
        CREATE INDEX IF NOT EXISTS idx_videos_date ON videos(date_recorded);
        "#,
    )?;

    // Additive migrations (ignore "duplicate column" errors)
    for sql in [
        "ALTER TABLE videos ADD COLUMN codec TEXT NOT NULL DEFAULT ''",
        "ALTER TABLE videos ADD COLUMN has_audio INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE videos ADD COLUMN chunk_count INTEGER NOT NULL DEFAULT 1",
        "ALTER TABLE videos ADD COLUMN stretch_factor REAL NOT NULL DEFAULT 1.0",
    ] {
        let _ = conn.execute(sql, []);
    }

    // Auto-correct any previously inverted stretch factors (e.g. 0.3333 -> 3.0)
    let _ = conn.execute(
        "UPDATE videos SET stretch_factor = 3.0 WHERE stretch_factor > 0.05 AND stretch_factor < 0.8",
        [],
    );

    let mut needs_rescan = false;
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version < SCHEMA_VERSION {
        let has_rows: i64 = conn.query_row("SELECT COUNT(*) FROM videos", [], |r| r.get(0))?;
        needs_rescan = has_rows > 0;
        conn.execute("DELETE FROM thumbs", [])?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }

    Ok(needs_rescan)
}
