//! Persistent application configuration stored in %APPDATA%/MLV Explorer/config.json
//! (or ~/.local/share/MLV Explorer/config.json on Linux/macOS).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    Dark,
    Light,
    System,
}

impl Default for ThemeMode {
    fn default() -> Self {
        Self::Dark
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Path to the last opened catalog SQLite database (.db).
    pub last_catalog_path: Option<PathBuf>,
    /// Default directory where catalog backups are written.
    pub backup_folder: Option<PathBuf>,
    /// Custom path to the MLV App executable.
    pub mlv_app_path: Option<PathBuf>,
    /// Last destination directory used for SD / Card ingest.
    #[serde(default)]
    pub last_ingest_destination: Option<PathBuf>,
    /// Selected UI theme mode (Dark, Light, System)
    #[serde(default)]
    pub theme_mode: ThemeMode,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            last_catalog_path: None,
            backup_folder: None,
            mlv_app_path: None,
            last_ingest_destination: None,
            theme_mode: ThemeMode::Dark,
        }
    }
}

/// Directory holding app data: %APPDATA%/MLV Explorer
pub fn app_data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_DATA_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("MLV Explorer");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn config_file_path() -> PathBuf {
    app_data_dir().join("config.json")
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_file_path();
        if path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
                    return cfg;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = config_file_path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    /// Resolves the catalog path to open on startup:
    /// returns last_catalog_path if valid, otherwise falls back to default_catalog_path.
    pub fn resolve_catalog_path(&self) -> PathBuf {
        if let Some(ref p) = self.last_catalog_path {
            if p.is_file() {
                return p.clone();
            }
        }
        crate::db::default_catalog_path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serde() {
        let mut cfg = AppConfig::default();
        cfg.last_catalog_path = Some(PathBuf::from("/path/to/my_catalog.db"));
        cfg.backup_folder = Some(PathBuf::from("/path/to/backups"));
        cfg.mlv_app_path = Some(PathBuf::from("/usr/bin/mlvapp"));

        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let loaded: AppConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.last_catalog_path, cfg.last_catalog_path);
        assert_eq!(loaded.backup_folder, cfg.backup_folder);
        assert_eq!(loaded.mlv_app_path, cfg.mlv_app_path);
    }
}

