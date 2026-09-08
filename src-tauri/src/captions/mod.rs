//! Pro auto-captioning: transcribe the speech in a video, then burn the
//! generated captions into a new copy of it.
//!
//! Reuses [`crate::transcribe::run_transcribe`] for the speech-to-text step
//! (whisper model lookup, 16 kHz audio extraction, progress) rather than
//! duplicating it, then runs a second FFmpeg pass with the `subtitles`
//! filter to render the resulting `.srt` onto the picture. Both steps stream
//! progress on the same `transcribe-progress` event, keyed by `job_id`, so
//! one dialog shows both stages.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

use crate::database::Db;
use crate::editor::{encode_args, ext_of, friendly_error, is_audio_ext, stem_of, unique_output};
use crate::models::{AddCaptionsRequest, Download, TranscribeProgress, TranscribeRequest};
use crate::transcribe;

/// `libass` `force_style` strings for each caption preset. Verified by
/// rendering a real frame from each and inspecting it — ASS colours are
/// `&HAABBGGRR` (alpha, blue, green, red), the reverse of the usual RRGGBB
/// order, and an easy, silent mistake to get backwards.
fn force_style(style: &str, font_size: u32) -> String {
    match style {
        "yellow" => format!(
            "FontName=Arial,FontSize={font_size},PrimaryColour=&H0000FFFF,\
             OutlineColour=&H00000000,BorderStyle=1,Outline=3,Shadow=0,MarginV=40,Bold=1"
        ),
        "boxed" => format!(
            "FontName=Arial,FontSize={font_size},PrimaryColour=&H00FFFFFF,\
             BackColour=&HC0000000,BorderStyle=4,Outline=6,Shadow=0,MarginV=40,Bold=1"
        ),
        "minimal" => format!(
            "FontName=Arial,FontSize={},PrimaryColour=&H00FFFFFF,OutlineColour=&H00000000,\
             BorderStyle=1,Outline=1,Shadow=0,MarginV=30,Bold=0",
            (font_size as f32 * 0.85) as u32
        ),
        // "classic" and any unrecognized id.
        _ => format!(
            "FontName=Arial,FontSize={font_size},PrimaryColour=&H00FFFFFF,\
             OutlineColour=&H00000000,BorderStyle=1,Outline=2,Shadow=0,MarginV=40,Bold=1"
        ),
    }
}

/// Scale caption size to the video. A library item carries its resolution
/// ("1080p"); a freshly picked local file does not, so fall back to a size
/// tuned for the most common downloaded resolutions (720p-1080p) rather than
/// probing the file just for this.
fn caption_font_size(resolution: Option<&str>) -> u32 {
    let height: u32 = resolution
        .and_then(|r| r.trim_end_matches('p').parse().ok())
        .unwrap_or(720);
    (height / 22).clamp(18, 72)
}

/// Escape a filesystem path for use inside an FFmpeg filtergraph option.
/// `:` is the filter's own option separator — colliding with a Windows drive
/// letter — and `\` must be doubled; the whole thing is then wrapped in
/// single quotes so spaces survive too. Verified against the shipped FFmpeg
/// build with a real Windows path (a naive `-replace ':', '\:'` swap is not
/// enough on its own; ordering and doubling both matter).
fn escape_for_filter(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let escaped = raw.replace('\\', "\\\\").replace(':', "\\:");
    format!("'{escaped}'")
}

fn emit(app: &AppHandle, job_id: &str, percent: Option<f64>, stage: &str) {
    let _ = app.emit(
        "transcribe-progress",
        TranscribeProgress {
            job_id: job_id.to_string(),
            percent,
            stage: stage.to_string(),
        },
    );
}

/// Pro: transcribe a library item or local file and burn the result onto a
/// new video. Returns the created library entry.
pub async fn run_add_captions(
    app: &AppHandle,
    req: AddCaptionsRequest,
    job_id: String,
) -> Result<Download, String> {
    let db = app.state::<Db>();
    if !db.is_pro() {
        return Err("Adding captions is a Pro feature. Activate Pro to unlock it.".into());
    }

    let (input, thumbnail, duration, base_title, url, resolution): (
        String,
        Option<String>,
        Option<f64>,
        String,
        String,
        Option<String>,
    ) = if let Some(p) = req.input_path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        (p.to_string(), None, None, stem_of(p), format!("file:///{p}"), None)
    } else if let Some(id) = req.source_id {
        let s = db
            .get_download(id)
            .map_err(|_| "Could not find the item to caption.".to_string())?;
        let path = s
            .file_path
            .clone()
            .ok_or_else(|| "This item has no file on disk.".to_string())?;
        (
            path,
            s.thumbnail.clone(),
            s.duration,
            s.title.clone(),
            s.url.clone(),
            s.resolution.clone(),
        )
    } else {
        return Err("No file was provided.".into());
    };

    if !Path::new(&input).is_file() {
        return Err("The source file could not be found on disk.".into());
    }
    let ext = ext_of(&input);
    if ext.is_empty() {
        return Err("This file has no recognizable format.".into());
    }
    if is_audio_ext(&ext) {
        return Err("Add captions works on videos — this file has no video track.".into());
    }

    // Step 1: transcribe. Emits its own "Preparing audio"/"Transcribing"
    // stages on "transcribe-progress" for this same job_id.
    let transcript = transcribe::run_transcribe(
        app,
        TranscribeRequest {
            source_id: None,
            input_path: Some(input.clone()),
            language: req.language.clone(),
        },
        job_id.clone(),
    )
    .await?;

    // Step 2: burn the generated captions into a new video file.
    emit(app, &job_id, Some(0.0), "Adding captions");

    let dir = {
        let settings = db.get_settings().map_err(|e| e.to_string())?;
        let d = PathBuf::from(&settings.download_path);
        std::fs::create_dir_all(&d)
            .map_err(|e| format!("Could not create the download folder: {e}"))?;
        d
    };
    let stem = stem_of(&input);
    let out_ext = if ["mp4", "mkv", "mov", "webm"].contains(&ext.as_str()) {
        ext.clone()
    } else {
        "mp4".to_string()
    };
    let output = unique_output(&dir, &stem, "captioned", &out_ext);

    let font_size = caption_font_size(resolution.as_deref());
    let style = force_style(&req.style, font_size);
    let vf = format!(
        "subtitles={}:force_style='{style}'",
        escape_for_filter(Path::new(&transcript.srt_path))
    );

    let mut args: Vec<String> = vec![
        "-y".into(),
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
        "-i".into(),
        input.clone(),
        "-vf".into(),
        vf,
    ];
    args.extend(encode_args(&out_ext));
    args.push(output.to_string_lossy().into_owned());

    let command = app
        .shell()
        .sidecar("ffmpeg")
        .map_err(|_| "FFmpeg is missing. Reinstall the app to restore it.".to_string())?
        .args(args);

    let (mut rx, _child) = command
        .spawn()
        .map_err(|e| format!("Could not start FFmpeg: {e}"))?;

    let mut stderr_tail: Vec<String> = Vec::new();
    let mut exit_code: Option<i32> = None;
    let mut last_emit = Instant::now() - Duration::from_secs(1);

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    if let Some(rest) = line.trim().strip_prefix("out_time_us=") {
                        // Only known for a library item; a freshly picked
                        // file has no duration until we probe it, so this
                        // phase stays indeterminate for that case.
                        if let (Some(total), Ok(us)) = (duration, rest.trim().parse::<f64>()) {
                            if total > 0.0 && last_emit.elapsed() >= Duration::from_millis(200) {
                                last_emit = Instant::now();
                                let pct = ((us / 1_000_000.0) / total * 100.0).clamp(0.0, 99.0);
                                emit(app, &job_id, Some(pct), "Adding captions");
                            }
                        }
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
            CommandEvent::Terminated(payload) => exit_code = payload.code,
            CommandEvent::Error(err) => stderr_tail.push(err),
            _ => {}
        }
    }

    let produced_ok = std::fs::metadata(&output).map(|m| m.len() > 0).unwrap_or(false);
    let ok = match exit_code {
        Some(0) => true,
        Some(_) => false,
        None => produced_ok,
    };
    if !ok {
        let _ = std::fs::remove_file(&output);
        return Err(friendly_error(&stderr_tail.join("\n")));
    }

    emit(app, &job_id, Some(100.0), "Done");

    let meta = std::fs::metadata(&output).ok();
    let file_size = meta.map(|m| m.len() as i64);
    let file_path = output.to_string_lossy().into_owned();
    let filename = output
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_path.clone());

    db.insert_completed_local(
        &url,
        &format!("{base_title} (captioned)"),
        "Captioned",
        thumbnail.as_deref(),
        &filename,
        &file_path,
        &out_ext,
        false,
        duration,
        file_size,
    )
    .map_err(|e| format!("Added captions but could not save it to the library: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{caption_font_size, escape_for_filter, force_style};
    use std::path::Path;

    #[test]
    fn font_size_scales_with_resolution_and_has_sane_bounds() {
        assert_eq!(caption_font_size(Some("1080p")), 49);
        assert_eq!(caption_font_size(Some("720p")), 32);
        // No resolution (a freshly picked local file) falls back to 720p.
        assert_eq!(caption_font_size(None), 32);
        // An 8K source or a 144p one both stay on-screen and legible.
        assert!(caption_font_size(Some("4320p")) <= 72);
        assert!(caption_font_size(Some("144p")) >= 18);
    }

    #[test]
    fn every_style_id_produces_ass_color_fields() {
        // Regression guard for the AABBGGRR byte order: each preset must set
        // PrimaryColour, and the values must be well-formed &H hex, not an
        // accidental RRGGBB string that would silently render the wrong hue.
        for style in ["classic", "yellow", "boxed", "minimal", "unknown-id"] {
            let s = force_style(style, 32);
            assert!(s.contains("PrimaryColour=&H"), "{style}: {s}");
            let hex = s
                .split("PrimaryColour=&H")
                .nth(1)
                .and_then(|rest| rest.split(',').next())
                .unwrap();
            assert_eq!(hex.len(), 8, "{style}: PrimaryColour should be AABBGGRR: {hex}");
            assert!(hex.chars().all(|c| c.is_ascii_hexdigit()), "{style}: {hex}");
        }
    }

    #[test]
    fn unknown_style_falls_back_to_classic() {
        assert_eq!(force_style("nonexistent", 32), force_style("classic", 32));
        assert_eq!(force_style("", 32), force_style("classic", 32));
    }

    #[test]
    fn windows_path_survives_filter_escaping() {
        // The exact failure mode this guards: a bare drive-letter colon
        // collides with the subtitles filter's own option separator, so
        // "subtitles=C:\..." silently truncates at the colon. Confirmed by
        // running this escaping through the real FFmpeg binary on a tagged
        // test video and reading back a rendered frame.
        let escaped = escape_for_filter(Path::new(r"C:\Users\me\clip.srt"));
        assert_eq!(escaped, r"'C\:\\Users\\me\\clip.srt'");
        // Every literal backslash from the input must be doubled.
        assert_eq!(escaped.matches('\\').count(), 7);
    }
}
