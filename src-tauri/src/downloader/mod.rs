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
    if lower.contains("cannot parse data") || lower.contains("unable to extract") {
        // Sites like Facebook rotate several page layouts for the same URL and
        // the extractor only understands some of them, so this is usually
        // luck rather than anything the user did. yt-dlp's own text tells them
        // to file a bug report, which is a dead end — don't pass that on.
        return "This site returned an unexpected response. Please try again in a moment.".into();
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
    if lower.contains("unable to open for writing") || lower.contains("errno 22") {
        // Almost always the file name: sites like Facebook hand back the whole
        // post caption as the title, and Windows refuses a name that long.
        return "The name from this page was too long for Windows. Try the download again — it will be saved under a shorter name.".into();
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

/// Is this failure worth another attempt?
///
/// Some sites serve several page layouts for the same URL and the extractor
/// only parses some of them, so an identical request can fail and then succeed
/// seconds later. Permanent failures are listed first and win outright, so a
/// user never waits through retries for something that can never work.
pub fn is_transient(stderr: &str) -> bool {
    let lower = stderr.to_lowercase();
    let permanent = [
        "unsupported url",
        "private video",
        "this video is private",
        "login required",
        "sign in",
        "http error 404",
        "does not exist",
        "no space left",
        "removed by the uploader",
    ];
    if permanent.iter().any(|p| lower.contains(p)) {
        return false;
    }
    let retryable = [
        "cannot parse data",
        "unable to extract",
        "unable to download webpage",
        "http error 5",
        "timed out",
        "temporary failure",
        "connection reset",
        "connection aborted",
    ];
    retryable.iter().any(|r| lower.contains(r))
}

/// Attempts for a metadata fetch, and the waits between them.
const METADATA_ATTEMPTS: usize = 3;
const RETRY_BACKOFF_MS: [u64; 2] = [400, 1200];

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

    // Retry transient extractor failures rather than surfacing them: the same
    // request can fail and then succeed moments later (see `is_transient`).
    let mut stdout = Vec::new();
    for attempt in 0..METADATA_ATTEMPTS {
        let command = app
            .shell()
            .sidecar("yt-dlp")
            .map_err(|_| "yt-dlp is missing. Reinstall the app to restore it.".to_string())?
            .args(args.clone());

        let output = command
            .output()
            .await
            .map_err(|e| format!("Could not start yt-dlp: {e}"))?;

        if output.status.success() {
            stdout = output.stdout;
            break;
        }

        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let last_attempt = attempt + 1 == METADATA_ATTEMPTS;
        if last_attempt || !is_transient(&stderr) {
            return Err(friendly_error(&stderr));
        }
        tokio::time::sleep(std::time::Duration::from_millis(RETRY_BACKOFF_MS[attempt])).await;
    }

    let json: Value = serde_json::from_slice(&stdout)
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

/// Longest output path we let yt-dlp produce, in characters.
///
/// Windows caps a single path component at 255 characters and the whole path at
/// 260 unless long paths are enabled. Facebook, Instagram and TikTok return the
/// entire post caption as `%(title)s`, which sails past both limits — the write
/// then fails with a bare "unable to open for writing: [Errno 22] Invalid
/// argument", which tells the user nothing and never succeeds on retry.
///
/// Kept well under 260 because yt-dlp counts characters while Windows counts
/// UTF-16 units, and the emoji these captions are full of cost two apiece.
const MAX_PATH_CHARS: usize = 200;

/// Room to leave for what yt-dlp appends after the template: the format id
/// (Facebook's run to `.f1769057684235718v`), the extension, and the
/// `.part`/`.temp` working suffixes.
const PATH_SUFFIX_HEADROOM: usize = 30;

/// Never squeeze the title below this, even from a deeply nested folder — a
/// valid long path beats a truncated one.
const MIN_FILENAME_CHARS: usize = 40;

/// Value for `--trim-filenames`, which caps the rendered output path (folder
/// included — yt-dlp slices the whole string, not just the base name).
///
/// Only a backstop for fields other than the title: see [`bounded_template`]
/// for why this option cannot be trusted on its own.
fn trim_filenames_len(out_dir: &Path) -> usize {
    let dir_chars = out_dir.to_string_lossy().chars().count() + 1; // + separator
    MAX_PATH_CHARS
        .saturating_sub(PATH_SUFFIX_HEADROOM)
        .max(dir_chars + MIN_FILENAME_CHARS)
}

/// How many characters of `%(title)s` fit in the path, after the folder and
/// everything yt-dlp appends have taken their share.
fn title_budget(out_dir: &Path) -> usize {
    let dir_chars = out_dir.to_string_lossy().chars().count() + 1; // + separator
    MAX_PATH_CHARS
        .saturating_sub(dir_chars + PATH_SUFFIX_HEADROOM)
        .max(MIN_FILENAME_CHARS)
}

/// Bound the title *inside* the output template, as `%(title).150s`.
///
/// `--trim-filenames` alone does not work, and fails in a way that looks
/// random. yt-dlp implements it as
///
/// ```python
/// no_ext, *ext = filename.rsplit('.', 2)
/// filename = join_nonempty(no_ext[:trim_file_name], *ext, delim='.')
/// ```
///
/// — it splits the *whole path* on the last two dots and truncates only the
/// part before them, then glues the rest back on untouched. One dot anywhere
/// in the caption ("3.9M views", "TalkyParrot 2.0") leaves almost nothing on
/// the left of that split, so the truncation does nothing and the full name is
/// reassembled. The same Facebook reel therefore succeeds while the view count
/// reads "4M views" and fails once it ticks over to "3.9M views".
///
/// Template precision truncates the field itself, before any of that, so no
/// caption can defeat it.
fn bounded_template(template: &str, out_dir: &Path) -> String {
    // A template that already carries its own precision is left alone.
    template.replace(
        "%(title)s",
        &format!("%(title).{}s", title_budget(out_dir)),
    )
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
            .join(bounded_template(&settings.filename_template, out_dir))
            .to_string_lossy()
            .into_owned(),
        // `--windows-filenames` only removes illegal characters; nothing in
        // yt-dlp shortens an over-long name. This bounds every field other
        // than the title, which `bounded_template` has already capped.
        "--trim-filenames".into(),
        trim_filenames_len(out_dir).to_string(),
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
    use std::path::Path;

    use super::{bounded_template, build_download_args, friendly_error, is_transient,
                title_budget, trim_filenames_len, video_format_selector, MAX_PATH_CHARS,
                METADATA_ATTEMPTS, MIN_FILENAME_CHARS, RETRY_BACKOFF_MS};
    use crate::models::{Download, Settings};

    fn a_download() -> Download {
        Download {
            id: 1,
            url: "https://www.facebook.com/watch/?v=1".into(),
            title: "A parrot".into(),
            platform: "facebook".into(),
            thumbnail: None,
            filename: None,
            file_path: None,
            format: "mp4".into(),
            resolution: Some("1080p".into()),
            audio_only: false,
            duration: None,
            file_size: None,
            status: "queued".into(),
            error: None,
            created_at: String::new(),
            completed_at: None,
            priority: 0,
            position: 0,
            clip_start: None,
            clip_end: None,
        }
    }

    fn settings_with(download_path: &str) -> Settings {
        Settings {
            download_path: download_path.into(),
            theme: "dark".into(),
            language: "en".into(),
            max_concurrent_downloads: 2,
            auto_update: true,
            notifications: true,
            filename_template: "%(title)s.%(ext)s".into(),
            organize_by_platform: false,
            cookies_browser: "none".into(),
        }
    }

    #[test]
    fn parse_failures_are_retried() {
        // The exact Facebook failure that prompted this: intermittent, and it
        // succeeds on a later attempt.
        assert!(is_transient(
            "ERROR: [facebook] 912571498105335: Cannot parse data; please report this issue"
        ));
        assert!(is_transient("ERROR: unable to extract player response"));
        assert!(is_transient("ERROR: Unable to download webpage: HTTP Error 503"));
    }

    #[test]
    fn hopeless_failures_are_not_retried() {
        // Retrying these only makes the user wait for the same answer.
        for stderr in [
            "ERROR: Unsupported URL: https://example.com/x",
            "ERROR: This video is private",
            "ERROR: HTTP Error 404: Not Found",
            "ERROR: Sign in to confirm your age",
        ] {
            assert!(!is_transient(stderr), "would retry: {stderr}");
        }
    }

    #[test]
    fn a_permanent_reason_beats_a_retryable_one() {
        // Login walls often say "unable to extract" too; the permanent reason
        // has to win or we retry a wall three times.
        assert!(!is_transient(
            "ERROR: unable to extract data; login required to view this video"
        ));
    }

    #[test]
    fn parse_failure_copy_is_actionable() {
        let message =
            friendly_error("ERROR: [facebook] 123: Cannot parse data; please report this issue on https://github.com/yt-dlp/yt-dlp/issues");
        assert_eq!(
            message,
            "This site returned an unexpected response. Please try again in a moment."
        );
        // Users must not be pointed at yt-dlp's bug tracker for this.
        assert!(!message.contains("github"));
        assert!(!message.contains("report"));
    }

    #[test]
    fn there_is_a_backoff_for_every_retry() {
        // One wait between each pair of attempts; a mismatch would panic on
        // indexing RETRY_BACKOFF_MS at run time.
        assert_eq!(RETRY_BACKOFF_MS.len(), METADATA_ATTEMPTS - 1);
    }


    #[test]
    fn long_titles_are_trimmed_to_a_writable_length() {
        // A Facebook caption used as the title: Windows rejects the whole path
        // with "[Errno 22] Invalid argument" long before it reaches disk.
        let len = trim_filenames_len(Path::new("D:\\Saved Videos"));
        assert!(len <= MAX_PATH_CHARS, "leaves no room for .f399.mp4.part");
        // The folder eats into the same budget, so the title must still get a
        // usable share of it.
        assert!(len > "D:\\Saved Videos".chars().count() + MIN_FILENAME_CHARS);
    }

    #[test]
    fn a_deep_folder_never_truncates_the_folder_itself() {
        // yt-dlp slices the rendered path as one string, so a limit shorter
        // than the folder would silently write somewhere else — or nowhere.
        let deep = format!("D:\\{}", "nested\\".repeat(40));
        let len = trim_filenames_len(Path::new(&deep));
        assert!(len >= deep.chars().count() + MIN_FILENAME_CHARS);
    }

    #[test]
    fn the_title_is_capped_inside_the_template() {
        // `--trim-filenames` is defeated by a single dot in the caption, so the
        // cap has to live in the template where no title can reach it.
        let tmpl = bounded_template("%(title)s.%(ext)s", Path::new("D:\\Saved Videos"));
        assert_eq!(tmpl, "%(title).154s.%(ext)s");
    }

    #[test]
    fn a_template_without_a_title_is_left_alone() {
        let tmpl = bounded_template("%(id)s.%(ext)s", Path::new("D:\\Saved Videos"));
        assert_eq!(tmpl, "%(id)s.%(ext)s");
    }

    #[test]
    fn an_existing_precision_is_not_doubled() {
        // Only the bare field is rewritten, so a user who already capped the
        // title keeps their own number.
        let tmpl = bounded_template("%(title).50s.%(ext)s", Path::new("D:\\Saved Videos"));
        assert_eq!(tmpl, "%(title).50s.%(ext)s");
    }

    #[test]
    fn the_title_cap_leaves_room_for_the_rest_of_the_path() {
        let dir = Path::new("D:\\Saved Videos");
        // Folder + title + the longest suffix yt-dlp appends (a Facebook format
        // id, the extension and `.part`) must still clear the Windows limits.
        let worst = dir.as_os_str().len() + 1 + title_budget(dir) + ".f1769057684235718v.mp4.part".len();
        assert!(worst < 255, "component would be rejected: {worst}");
        assert!(worst < 260, "path would be rejected: {worst}");
    }

    #[test]
    fn a_deep_folder_still_leaves_a_usable_title() {
        let deep = format!("D:\\{}", "nested\\".repeat(40));
        assert_eq!(title_budget(Path::new(&deep)), MIN_FILENAME_CHARS);
    }

    #[test]
    fn downloads_ask_yt_dlp_to_trim() {
        let args = build_download_args(
            &a_download(),
            &settings_with("D:\\Saved Videos"),
            Path::new("D:\\Saved Videos"),
            Path::new("C:\\cache\\out.txt"),
        );
        let at = args.iter().position(|a| a == "--trim-filenames").expect(
            "without this, an over-long title fails the download permanently",
        );
        assert_eq!(
            args[at + 1],
            trim_filenames_len(Path::new("D:\\Saved Videos")).to_string()
        );

        // …and the output template carries the cap that actually holds.
        let out = args.iter().position(|a| a == "-o").expect("no -o");
        assert!(args[out + 1].contains("%(title).154s"), "{}", args[out + 1]);
    }

    #[test]
    fn an_unwritable_name_does_not_leak_python_errno_text() {
        let message = friendly_error(
            "ERROR: unable to open for writing: [Errno 22] Invalid argument: 'D:\\\\Saved Videos\\\\39M views 1.2M reactions When the parrot...'",
        );
        assert!(message.contains("too long"), "{message}");
        assert!(!message.contains("Errno"), "{message}");
    }

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
