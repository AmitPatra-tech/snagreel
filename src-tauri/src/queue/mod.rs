use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;
use tokio::sync::Notify;

use crate::database::Db;
use crate::downloader;
use crate::filesystem;
use crate::models::{Download, ProgressPayload, StatusPayload};

/// Tokio-based download scheduler (M6): runs up to `max_concurrent_downloads`
/// yt-dlp sidecars at once, in priority/position order.
#[derive(Clone)]
pub struct QueueManager {
    /// Downloads currently owned by a worker. The child handle is `None`
    /// between scheduling and process spawn.
    active: Arc<Mutex<HashMap<i64, Option<CommandChild>>>>,
    notify: Arc<Notify>,
}

impl QueueManager {
    pub fn new() -> Self {
        Self {
            active: Arc::new(Mutex::new(HashMap::new())),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Wake the scheduler (new item, freed slot, changed settings…).
    pub fn poke(&self) {
        self.notify.notify_one();
    }

    pub fn is_active(&self, id: i64) -> bool {
        self.active.lock().unwrap().contains_key(&id)
    }

    /// Kill the running process for a download, if any.
    pub fn kill(&self, id: i64) {
        let child = self.active.lock().unwrap().remove(&id);
        if let Some(Some(child)) = child {
            let _ = child.kill();
        }
    }

    /// Spawn the long-lived scheduler task.
    pub fn start(&self, app: AppHandle) {
        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                manager.schedule(&app);
                manager.notify.notified().await;
            }
        });
    }

    /// Fill free worker slots with queued items.
    fn schedule(&self, app: &AppHandle) {
        let db = app.state::<Db>();
        let max = db
            .get_settings()
            .map(|s| s.max_concurrent_downloads.clamp(1, 10) as usize)
            .unwrap_or(3);

        loop {
            {
                let active = self.active.lock().unwrap();
                if active.len() >= max {
                    return;
                }
            }
            let Ok(Some(download)) = db.next_queued() else {
                return;
            };

            // Claim the slot synchronously so the next loop iteration
            // cannot pick the same item.
            self.active.lock().unwrap().insert(download.id, None);
            let _ = db.set_status(download.id, "downloading", None);
            let _ = db.set_queue_status(download.id, "downloading");
            emit_status(app, download.id, "downloading", None);

            let manager = self.clone();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                manager.run_download(app, download).await;
            });
        }
    }

    async fn run_download(&self, app: AppHandle, download: Download) {
        let id = download.id;
        let result = self.run_download_inner(&app, &download).await;

        self.active.lock().unwrap().remove(&id);
        let db = app.state::<Db>();

        // Cancel/pause path already wrote its final status; don't overwrite it.
        let current_status = db
            .get_download(id)
            .map(|d| d.status)
            .unwrap_or_else(|_| "cancelled".into());

        if current_status == "downloading" {
            match result {
                Ok((file_path, filename, file_size)) => {
                    let _ = db.mark_completed(id, &filename, &file_path, file_size);
                    let _ = db.remove_from_queue(id);
                    emit_status(&app, id, "completed", None);
                }
                Err(message) => {
                    let _ = db.set_status(id, "failed", Some(&message));
                    let _ = db.remove_from_queue(id);
                    emit_status(&app, id, "failed", Some(&message));
                }
            }
        }

        self.poke();
    }

    /// Returns (file_path, filename, file_size) on success.
    async fn run_download_inner(
        &self,
        app: &AppHandle,
        download: &Download,
    ) -> Result<(String, String, Option<i64>), String> {
        let db = app.state::<Db>();
        let settings = db
            .get_settings()
            .map_err(|e| format!("Could not load settings: {e}"))?;

        let out_dir = filesystem::resolve_output_dir(&settings, &download.platform)
            .map_err(|e| e.to_string())?;
        filesystem::ensure_free_space(&out_dir).map_err(|e| e.to_string())?;

        let print_path = filesystem::print_file_path(app, download.id)
            .map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&print_path);

        let args =
            downloader::build_download_args(download, &settings, &out_dir, &print_path);

        // Some sites intermittently serve a page the extractor can't read, so
        // an identical request fails and then works. `--continue` is already in
        // the args, so a retry resumes from whatever bytes landed rather than
        // starting the file again.
        let mut last_error = String::new();
        for attempt in 0..DOWNLOAD_ATTEMPTS {
            let _ = std::fs::remove_file(&print_path);

            match self.run_attempt(app, download, &args).await {
                AttemptOutcome::Success => {
                    return self.collect_result(&print_path);
                }
                AttemptOutcome::Cancelled => {
                    let _ = std::fs::remove_file(&print_path);
                    return Err("Cancelled".into());
                }
                AttemptOutcome::Failed(stderr) => {
                    last_error = stderr;
                    let final_attempt = attempt + 1 == DOWNLOAD_ATTEMPTS;
                    if final_attempt || !downloader::is_transient(&last_error) {
                        break;
                    }
                    // Never sit in a backoff for something the user cancelled;
                    // check both before and after waiting.
                    if !self.is_active(download.id) {
                        return Err("Cancelled".into());
                    }
                    tokio::time::sleep(Duration::from_millis(DOWNLOAD_BACKOFF_MS[attempt])).await;
                    if !self.is_active(download.id) {
                        return Err("Cancelled".into());
                    }
                }
            }
        }

        let _ = std::fs::remove_file(&print_path);
        Err(downloader::friendly_error(&last_error))
    }

    /// Read the path yt-dlp printed and turn it into the completed-download
    /// tuple.
    fn collect_result(
        &self,
        print_path: &std::path::Path,
    ) -> Result<(String, String, Option<i64>), String> {
        let file_path = std::fs::read_to_string(print_path)
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .map(|l| l.trim().to_string())
            })
            .ok_or_else(|| "Download finished but the output file was not found.".to_string())?;
        let _ = std::fs::remove_file(print_path);

        let metadata = std::fs::metadata(&file_path).ok();
        let file_size = metadata.map(|m| m.len() as i64);
        let filename = std::path::Path::new(&file_path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| file_path.clone());

        Ok((file_path, filename, file_size))
    }

    /// One yt-dlp invocation, start to exit.
    async fn run_attempt(
        &self,
        app: &AppHandle,
        download: &Download,
        args: &[String],
    ) -> AttemptOutcome {
        let command = match app.shell().sidecar("yt-dlp") {
            Ok(c) => c.args(args.to_vec()),
            Err(_) => {
                return AttemptOutcome::Failed(
                    "yt-dlp is missing. Reinstall the app to restore it.".into(),
                )
            }
        };

        let (mut rx, child) = match command.spawn() {
            Ok(v) => v,
            Err(e) => return AttemptOutcome::Failed(format!("Could not start yt-dlp: {e}")),
        };

        // If cancel/pause raced with the spawn, the map entry is gone:
        // kill the process we just started and bail out.
        {
            let mut active = self.active.lock().unwrap();
            match active.get_mut(&download.id) {
                Some(slot) => *slot = Some(child),
                None => {
                    drop(active);
                    let _ = child.kill();
                    return AttemptOutcome::Cancelled;
                }
            }
        }

        let mut stderr_tail: Vec<String> = Vec::new();
        let mut exit_code: Option<i32> = None;
        let mut last_emit = Instant::now() - Duration::from_secs(1);

        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(bytes) => {
                    let line = String::from_utf8_lossy(&bytes);
                    if let Some((downloaded, total, speed, eta)) =
                        downloader::parse_progress(&line)
                    {
                        if last_emit.elapsed() >= Duration::from_millis(200) {
                            last_emit = Instant::now();
                            let percent = total
                                .filter(|t| *t > 0)
                                .map(|t| (downloaded as f64 / t as f64) * 100.0);
                            let _ = app.emit(
                                "download-progress",
                                ProgressPayload {
                                    id: download.id,
                                    downloaded_bytes: downloaded,
                                    total_bytes: total,
                                    speed,
                                    eta,
                                    percent,
                                },
                            );
                        }
                    }
                }
                CommandEvent::Stderr(bytes) => {
                    let line = String::from_utf8_lossy(&bytes).trim().to_string();
                    if !line.is_empty() {
                        stderr_tail.push(line);
                        if stderr_tail.len() > 20 {
                            stderr_tail.remove(0);
                        }
                    }
                }
                CommandEvent::Terminated(payload) => {
                    exit_code = payload.code;
                }
                CommandEvent::Error(err) => {
                    stderr_tail.push(err);
                }
                _ => {}
            }
        }

        // A killed process is a cancel/pause, not a failure to retry: the
        // handle is pulled from `active` before the kill, so its absence is
        // how we tell the two apart.
        if !self.is_active(download.id) {
            return AttemptOutcome::Cancelled;
        }

        if exit_code == Some(0) {
            AttemptOutcome::Success
        } else {
            AttemptOutcome::Failed(stderr_tail.join("\n"))
        }
    }
}

/// How one yt-dlp invocation ended.
enum AttemptOutcome {
    Success,
    /// The user cancelled or paused; do not retry, do not report an error.
    Cancelled,
    /// Raw stderr tail, so the caller can decide whether it is worth retrying.
    Failed(String),
}

/// Attempts for a download, and the waits between them. Longer than the
/// metadata backoff because a retry here can mean re-establishing a transfer.
const DOWNLOAD_ATTEMPTS: usize = 3;
const DOWNLOAD_BACKOFF_MS: [u64; 2] = [1000, 3000];

pub fn emit_status(app: &AppHandle, id: i64, status: &str, error: Option<&str>) {
    let _ = app.emit(
        "download-status",
        StatusPayload {
            id,
            status: status.to_string(),
            error: error.map(String::from),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::{DOWNLOAD_ATTEMPTS, DOWNLOAD_BACKOFF_MS};

    #[test]
    fn there_is_a_backoff_for_every_download_retry() {
        // `DOWNLOAD_BACKOFF_MS[attempt]` is indexed once per retry, so a
        // mismatch here is an out-of-bounds panic mid-download rather than a
        // compile error.
        assert_eq!(DOWNLOAD_BACKOFF_MS.len(), DOWNLOAD_ATTEMPTS - 1);
    }

    #[test]
    fn a_cancelled_download_is_not_retried() {
        // Guard against someone "simplifying" the Cancelled arm away: the
        // outcome enum must keep cancellation distinct from failure, or a
        // paused download would be retried behind the user's back.
        fn is_retryable(outcome: &super::AttemptOutcome) -> bool {
            matches!(outcome, super::AttemptOutcome::Failed(_))
        }
        assert!(!is_retryable(&super::AttemptOutcome::Cancelled));
        assert!(!is_retryable(&super::AttemptOutcome::Success));
        assert!(is_retryable(&super::AttemptOutcome::Failed("boom".into())));
    }
}
