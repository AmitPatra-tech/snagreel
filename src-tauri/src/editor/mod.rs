//! Pro media editing built on the bundled FFmpeg sidecar.
//!
//! One [`EditRequest`] runs a single, well-defined operation on an existing
//! library item and writes the result next to the source, then registers it as
//! a new completed library entry. Progress is streamed via `edit-progress`
//! events keyed by a caller-supplied `job_id`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

use crate::database::Db;
use crate::models::{Download, EditProgress, EditRequest, ExtractAudioRequest};

const AUDIO_EXTS: &[&str] = &["mp3", "m4a", "wav", "opus", "ogg", "flac", "aac"];

fn is_audio_ext(ext: &str) -> bool {
    AUDIO_EXTS.contains(&ext.to_ascii_lowercase().as_str())
}

fn ext_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn stem_of(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".into())
}

/// Encoder arguments for a target container/extension.
fn encode_args(ext: &str) -> Vec<String> {
    let s = |v: &str| v.to_string();
    match ext.to_ascii_lowercase().as_str() {
        "mp4" | "mkv" | "mov" => vec![
            s("-c:v"), s("libx264"), s("-preset"), s("veryfast"), s("-crf"), s("23"),
            s("-c:a"), s("aac"), s("-b:a"), s("192k"),
        ],
        "webm" => vec![
            s("-c:v"), s("libvpx-vp9"), s("-b:v"), s("0"), s("-crf"), s("32"),
            s("-c:a"), s("libopus"),
        ],
        "mp3" => vec![s("-c:a"), s("libmp3lame"), s("-q:a"), s("2")],
        "m4a" | "aac" => vec![s("-c:a"), s("aac"), s("-b:a"), s("192k")],
        "wav" => vec![s("-c:a"), s("pcm_s16le")],
        "opus" | "ogg" => vec![s("-c:a"), s("libopus")],
        "flac" => vec![s("-c:a"), s("flac")],
        _ => vec![s("-c:v"), s("libx264"), s("-c:a"), s("aac")],
    }
}

fn audio_encode_args(ext: &str) -> Vec<String> {
    encode_args(if is_audio_ext(ext) { ext } else { "mp3" })
}

/// Turn FFmpeg stderr into something a user can read.
fn friendly_error(stderr: &str) -> String {
    let lower = stderr.to_lowercase();
    if lower.contains("no space left") {
        return "Not enough disk space to save the edited file.".into();
    }
    if lower.contains("permission denied") {
        return "Permission denied writing the edited file.".into();
    }
    if lower.contains("no such file") || lower.contains("does not exist") {
        return "The source file could not be found.".into();
    }
    stderr
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| "Editing failed.".into())
}

/// A validated plan: what FFmpeg should do and how to name/classify the result.
struct EditPlan {
    /// FFmpeg args *after* the global flags, i.e. everything from `-ss`/`-i`
    /// through the output path.
    args: Vec<String>,
    output_path: PathBuf,
    output_ext: String,
    /// Expected output duration in seconds, for progress (None = indeterminate).
    total_secs: Option<f64>,
    suffix: String,
}

fn round2(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract().abs() < f64::EPSILON {
        format!("{}", r as i64)
    } else {
        format!("{r:.2}")
    }
}

/// Pick a non-colliding output path in `dir` for `<stem> (<suffix>).<ext>`.
fn unique_output(dir: &Path, stem: &str, suffix: &str, ext: &str) -> PathBuf {
    let base = if suffix.is_empty() {
        format!("{stem} (edited)")
    } else {
        format!("{stem} ({suffix})")
    };
    let mut candidate = dir.join(format!("{base}.{ext}"));
    let mut n = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{base} {n}.{ext}"));
        n += 1;
    }
    candidate
}

fn build_plan(req: &EditRequest, source: &Download) -> Result<EditPlan, String> {
    let input = source
        .file_path
        .as_deref()
        .ok_or_else(|| "This item has no file on disk to edit.".to_string())?;
    if !Path::new(input).is_file() {
        return Err("The source file no longer exists on disk.".into());
    }
    let dir = Path::new(input)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = stem_of(input);
    let in_ext = ext_of(input);
    let src_duration = source.duration.filter(|d| *d > 0.0);

    let s = |v: &str| v.to_string();
    let out_fmt = req
        .output_format
        .as_deref()
        .map(|f| f.trim().to_ascii_lowercase())
        .filter(|f| !f.is_empty());

    // (output_ext, suffix, ffmpeg args, total_secs)
    let (output_ext, suffix, args, total_secs): (String, String, Vec<String>, Option<f64>) =
        match req.operation.as_str() {
            "trim" => {
                let start = req.start.unwrap_or(0.0).max(0.0);
                let end = req.end;
                if let Some(e) = end {
                    if e <= start {
                        return Err("End time must be after start time.".into());
                    }
                }
                let ext = out_fmt.clone().unwrap_or_else(|| in_ext.clone());
                let dur = end.map(|e| e - start);
                let placeholder = dir.join(format!("__aio_out.{ext}"));
                let mut a = vec![s("-ss"), round2(start), s("-i"), s(input)];
                if let Some(d) = dur {
                    a.push(s("-t"));
                    a.push(round2(d));
                }
                if ext == in_ext {
                    a.push(s("-c"));
                    a.push(s("copy"));
                } else {
                    a.extend(encode_args(&ext));
                }
                a.push(placeholder.to_string_lossy().into_owned());
                (ext, "clip".into(), a, dur.or(src_duration))
            }
            "convert" => {
                let ext = out_fmt.ok_or("Choose a format to convert to.")?;
                let placeholder = dir.join(format!("__aio_out.{ext}"));
                let mut a = vec![s("-i"), s(input)];
                if is_audio_ext(&ext) {
                    a.push(s("-vn"));
                    a.extend(audio_encode_args(&ext));
                } else {
                    a.extend(encode_args(&ext));
                }
                a.push(placeholder.to_string_lossy().into_owned());
                (ext.clone(), format!("to {}", ext.to_uppercase()), a, src_duration)
            }
            other => return Err(format!("Unknown edit operation: {other}")),
        };

    let output_path = unique_output(&dir, &stem, &suffix, &output_ext);
    // Swap the placeholder output arg (always the last one) for the real path.
    let mut args = args;
    if let Some(last) = args.last_mut() {
        *last = output_path.to_string_lossy().into_owned();
    }

    Ok(EditPlan {
        args,
        output_path,
        output_ext,
        total_secs,
        suffix,
    })
}

fn emit_progress(app: &AppHandle, job_id: &str, percent: Option<f64>) {
    let _ = app.emit(
        "edit-progress",
        EditProgress {
            job_id: job_id.to_string(),
            percent,
        },
    );
}

/// Run a Pro edit. Returns the newly created library item on success.
pub async fn run_edit(
    app: &AppHandle,
    req: EditRequest,
    job_id: String,
) -> Result<Download, String> {
    let db = app.state::<Db>();
    if !db.is_pro() {
        return Err("Editing is a Pro feature. Activate Pro to unlock it.".into());
    }

    let source = db
        .get_download(req.source_id)
        .map_err(|_| "Could not find the item to edit.".to_string())?;

    let plan = build_plan(&req, &source)?;

    let mut global: Vec<String> = vec![
        "-y".into(),
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
    ];
    global.extend(plan.args.iter().cloned());

    let command = app
        .shell()
        .sidecar("ffmpeg")
        .map_err(|_| "FFmpeg is missing. Reinstall the app to restore it.".to_string())?
        .args(global);

    let (mut rx, _child) = command
        .spawn()
        .map_err(|e| format!("Could not start FFmpeg: {e}"))?;

    emit_progress(app, &job_id, Some(0.0));

    let mut stderr_tail: Vec<String> = Vec::new();
    let mut exit_code: Option<i32> = None;
    let mut last_emit = Instant::now() - Duration::from_secs(1);

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    if let Some(rest) = line.trim().strip_prefix("out_time_us=") {
                        if let (Some(total), Ok(us)) = (plan.total_secs, rest.trim().parse::<f64>()) {
                            if total > 0.0 && last_emit.elapsed() >= Duration::from_millis(200) {
                                last_emit = Instant::now();
                                let pct = ((us / 1_000_000.0) / total * 100.0).clamp(0.0, 99.0);
                                emit_progress(app, &job_id, Some(pct));
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

    // A very fast `-c copy` can end before a Terminated code is observed; in
    // that case treat a non-empty output file as success.
    let produced_ok = std::fs::metadata(&plan.output_path)
        .map(|m| m.len() > 0)
        .unwrap_or(false);
    let ok = match exit_code {
        Some(0) => true,
        Some(_) => false,
        None => produced_ok,
    };
    if !ok {
        let _ = std::fs::remove_file(&plan.output_path);
        return Err(friendly_error(&stderr_tail.join("\n")));
    }

    let meta = std::fs::metadata(&plan.output_path).ok();
    let file_size = meta.map(|m| m.len() as i64);
    let file_path = plan.output_path.to_string_lossy().into_owned();
    let filename = plan
        .output_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_path.clone());
    let title = format!("{} ({})", source.title, plan.suffix);
    let audio_only = is_audio_ext(&plan.output_ext);

    let created = db
        .insert_completed_local(
            &source.url,
            &title,
            "Edited",
            source.thumbnail.as_deref(),
            &filename,
            &file_path,
            &plan.output_ext,
            audio_only,
            plan.total_secs.or(source.duration),
            file_size,
        )
        .map_err(|e| format!("Edited the file but could not save it to the library: {e}"))?;

    emit_progress(app, &job_id, Some(100.0));
    Ok(created)
}

/// Pick a non-colliding `<stem>.<ext>` (then `<stem> (2).<ext>`, …) in `dir`.
fn unique_simple(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut candidate = dir.join(format!("{stem}.{ext}"));
    let mut n = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{stem} ({n}).{ext}"));
        n += 1;
    }
    candidate
}

fn emit_extract(app: &AppHandle, job_id: &str, percent: Option<f64>) {
    let _ = app.emit(
        "extract-progress",
        EditProgress {
            job_id: job_id.to_string(),
            percent,
        },
    );
}

/// Pro: extract the audio track from a video (library item or local file) into
/// one or more audio formats. Returns the created library entries.
pub async fn run_extract_audio(
    app: &AppHandle,
    req: ExtractAudioRequest,
    job_id: String,
) -> Result<Vec<Download>, String> {
    let db = app.state::<Db>();
    if !db.is_pro() {
        return Err("Audio extraction is a Pro feature. Activate Pro to unlock it.".into());
    }

    // Resolve the input and any metadata to carry onto the results.
    let (input, thumbnail, duration, base_title, url): (
        String,
        Option<String>,
        Option<f64>,
        String,
        String,
    ) = if let Some(p) = req.input_path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        (p.to_string(), None, None, stem_of(p), format!("file:///{p}"))
    } else if let Some(id) = req.source_id {
        let s = db
            .get_download(id)
            .map_err(|_| "Could not find the item to extract from.".to_string())?;
        let path = s
            .file_path
            .clone()
            .ok_or_else(|| "This item has no file on disk.".to_string())?;
        (path, s.thumbnail.clone(), s.duration, s.title.clone(), s.url.clone())
    } else {
        return Err("No file was provided.".into());
    };

    if !Path::new(&input).is_file() {
        return Err("The source file could not be found on disk.".into());
    }

    // Validate the requested formats.
    let formats: Vec<String> = req
        .formats
        .iter()
        .map(|f| f.trim().to_ascii_lowercase())
        .filter(|f| is_audio_ext(f))
        .collect();
    if formats.is_empty() {
        return Err("Choose at least one audio format to extract.".into());
    }

    // Save extracted audio into the app's configured download folder (not next
    // to the picked source, which may live anywhere on disk).
    let dir = {
        let settings = db.get_settings().map_err(|e| e.to_string())?;
        let d = PathBuf::from(&settings.download_path);
        std::fs::create_dir_all(&d)
            .map_err(|e| format!("Could not create the download folder: {e}"))?;
        d
    };
    let stem = stem_of(&input);
    let total = formats.len();

    let mut created: Vec<Download> = Vec::new();
    let mut last_error: Option<String> = None;

    for (i, fmt) in formats.iter().enumerate() {
        emit_extract(app, &job_id, Some((i as f64 / total as f64) * 100.0));

        let output = unique_simple(&dir, &stem, fmt);
        let mut args: Vec<String> = vec![
            "-y".into(),
            "-hide_banner".into(),
            "-loglevel".into(),
            "error".into(),
            "-i".into(),
            input.clone(),
            "-vn".into(),
        ];
        args.extend(audio_encode_args(fmt));
        args.push(output.to_string_lossy().into_owned());

        let command = app
            .shell()
            .sidecar("ffmpeg")
            .map_err(|_| "FFmpeg is missing. Reinstall the app to restore it.".to_string())?
            .args(args);

        let result = command.output().await;
        match result {
            Ok(out) if out.status.success() => {
                let meta = std::fs::metadata(&output).ok();
                let file_size = meta.map(|m| m.len() as i64);
                let file_path = output.to_string_lossy().into_owned();
                let filename = output
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| file_path.clone());
                match db.insert_completed_local(
                    &url,
                    &format!("{base_title} ({})", fmt.to_uppercase()),
                    "Extracted",
                    thumbnail.as_deref(),
                    &filename,
                    &file_path,
                    fmt,
                    true,
                    duration,
                    file_size,
                ) {
                    Ok(d) => created.push(d),
                    Err(e) => last_error = Some(e.to_string()),
                }
            }
            Ok(out) => {
                let _ = std::fs::remove_file(&output);
                last_error = Some(friendly_error(&String::from_utf8_lossy(&out.stderr)));
            }
            Err(e) => {
                let _ = std::fs::remove_file(&output);
                last_error = Some(format!("Could not start FFmpeg: {e}"));
            }
        }
    }

    emit_extract(app, &job_id, Some(100.0));

    if created.is_empty() {
        Err(last_error.unwrap_or_else(|| "Audio extraction failed.".into()))
    } else {
        Ok(created)
    }
}
