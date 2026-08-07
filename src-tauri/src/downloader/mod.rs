use std::path::Path;

use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::ShellExt;

use crate::database::Db;
use crate::models::{Download, MediaFormat, MediaInfo, PlaylistEntry, Settings};

/// Browsers yt-dlp can read cookies from. Anything else is ignored so we never
/// pass an arbitrary string through to the sidecar.
const SUPPORTED_COOKIE_BROWSERS: &[&str] = &[
    "chrome", "chromium", "edge", "firefox", "brave", "opera", "vivaldi", "safari", "whale",
];

/// `--cookies-from-browser <browser>` args, or empty when disabled/invalid.
pub fn cookies_args(settings: &Settings) -> Vec<String> {
    let b = settings.cookies_browser.trim().to_ascii_lowercase();
    if SUPPORTED_COOKIE_BROWSERS.contains(&b.as_str()) {
        vec!["--cookies-from-browser".into(), b]
    } else {
        Vec::new()
    }
}

/// Turn raw yt-dlp stderr into a message a user can act on (PRD §16).
pub fn friendly_error(stderr: &str) -> String {
    let lower = stderr.to_lowercase();
    if lower.contains("unsupported url") {
        return "This website or URL is not supported.".into();
    }
    if lower.contains("private video") || lower.contains("this video is private") {
        return "This content is private and cannot be downloaded.".into();
    }
    if lower.contains("sign in") || lower.contains("login required") || lower.contains("--cookies")
    {
        return "This content requires a login. Login-protected downloads are not supported yet.".into();
    }
    if lower.contains("http error 404") || lower.contains("does not exist") {
        return "Content not found. It may have been removed or the URL is wrong.".into();
    }
    if lower.contains("http error 403") {
        return "Access denied by the website (HTTP 403).".into();
    }
    if lower.contains("getaddrinfo")
        || lower.contains("timed out")
        || lower.contains("temporary failure")
        || lower.contains("connection")
        || lower.contains("network")
    {
        return "Network error. Check your internet connection and try again.".into();
    }
    if lower.contains("no space left") || lower.contains("disk full") {
        return "Not enough disk space to finish the download.".into();
    }
    if lower.contains("ffmpeg") && (lower.contains("not found") || lower.contains("not installed"))
    {
        return "FFmpeg is missing. Reinstall the app to restore it.".into();
    }
    // Fall back to the last meaningful stderr line.
    stderr
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().trim_start_matches("ERROR:").trim().to_string())
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| "Download failed.".into())
}

fn json_str(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn json_f64(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|v| v.as_f64())
}

fn entry_thumbnail(entry: &Value) -> Option<String> {
    if let Some(t) = json_str(entry, "thumbnail") {
        return Some(t);
    }
    entry
        .get("thumbnails")
        .and_then(|t| t.as_array())
        .and_then(|arr| arr.last())
        .and_then(|t| t.get("url"))
        .and_then(|u| u.as_str())
        .map(String::from)
}

/// Run `yt-dlp -J` and shape the result for the UI.
pub async fn fetch_metadata(app: &AppHandle, url: &str) -> Result<MediaInfo, String> {
    let mut args: Vec<String> =
        vec!["-J".into(), "--flat-playlist".into(), "--no-warnings".into()];
    if let Ok(settings) = app.state::<Db>().get_settings() {
        args.extend(cookies_args(&settings));
    }
    args.push("--".into());
    args.push(url.to_string());

    let command = app
        .shell()
        .sidecar("yt-dlp")
        .map_err(|_| "yt-dlp is missing. Reinstall the app to restore it.".to_string())?
        .args(args);

    let output = command
        .output()
        .await
        .map_err(|e| format!("Could not start yt-dlp: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(friendly_error(&stderr));
    }

    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "Could not read media information for this URL.".to_string())?;

    let platform = json_str(&json, "extractor_key")
        .or_else(|| json_str(&json, "extractor"))
        .unwrap_or_else(|| "Unknown".into());

    let is_playlist = json.get("_type").and_then(|t| t.as_str()) == Some("playlist");

    if is_playlist {
        let entries = json
            .get("entries")
            .and_then(|e| e.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|entry| {
                        let entry_url = json_str(entry, "url")
                            .or_else(|| json_str(entry, "webpage_url"))?;
                        Some(PlaylistEntry {
                            title: json_str(entry, "title")
                                .unwrap_or_else(|| "Untitled".into()),
                            duration: json_f64(entry, "duration"),
                            thumbnail: entry_thumbnail(entry),
                            url: entry_url,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if entries.is_empty() {
            return Err("This playlist is empty or its items are unavailable.".into());
        }

        return Ok(MediaInfo {
            kind: "playlist".into(),
            url: url.to_string(),
            title: json_str(&json, "title").unwrap_or_else(|| "Playlist".into()),
            platform,
            thumbnail: entry_thumbnail(&json),
            duration: None,
            uploader: json_str(&json, "uploader").or_else(|| json_str(&json, "channel")),
            formats: Vec::new(),
            entries,
        });
    }

    let formats = json
        .get("formats")
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| {
                    Some(MediaFormat {
                        format_id: json_str(f, "format_id")?,
                        ext: json_str(f, "ext").unwrap_or_default(),
                        height: f.get("height").and_then(|h| h.as_i64()),
                        fps: json_f64(f, "fps"),
                        vcodec: json_str(f, "vcodec"),
                        acodec: json_str(f, "acodec"),
                        filesize: f
                            .get("filesize")
                            .and_then(|s| s.as_i64())
                            .or_else(|| f.get("filesize_approx").and_then(|s| s.as_i64())),
                        format_note: json_str(f, "format_note"),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(MediaInfo {
        kind: "video".into(),
        url: json_str(&json, "webpage_url").unwrap_or_else(|| url.to_string()),
        title: json_str(&json, "title").unwrap_or_else(|| "Untitled".into()),
        platform,
        thumbnail: entry_thumbnail(&json),
        duration: json_f64(&json, "duration"),
        uploader: json_str(&json, "uploader").or_else(|| json_str(&json, "channel")),
        formats,
        entries: Vec::new(),
    })
}

/// Progress line marker written via --progress-template.
pub const PROGRESS_PREFIX: &str = "PROG|";

pub const PROGRESS_TEMPLATE: &str = "download:PROG|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s";

fn parse_field(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "NA" || trimmed == "None" || trimmed == "null" {
        return None;
    }
    trimmed.parse::<f64>().ok()
}

/// Parse one `PROG|downloaded|total|total_estimate|speed|eta` line.
pub fn parse_progress(line: &str) -> Option<(i64, Option<i64>, Option<f64>, Option<i64>)> {
    let rest = line.trim().strip_prefix(PROGRESS_PREFIX)?;
    let mut parts = rest.split('|');
    let downloaded = parse_field(parts.next()?)? as i64;
    let total = parse_field(parts.next().unwrap_or(""));
    let total_estimate = parse_field(parts.next().unwrap_or(""));
    let speed = parse_field(parts.next().unwrap_or(""));
    let eta = parse_field(parts.next().unwrap_or("")).map(|v| v as i64);
    let total_bytes = total.or(total_estimate).map(|v| v as i64);
    Some((downloaded, total_bytes, speed, eta))
}

/// Directory holding the bundled sidecar binaries (next to the app executable).
fn sidecar_dir() -> Option<std::path::PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

/// Format seconds for yt-dlp `--download-sections` (accepts plain seconds).
fn fmt_secs(s: f64) -> String {
    // Trim to 2 decimals, drop a trailing ".00".
    let rounded = (s * 100.0).round() / 100.0;
    if (rounded.fract()).abs() < f64::EPSILON {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded:.2}")
    }
}

/// Build the `-f` selector for a video download.
///
/// The container decides which codecs are actually playable: an MP4 holding an
/// Opus track muxes fine but plays silent in Windows Media Player, Movies & TV,
/// QuickTime and most phone players, and a WebM cannot hold AVC/AAC at all. So
/// we ask for container-native codecs first and only widen the net if the site
/// has nothing better, which keeps every download succeeding *and* audible.
fn video_format_selector(height: Option<&str>, container: &str) -> String {
    let h = height
        .map(|h| format!("[height<={h}]"))
        .unwrap_or_default();

    // (video codec filter, audio codec filter, video ext, audio ext)
    let native = match container {
        "mp4" => Some(("[vcodec^=avc1]", "[acodec^=mp4a]", "mp4", "m4a")),
        "webm" => Some(("[vcodec^=vp9]", "[acodec^=opus]", "webm", "webm")),
        // mkv (and anything else) carries any codec combination.
        _ => None,
    };

    let mut tiers: Vec<String> = Vec::new();
    if let Some((vcodec, acodec, vext, aext)) = native {
        tiers.push(format!("bestvideo{h}{vcodec}+bestaudio{acodec}"));
        tiers.push(format!("bestvideo{h}[ext={vext}]+bestaudio[ext={aext}]"));
        tiers.push(format!("best{h}[ext={vext}]"));
    }
    tiers.push(format!("bestvideo{h}+bestaudio"));
    if !h.is_empty() {
        tiers.push(format!("best{h}"));
    }
    tiers.push("best".into());
    tiers.join("/")
}

/// Build the yt-dlp argument list for a queued download.
pub fn build_download_args(
    download: &Download,
    settings: &Settings,
    out_dir: &Path,
    print_path: &Path,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "--no-playlist".into(),
        "--newline".into(),
        "--no-warnings".into(),
        "--continue".into(),
        "--no-simulate".into(),
        "--progress".into(),
        "--progress-template".into(),
        PROGRESS_TEMPLATE.into(),
        "--print-to-file".into(),
        "after_move:filepath".into(),
        print_path.to_string_lossy().into_owned(),
        "-o".into(),
        out_dir
            .join(&settings.filename_template)
            .to_string_lossy()
            .into_owned(),
    ];

    #[cfg(target_os = "windows")]
    args.push("--windows-filenames".into());

    // Use browser cookies for login-gated sites, when configured.
    args.extend(cookies_args(settings));

    // Pro "cutout": download only a time section of a long video.
    if download.clip_start.is_some() || download.clip_end.is_some() {
        let start = download.clip_start.unwrap_or(0.0).max(0.0);
        let section = match download.clip_end {
            Some(end) if end > start => format!("*{}-{}", fmt_secs(start), fmt_secs(end)),
            _ => format!("*{}-inf", fmt_secs(start)),
        };
        args.push("--download-sections".into());
        args.push(section);
        args.push("--force-keyframes-at-cuts".into());
    }

    if let Some(dir) = sidecar_dir() {
        let ffmpeg = dir.join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });
        if ffmpeg.exists() {
            args.push("--ffmpeg-location".into());
            args.push(dir.to_string_lossy().into_owned());
        }
    }

    if download.audio_only {
        args.push("-f".into());
        args.push("bestaudio/best".into());
        args.push("-x".into());
        if !download.format.is_empty() && download.format != "best" {
            args.push("--audio-format".into());
            args.push(download.format.clone());
            args.push("--audio-quality".into());
            args.push("0".into());
        }
    } else {
        let height = download
            .resolution
            .as_ref()
            .map(|r| r.trim_end_matches('p').to_string())
            .filter(|h| h.parse::<u32>().is_ok());
        args.push("-f".into());
        args.push(video_format_selector(height.as_deref(), &download.format));
        if !download.format.is_empty() {
            args.push("--merge-output-format".into());
            args.push(download.format.clone());
        }
    }

    args.push("--".into());
    args.push(download.url.clone());
    args
}

#[cfg(test)]
mod tests {
    use super::video_format_selector;

    #[test]
    fn mp4_prefers_codecs_that_stay_audible() {
        let sel = video_format_selector(Some("1080"), "mp4");
        // First choice pins AVC video + AAC audio, so the merged MP4 plays
        // everywhere instead of carrying a silent-on-Windows Opus track.
        assert!(sel.starts_with("bestvideo[height<=1080][vcodec^=avc1]+bestaudio[acodec^=mp4a]/"));
        // …and it still degrades to something downloadable.
        assert!(sel.ends_with("/bestvideo[height<=1080]+bestaudio/best[height<=1080]/best"));
    }

    #[test]
    fn webm_prefers_vp9_and_opus() {
        let sel = video_format_selector(None, "webm");
        assert!(sel.starts_with("bestvideo[vcodec^=vp9]+bestaudio[acodec^=opus]/"));
        assert!(!sel.contains("height<="));
    }

    #[test]
    fn mkv_takes_any_codec_pair() {
        assert_eq!(
            video_format_selector(Some("720"), "mkv"),
            "bestvideo[height<=720]+bestaudio/best[height<=720]/best"
        );
    }

    #[test]
    fn every_selector_ends_with_a_catch_all() {
        for container in ["mp4", "webm", "mkv", ""] {
            for height in [None, Some("480")] {
                let sel = video_format_selector(height, container);
                assert!(sel.ends_with("/best"), "{container}/{height:?}: {sel}");
                assert!(sel.contains("+bestaudio"), "{container}/{height:?}: {sel}");
            }
        }
    }
}
