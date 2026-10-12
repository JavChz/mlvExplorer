use crate::db::Database;
use std::path::{Path, PathBuf};

/// Handles discovering and launching external applications (e.g. MLV App)
pub struct ExternalLauncher;

impl ExternalLauncher {
    /// Launches MLV App with the given clip file path.
    /// Returns a status message string describing the outcome.
    pub fn open_in_mlv_app(
        db: &Database,
        configured_path: Option<&PathBuf>,
        file_path: &Path,
    ) -> Result<String, String> {
        let abs_path = match std::fs::canonicalize(file_path) {
            Ok(p) => p,
            Err(_) => file_path.to_path_buf(),
        };

        // 1. Check if configured path is provided, or user already saved one in database
        let mut exe_path: Option<PathBuf> = configured_path
            .cloned()
            .filter(|p| p.is_file())
            .or_else(|| {
                db.get_setting("mlv_app_path")
                    .ok()
                    .flatten()
                    .map(PathBuf::from)
                    .filter(|p| p.is_file())
            });

        // 2. If not saved or file was moved/deleted, search standard installation paths & PATH
        if exe_path.is_none() {
            let mut candidates: Vec<PathBuf> = Vec::new();

            // Check standard program directories on Windows
            if let Ok(prog_files) = std::env::var("ProgramFiles") {
                candidates.push(PathBuf::from(&prog_files).join("MLV App").join("MLV App.exe"));
                candidates.push(PathBuf::from(&prog_files).join("MLVApp").join("MLV App.exe"));
                candidates.push(PathBuf::from(&prog_files).join("MLVApp").join("mlvapp.exe"));
            }
            if let Ok(prog_files_x86) = std::env::var("ProgramFiles(x86)") {
                candidates.push(PathBuf::from(&prog_files_x86).join("MLV App").join("MLV App.exe"));
                candidates.push(PathBuf::from(&prog_files_x86).join("MLVApp").join("mlvapp.exe"));
            }
            if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
                candidates.push(PathBuf::from(&local_appdata).join("Programs").join("MLV App").join("MLV App.exe"));
                candidates.push(PathBuf::from(&local_appdata).join("MLV App").join("MLV App.exe"));
            }

            // Also check Desktop, Downloads, and common user folders where portable builds are extracted
            if let Ok(user_profile) = std::env::var("USERPROFILE") {
                let up = PathBuf::from(user_profile);
                candidates.push(up.join("Downloads").join("MLV App.exe"));
                candidates.push(up.join("Desktop").join("MLV App.exe"));
            }

            // Check system PATH
            if let Ok(path_var) = std::env::var("PATH") {
                for dir in std::env::split_paths(&path_var) {
                    let c1 = dir.join("MLV App.exe");
                    let c2 = dir.join("mlvapp.exe");
                    let c3 = dir.join("mlv_app.exe");
                    if c1.is_file() {
                        candidates.push(c1);
                        break;
                    }
                    if c2.is_file() {
                        candidates.push(c2);
                        break;
                    }
                    if c3.is_file() {
                        candidates.push(c3);
                        break;
                    }
                }
            }

            for cand in candidates {
                if cand.is_file() {
                    exe_path = Some(cand);
                    break;
                }
            }
        }

        // 3. If still not found, open a file picker to prompt user to locate MLV App
        if exe_path.is_none() {
            let filter_ext = if cfg!(target_os = "windows") {
                &["exe"][..]
            } else if cfg!(target_os = "macos") {
                &["app"][..]
            } else {
                &["AppImage", "bin", "*"][..]
            };

            if let Some(picked) = rfd::FileDialog::new()
                .set_title("Locate MLV App executable (MLV App.exe)")
                .add_filter("MLV App Executable", filter_ext)
                .pick_file()
            {
                if picked.is_file() {
                    let _ = db.set_setting("mlv_app_path", &picked.to_string_lossy());
                    exe_path = Some(picked);
                }
            }
        }

        // 4. Launch MLV App with the clip path as argument
        if let Some(exe) = exe_path {
            let _ = db.set_setting("mlv_app_path", &exe.to_string_lossy());
            match std::process::Command::new(&exe).arg(&abs_path).spawn() {
                Ok(_) => Ok(format!("Launched MLV App with: {}", abs_path.display())),
                Err(e) => Err(format!("Failed to launch MLV App ({}): {}", exe.display(), e)),
            }
        } else {
            Err("MLV App executable not configured or not found.".to_string())
        }
    }

    /// Searches standard paths to locate an MLV App installation executable.
    pub fn detect_mlv_app() -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();

        if let Ok(prog_files) = std::env::var("ProgramFiles") {
            candidates.push(PathBuf::from(&prog_files).join("MLV App").join("MLV App.exe"));
            candidates.push(PathBuf::from(&prog_files).join("MLVApp").join("MLV App.exe"));
            candidates.push(PathBuf::from(&prog_files).join("MLVApp").join("mlvapp.exe"));
        }
        if let Ok(prog_files_x86) = std::env::var("ProgramFiles(x86)") {
            candidates.push(PathBuf::from(&prog_files_x86).join("MLV App").join("MLV App.exe"));
            candidates.push(PathBuf::from(&prog_files_x86).join("MLVApp").join("mlvapp.exe"));
        }
        if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
            candidates.push(PathBuf::from(&local_appdata).join("Programs").join("MLV App").join("MLV App.exe"));
            candidates.push(PathBuf::from(&local_appdata).join("MLV App").join("MLV App.exe"));
        }
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let up = PathBuf::from(user_profile);
            candidates.push(up.join("Downloads").join("MLV App.exe"));
            candidates.push(up.join("Desktop").join("MLV App.exe"));
        }
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let c1 = dir.join("MLV App.exe");
                let c2 = dir.join("mlvapp.exe");
                let c3 = dir.join("mlv_app.exe");
                if c1.is_file() { candidates.push(c1); break; }
                if c2.is_file() { candidates.push(c2); break; }
                if c3.is_file() { candidates.push(c3); break; }
            }
        }

        candidates.into_iter().find(|p| p.is_file())
    }
}
