use crossbeam_channel::Receiver;
use serde::Deserialize;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_REPO: &str = "JavChz/mlvExplorer";

#[derive(Debug, Clone, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub html_url: String,
    pub body: Option<String>,
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate {
        current_version: String,
    },
    UpdateAvailable {
        current_version: String,
        latest_version: String,
        release_title: String,
        release_notes: String,
        release_url: String,
        target_asset_name: Option<String>,
        target_download_url: Option<String>,
        target_asset_size: Option<u64>,
    },
    Downloading {
        latest_version: String,
        asset_name: String,
        bytes_downloaded: u64,
        total_bytes: Option<u64>,
    },
    Downloaded {
        latest_version: String,
        asset_name: String,
        file_path: PathBuf,
    },
    Error(String),
}

pub struct UpdateManager {
    pub status: UpdateStatus,
    receiver: Option<Receiver<UpdateStatus>>,
    cancel_flag: Arc<AtomicBool>,
}

impl Default for UpdateManager {
    fn default() -> Self {
        Self {
            status: UpdateStatus::Idle,
            receiver: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl UpdateManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Poll for status updates from background threads
    pub fn update(&mut self) {
        if let Some(ref rx) = self.receiver {
            while let Ok(new_status) = rx.try_recv() {
                self.status = new_status;
            }
        }
    }

    /// Check if currently busy checking or downloading
    pub fn is_busy(&self) -> bool {
        matches!(
            self.status,
            UpdateStatus::Checking | UpdateStatus::Downloading { .. }
        )
    }

    /// Cancel any active download
    pub fn cancel(&mut self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
        self.status = UpdateStatus::Idle;
        self.receiver = None;
    }

    /// Start an asynchronous update check against GitHub Releases
    pub fn check_for_updates(&mut self) {
        self.cancel_flag.store(false, Ordering::SeqCst);
        self.status = UpdateStatus::Checking;

        let (tx, rx) = crossbeam_channel::unbounded();
        self.receiver = Some(rx);

        thread::spawn(move || {
            let api_url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
            let response = ureq::get(&api_url)
                .set("User-Agent", "mlv-explorer-updater")
                .set("Accept", "application/vnd.github.v3+json")
                .timeout(std::time::Duration::from_secs(10))
                .call();

            match response {
                Ok(resp) => {
                    match resp.into_json::<GithubRelease>() {
                        Ok(release) => {
                            let clean_tag = release.tag_name.trim_start_matches('v').trim();
                            if is_newer_version(CURRENT_VERSION, clean_tag) {
                                // Find matching asset for the current OS
                                let (asset_name, download_url, asset_size) = find_best_asset(&release.assets);
                                let _ = tx.send(UpdateStatus::UpdateAvailable {
                                    current_version: CURRENT_VERSION.to_string(),
                                    latest_version: clean_tag.to_string(),
                                    release_title: release.name.unwrap_or_else(|| release.tag_name.clone()),
                                    release_notes: release.body.unwrap_or_default(),
                                    release_url: release.html_url,
                                    target_asset_name: asset_name,
                                    target_download_url: download_url,
                                    target_asset_size: asset_size,
                                });
                            } else {
                                let _ = tx.send(UpdateStatus::UpToDate {
                                    current_version: CURRENT_VERSION.to_string(),
                                });
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(UpdateStatus::Error(format!("Failed to parse release response: {e}")));
                        }
                    }
                }
                Err(ureq::Error::Status(404, _)) => {
                    // No releases published yet
                    let _ = tx.send(UpdateStatus::UpToDate {
                        current_version: CURRENT_VERSION.to_string(),
                    });
                }
                Err(e) => {
                    let _ = tx.send(UpdateStatus::Error(format!("Network error checking for updates: {e}")));
                }
            }
        });
    }

    /// Start downloading the chosen asset in a background thread with progress reporting
    pub fn download_update(
        &mut self,
        latest_version: String,
        asset_name: String,
        download_url: String,
    ) {
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&self.cancel_flag);

        self.status = UpdateStatus::Downloading {
            latest_version: latest_version.clone(),
            asset_name: asset_name.clone(),
            bytes_downloaded: 0,
            total_bytes: None,
        };

        let (tx, rx) = crossbeam_channel::unbounded();
        self.receiver = Some(rx);

        thread::spawn(move || {
            let target_dir = get_download_dir();
            let dest_path = target_dir.join(&asset_name);

            let resp = match ureq::get(&download_url)
                .set("User-Agent", "mlv-explorer-updater")
                .timeout(std::time::Duration::from_secs(60))
                .call()
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(UpdateStatus::Error(format!("Failed to initiate download: {e}")));
                    return;
                }
            };

            let total_bytes = resp
                .header("Content-Length")
                .and_then(|val| val.parse::<u64>().ok());

            let mut reader = resp.into_reader();
            let mut file = match File::create(&dest_path) {
                Ok(f) => f,
                Err(e) => {
                    let _ = tx.send(UpdateStatus::Error(format!("Failed to create destination file {}: {e}", dest_path.display())));
                    return;
                }
            };

            let mut buffer = [0u8; 65536];
            let mut bytes_downloaded: u64 = 0;
            let mut last_reported = std::time::Instant::now();

            loop {
                if cancel.load(Ordering::SeqCst) {
                    let _ = std::fs::remove_file(&dest_path);
                    return;
                }

                match reader.read(&mut buffer) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        if let Err(e) = file.write_all(&buffer[..n]) {
                            let _ = tx.send(UpdateStatus::Error(format!("Disk write error: {e}")));
                            return;
                        }
                        bytes_downloaded += n as u64;

                        // Throttle progress events to every 100ms
                        if last_reported.elapsed() >= std::time::Duration::from_millis(100) {
                            let _ = tx.send(UpdateStatus::Downloading {
                                latest_version: latest_version.clone(),
                                asset_name: asset_name.clone(),
                                bytes_downloaded,
                                total_bytes,
                            });
                            last_reported = std::time::Instant::now();
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(UpdateStatus::Error(format!("Download stream interrupted: {e}")));
                        return;
                    }
                }
            }

            let _ = file.flush();

            let _ = tx.send(UpdateStatus::Downloaded {
                latest_version,
                asset_name,
                file_path: dest_path,
            });
        });
    }
}

/// Parse semantic version string into numeric parts (major, minor, patch)
fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let parts: Vec<&str> = v.trim_start_matches('v').split('.').collect();
    if parts.is_empty() {
        return None;
    }
    let major = parts.get(0).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let minor = parts.get(1).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let patch = parts.get(2).and_then(|s| s.split('-').next()).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

/// Compare two versions. Returns true if `latest` is newer than `current`.
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    let c = parse_version(current);
    let l = parse_version(latest);

    match (c, l) {
        (Some((c_maj, c_min, c_pat)), Some((l_maj, l_min, l_pat))) => {
            if l_maj != c_maj {
                l_maj > c_maj
            } else if l_min != c_min {
                l_min > c_min
            } else {
                l_pat > c_pat
            }
        }
        _ => latest != current,
    }
}

/// Select best asset for the current operating system
fn find_best_asset(assets: &[GithubAsset]) -> (Option<String>, Option<String>, Option<u64>) {
    let os = std::env::consts::OS;

    // 1. Try exact platform match
    let matched = assets.iter().find(|a| {
        let name = a.name.to_lowercase();
        match os {
            "windows" => name.ends_with(".zip") && (name.contains("windows") || name.contains("win")),
            "linux" => name.ends_with(".appimage") || (name.contains("linux") && name.ends_with(".tar.gz")),
            _ => false,
        }
    });

    // 2. Fallback to generic extension
    let fallback = matched.or_else(|| {
        assets.iter().find(|a| {
            let name = a.name.to_lowercase();
            match os {
                "windows" => name.ends_with(".zip"),
                "linux" => name.ends_with(".appimage") || name.ends_with(".tar.gz"),
                _ => false,
            }
        })
    });

    if let Some(asset) = fallback {
        (Some(asset.name.clone()), Some(asset.browser_download_url.clone()), Some(asset.size))
    } else {
        (None, None, None)
    }
}

/// Find the appropriate download directory on user system
fn get_download_dir() -> PathBuf {
    if let Ok(home) = std::env::var("USERPROFILE") {
        let downloads = PathBuf::from(home).join("Downloads");
        if downloads.exists() {
            return downloads;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let downloads = PathBuf::from(home).join("Downloads");
        if downloads.exists() {
            return downloads;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparisons() {
        assert!(is_newer_version("0.0.1", "0.0.2"));
        assert!(is_newer_version("0.0.1", "0.1.0"));
        assert!(is_newer_version("0.0.1", "1.0.0"));
        assert!(is_newer_version("0.0.1", "v0.0.2"));
        assert!(!is_newer_version("0.0.2", "0.0.1"));
        assert!(!is_newer_version("0.0.1", "0.0.1"));
        assert!(!is_newer_version("1.0.0", "0.9.9"));
    }

    #[test]
    fn test_asset_matching() {
        let assets = vec![
            GithubAsset {
                name: "mlv-explorer-v0.0.2-windows-x64.zip".to_string(),
                browser_download_url: "https://example.com/win.zip".to_string(),
                size: 1024,
            },
            GithubAsset {
                name: "mlv-explorer-v0.0.2-linux-x64.tar.gz".to_string(),
                browser_download_url: "https://example.com/linux.tar.gz".to_string(),
                size: 2048,
            },
            GithubAsset {
                name: "mlv-explorer-v0.0.2-x86_64.AppImage".to_string(),
                browser_download_url: "https://example.com/linux.AppImage".to_string(),
                size: 4096,
            },
        ];

        let (name, url, size) = find_best_asset(&assets);
        assert!(name.is_some());
        assert!(url.is_some());
        assert!(size.is_some());
    }
}

