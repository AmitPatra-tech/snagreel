use tauri::{AppHandle, Emitter, Manager, State};

use crate::activation::{self, ActivationState};
use crate::database::Db;
use crate::downloader;
use crate::editor;
use crate::filesystem;
use crate::models::{
    Download, DownloadRequest, EditRequest, ExtractAudioRequest, LibraryQuery, MediaInfo, Settings,
    TranscribeRequest, TranscribeResult,
};
use crate::transcribe;
use crate::queue::{emit_status, QueueManager};

fn err_string(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
pub async fn fetch_metadata(app: AppHandle, url: String) -> Result<MediaInfo, String> {
    let trimmed = url.trim().to_string();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err("Please enter a valid http(s) URL.".into());
    }
    downloader::fetch_metadata(&app, &trimmed).await
}

#[tauri::command]
pub fn check_duplicate(db: State<Db>, url: String) -> Result<Option<Download>, String> {
    db.get_completed_by_url(url.trim()).map_err(err_string)
}

#[tauri::command]
pub fn start_download(
    app: AppHandle,
    db: State<Db>,
    queue: State<QueueManager>,
    request: DownloadRequest,
) -> Result<Download, String> {
    let url = request.url.trim();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Please enter a valid http(s) URL.".into());
    }
    // "Cutout" (download a section of a long video) is a Pro feature.
    if (request.clip_start.is_some() || request.clip_end.is_some()) && !db.is_pro() {
        return Err("Downloading a clip/section is a Pro feature. Activate Pro to unlock it.".into());
    }
    let download = db.insert_download(&request).map_err(err_string)?;
    let _ = app.emit("queue-updated", ());
    queue.poke();
    Ok(download)
}

#[tauri::command]
pub fn pause_download(
    app: AppHandle,
    db: State<Db>,
    queue: State<QueueManager>,
    id: i64,
) -> Result<(), String> {
    // Mark first so the worker's terminate handler sees the paused state.
    db.set_status(id, "paused", None).map_err(err_string)?;
    db.set_queue_status(id, "paused").map_err(err_string)?;
    queue.kill(id);
    emit_status(&app, id, "paused", None);
    queue.poke();
    Ok(())
}

#[tauri::command]
pub fn resume_download(
    app: AppHandle,
    db: State<Db>,
    queue: State<QueueManager>,
    id: i64,
) -> Result<(), String> {
    db.set_status(id, "queued", None).map_err(err_string)?;
    db.enqueue(id).map_err(err_string)?;
    emit_status(&app, id, "queued", None);
    queue.poke();
    Ok(())
}

#[tauri::command]
pub fn cancel_download(
    app: AppHandle,
    db: State<Db>,
    queue: State<QueueManager>,
    id: i64,
) -> Result<(), String> {
    db.set_status(id, "cancelled", None).map_err(err_string)?;
    db.remove_from_queue(id).map_err(err_string)?;
    queue.kill(id);
    emit_status(&app, id, "cancelled", None);
    queue.poke();
    Ok(())
}

#[tauri::command]
pub fn retry_download(
    app: AppHandle,
    db: State<Db>,
    queue: State<QueueManager>,
    id: i64,
) -> Result<(), String> {
    db.set_status(id, "queued", None).map_err(err_string)?;
    db.enqueue(id).map_err(err_string)?;
    emit_status(&app, id, "queued", None);
    queue.poke();
    Ok(())
}

#[tauri::command]
pub fn remove_download(
    app: AppHandle,
    db: State<Db>,
    queue: State<QueueManager>,
    id: i64,
    delete_file: bool,
) -> Result<(), String> {
    if queue.is_active(id) {
        queue.kill(id);
    }
    let download = db.get_download(id).map_err(err_string)?;
    if delete_file {
        if let Some(path) = download.file_path.as_deref() {
            filesystem::delete_file(path).map_err(err_string)?;
        }
    }
    db.delete_download(id).map_err(err_string)?;
    let _ = app.emit("queue-updated", ());
    queue.poke();
    Ok(())
}

#[tauri::command]
pub fn clear_history(app: AppHandle, db: State<Db>, delete_files: bool) -> Result<(), String> {
    if delete_files {
        for path in db.list_completed_paths().map_err(err_string)? {
            let _ = filesystem::delete_file(&path);
        }
    }
    db.clear_completed().map_err(err_string)?;
    let _ = app.emit("queue-updated", ());
    Ok(())
}

#[tauri::command]
pub fn reorder_queue(
    app: AppHandle,
    db: State<Db>,
    ordered_ids: Vec<i64>,
) -> Result<(), String> {
    db.reorder_queue(&ordered_ids).map_err(err_string)?;
    let _ = app.emit("queue-updated", ());
    Ok(())
}

#[tauri::command]
pub fn list_downloads(db: State<Db>) -> Result<Vec<Download>, String> {
    db.list_downloads().map_err(err_string)
}

#[tauri::command]
pub fn search_library(db: State<Db>, query: LibraryQuery) -> Result<Vec<Download>, String> {
    db.search_library(&query).map_err(err_string)
}

#[tauri::command]
pub fn list_platforms(db: State<Db>) -> Result<Vec<String>, String> {
    db.list_platforms().map_err(err_string)
}

#[tauri::command]
pub fn get_settings(db: State<Db>) -> Result<Settings, String> {
    db.get_settings().map_err(err_string)
}

#[tauri::command]
pub fn update_settings(
    db: State<Db>,
    queue: State<QueueManager>,
    settings: Settings,
) -> Result<(), String> {
    db.update_settings(&settings).map_err(err_string)?;
    // Concurrency may have been raised: give the scheduler a chance to fill slots.
    queue.poke();
    Ok(())
}

#[tauri::command]
pub fn open_file(db: State<Db>, id: i64) -> Result<(), String> {
    let download = db.get_download(id).map_err(err_string)?;
    let path = download
        .file_path
        .ok_or_else(|| "No file recorded for this download.".to_string())?;
    filesystem::open_file(&path).map_err(err_string)
}

#[tauri::command]
pub fn show_in_folder(db: State<Db>, id: i64) -> Result<(), String> {
    let download = db.get_download(id).map_err(err_string)?;
    let path = download
        .file_path
        .ok_or_else(|| "No file recorded for this download.".to_string())?;
    filesystem::show_in_folder(&path).map_err(err_string)
}

// ---------- activation (Pro) ----------

#[tauri::command]
pub fn get_activation(db: State<Db>) -> Result<ActivationState, String> {
    Ok(ActivationState { is_pro: db.is_pro() })
}

#[tauri::command]
pub fn activate_pro(db: State<Db>, key: String) -> Result<ActivationState, String> {
    if !activation::is_valid_key(&key) {
        return Err("That activation key is not valid. Double-check it and try again.".into());
    }
    let hash = activation::hash_key(&key);
    db.set_pro_key_hash(&hash).map_err(err_string)?;
    Ok(ActivationState { is_pro: db.is_pro() })
}

#[tauri::command]
pub fn deactivate_pro(db: State<Db>) -> Result<ActivationState, String> {
    db.clear_pro_key_hash().map_err(err_string)?;
    Ok(ActivationState { is_pro: db.is_pro() })
}

// ---------- editing (Pro) ----------

#[tauri::command]
pub async fn run_edit(
    app: AppHandle,
    request: EditRequest,
    job_id: String,
) -> Result<Download, String> {
    editor::run_edit(&app, request, job_id).await
}

#[tauri::command]
pub async fn transcribe(
    app: AppHandle,
    request: TranscribeRequest,
    job_id: String,
) -> Result<TranscribeResult, String> {
    transcribe::run_transcribe(&app, request, job_id).await
}

#[tauri::command]
pub async fn extract_audio(
    app: AppHandle,
    request: ExtractAudioRequest,
    job_id: String,
) -> Result<Vec<Download>, String> {
    editor::run_extract_audio(&app, request, job_id).await
}

#[tauri::command]
pub fn open_download_folder(app: AppHandle) -> Result<(), String> {
    let db = app.state::<Db>();
    let settings = db.get_settings().map_err(err_string)?;
    filesystem::open_folder(&settings.download_path).map_err(err_string)
}
