use tauri::{AppHandle, Manager};

/// Default download location: `<system Downloads>/Snagreel`,
/// falling back to the user's home directory.
pub fn default_download_dir(app: &AppHandle) -> String {
    let base = app
        .path()
        .download_dir()
        .or_else(|_| app.path().home_dir())
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    base.join("Snagreel").to_string_lossy().into_owned()
}
