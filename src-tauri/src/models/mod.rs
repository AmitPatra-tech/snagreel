use serde::{Deserialize, Serialize};

/// A row in the `downloads` table, joined with its optional `queue` row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Download {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub platform: String,
    pub thumbnail: Option<String>,
    pub filename: Option<String>,
    pub file_path: Option<String>,
    pub format: String,
    pub resolution: Option<String>,
    pub audio_only: bool,
    pub duration: Option<f64>,
    pub file_size: Option<i64>,
    pub status: String,
    pub error: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub priority: i64,
    pub position: i64,
    pub clip_start: Option<f64>,
    pub clip_end: Option<f64>,
}

/// What the frontend sends when the user confirms a download.
#[derive(Debug, Clone, Deserialize)]
pub struct DownloadRequest {
    pub url: String,
    pub title: String,
    pub platform: String,
    pub thumbnail: Option<String>,
    pub duration: Option<f64>,
    pub audio_only: bool,
    /// Max video height, e.g. "1080". `None` means best available.
    pub quality: Option<String>,
    /// Target container: mp4/webm/mkv for video, mp3/m4a/wav for audio.
    pub container: String,
    /// Pro "cutout": download only the section [clip_start, clip_end] (seconds).
    /// Both `None` downloads the whole video.
    #[serde(default)]
    pub clip_start: Option<f64>,
    #[serde(default)]
    pub clip_end: Option<f64>,
}

/// A Pro edit job on an existing library item (`source_id`). One operation per
/// run; the result is written as a new file and added to the library.
#[derive(Debug, Clone, Deserialize)]
pub struct EditRequest {
    pub source_id: i64,
    /// "trim" | "convert"
    pub operation: String,
    /// Target extension for convert (mp4/mkv/webm/mp3/m4a/wav).
    #[serde(default)]
    pub output_format: Option<String>,
    #[serde(default)]
    pub start: Option<f64>,
    #[serde(default)]
    pub end: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditProgress {
    pub job_id: String,
    pub percent: Option<f64>,
}

/// A Pro speech-to-text job. Provide either a library `source_id` or a local
/// `input_path` (for a file the user picked from disk).
#[derive(Debug, Clone, Deserialize)]
pub struct TranscribeRequest {
    #[serde(default)]
    pub source_id: Option<i64>,
    #[serde(default)]
    pub input_path: Option<String>,
    /// ISO language code (e.g. "en", "es", "hi") or "auto" to detect.
    pub language: String,
}

/// A Pro audio-extraction job: pull the audio track out of a video (a library
/// item or a local file of any format) into one or more audio formats.
#[derive(Debug, Clone, Deserialize)]
pub struct ExtractAudioRequest {
    #[serde(default)]
    pub source_id: Option<i64>,
    #[serde(default)]
    pub input_path: Option<String>,
    /// Target audio extensions, e.g. ["mp3", "wav"].
    pub formats: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TranscribeResult {
    pub text: String,
    pub txt_path: String,
    pub srt_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TranscribeProgress {
    pub job_id: String,
    /// 0–100 when known, else `None` (indeterminate).
    pub percent: Option<f64>,
    /// Short human-readable stage label.
    pub stage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub download_path: String,
    pub theme: String,
    pub language: String,
    pub max_concurrent_downloads: i64,
    pub auto_update: bool,
    pub notifications: bool,
    pub filename_template: String,
    pub organize_by_platform: bool,
    /// Browser to read cookies from for login-gated sites, or "none".
    /// One of: none | chrome | edge | firefox | brave | opera | vivaldi | chromium.
    pub cookies_browser: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaFormat {
    pub format_id: String,
    pub ext: String,
    pub height: Option<i64>,
    pub fps: Option<f64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub filesize: Option<i64>,
    pub format_note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaylistEntry {
    pub url: String,
    pub title: String,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaInfo {
    pub kind: String,
    pub url: String,
    pub title: String,
    pub platform: String,
    pub thumbnail: Option<String>,
    pub duration: Option<f64>,
    pub uploader: Option<String>,
    pub formats: Vec<MediaFormat>,
    pub entries: Vec<PlaylistEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryQuery {
    pub query: String,
    pub platform: Option<String>,
    pub kind: String,
    pub sort: String,
    pub descending: bool,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgressPayload {
    pub id: i64,
    pub downloaded_bytes: i64,
    pub total_bytes: Option<i64>,
    pub speed: Option<f64>,
    pub eta: Option<i64>,
    pub percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusPayload {
    pub id: i64,
    pub status: String,
    pub error: Option<String>,
}
