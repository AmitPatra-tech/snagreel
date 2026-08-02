use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use tauri::{AppHandle, Manager};

use crate::models::Settings;

/// Minimum free space required to start a download (100 MB).
const MIN_FREE_BYTES: u64 = 100 * 1024 * 1024;

/// Compute (and create) the folder a download should be saved into,
/// honoring the optional per-platform organization rule.
pub fn resolve_output_dir(settings: &Settings, platform: &str) -> Result<PathBuf> {
    let mut dir = PathBuf::from(&settings.download_path);
    if settings.organize_by_platform && !platform.is_empty() {
        dir.push(sanitize_component(platform));
    }
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Could not create download folder {}", dir.display()))?;
    Ok(dir)
}

/// Keep folder names derived from website names safe on every OS.
fn sanitize_component(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .trim_end_matches('.')
        .to_string()
}

pub fn ensure_free_space(dir: &Path) -> Result<()> {
    match fs2::available_space(dir) {
        Ok(free) if free < MIN_FREE_BYTES => {
            Err(anyhow!("Not enough disk space in the download folder."))
        }
        _ => Ok(()),
    }
}

/// Scratch file where yt-dlp writes the final output path.
pub fn print_file_path(app: &AppHandle, download_id: i64) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_cache_dir()
        .context("Could not resolve the app cache directory")?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("yt-dlp-out-{download_id}.txt")))
}

/// Open a file with the default application, validating it first.
pub fn open_file(path: &str) -> Result<()> {
    let path = Path::new(path);
    if !path.is_file() {
        return Err(anyhow!("The file no longer exists on disk."));
    }
    tauri_plugin_opener::open_path(path, None::<&str>)
        .map_err(|e| anyhow!("Could not open file: {e}"))
}

/// Reveal a file in Explorer/Finder, validating it first.
pub fn show_in_folder(path: &str) -> Result<()> {
    let path = Path::new(path);
    if !path.exists() {
        return Err(anyhow!("The file no longer exists on disk."));
    }
    tauri_plugin_opener::reveal_item_in_dir(path)
        .map_err(|e| anyhow!("Could not open folder: {e}"))
}

pub fn open_folder(path: &str) -> Result<()> {
    let dir = Path::new(path);
    std::fs::create_dir_all(dir)?;
    tauri_plugin_opener::open_path(dir, None::<&str>)
        .map_err(|e| anyhow!("Could not open folder: {e}"))
}

pub fn delete_file(path: &str) -> Result<()> {
    let path = Path::new(path);
    if path.is_file() {
        std::fs::remove_file(path)
            .with_context(|| format!("Could not delete {}", path.display()))?;
    }
    Ok(())
}
