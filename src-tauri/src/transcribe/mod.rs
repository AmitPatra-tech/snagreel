//! Pro speech-to-text: transcribe an existing library item (audio or video)
//! into text using an offline Whisper engine.
//!
//! Pipeline: FFmpeg down-mixes the source to 16 kHz mono WAV → the bundled
//! `whisper-cli` sidecar transcribes it with the bundled model → the resulting
//! `.txt`/`.srt` are written next to the source file. Progress is streamed via
//! `transcribe-progress` events keyed by a caller-supplied `job_id`.
//!
//! Like every automatic speech-to-text system, accuracy is very high but not
//! guaranteed to be perfect; it depends on audio clarity, accents and noise.
//!
//! Requires a `whisper-cli` binary (declared as an external sidecar) and a
//! Whisper model file (`ggml-*.bin`) placed next to the app, exactly like the
//! bundled `yt-dlp` and `ffmpeg` binaries.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::ShellExt;

use crate::database::Db;
use crate::models::{TranscribeProgress, TranscribeRequest, TranscribeResult};

/// Directories to search for the Whisper binary and model file. Covers both a
/// packaged install (next to the executable / resources) and `tauri dev` (the
/// `binaries/` folder used for the other sidecars).
fn search_dirs(app: &AppHandle) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.to_path_buf());
        }
    }
    if let Ok(res) = app.path().resource_dir() {
        dirs.push(res);
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("binaries"));
        dirs.push(cwd);
    }
    dirs
}

/// Locate the whisper CLI binary next to the app (optional component).
fn find_whisper(app: &AppHandle) -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["whisper-cli.exe", "whisper.exe", "main.exe"]
    } else {
        &["whisper-cli", "whisper", "main"]
    };
    for dir in search_dirs(app) {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Locate a Whisper model, preferring larger/more-accurate variants.
fn find_model(app: &AppHandle) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for dir in search_dirs(app) {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                if name.starts_with("ggml-") && name.ends_with(".bin") {
                    candidates.push(path);
                }
            }
        }
    }
    // Prefer the most accurate model when several are present.
    fn rank(name: &str) -> u8 {
        let n = name.to_ascii_lowercase();
        if n.contains("large") {
            5
        } else if n.contains("medium") {
            4
        } else if n.contains("small") {
            3
        } else if n.contains("base") {
            2
        } else {
            1
        }
    }
    candidates.sort_by_key(|p| {
        std::cmp::Reverse(rank(&p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))
    });
    candidates.into_iter().next()
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

/// Parse a whisper `progress = NN%` line into a percentage.
fn parse_whisper_progress(line: &str) -> Option<f64> {
    let idx = line.find("progress")?;
    let rest = &line[idx..];
    let eq = rest.find('=')?;
    let after = &rest[eq + 1..];
    let pct = after.find('%')?;
    after[..pct].trim().parse::<f64>().ok()
}

pub async fn run_transcribe(
    app: &AppHandle,
    req: TranscribeRequest,
    job_id: String,
) -> Result<TranscribeResult, String> {
    let db = app.state::<Db>();
    if !db.is_pro() {
        return Err("Transcription is a Pro feature. Activate Pro to unlock it.".into());
    }

    // Resolve the input: an explicit local file wins, otherwise a library item.
    let input = if let Some(p) = req.input_path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        p.to_string()
    } else if let Some(id) = req.source_id {
        db.get_download(id)
            .map_err(|_| "Could not find the item to transcribe.".to_string())?
            .file_path
            .ok_or_else(|| "This item has no file on disk to transcribe.".to_string())?
    } else {
        return Err("No file was provided to transcribe.".into());
    };
    if !Path::new(&input).is_file() {
        return Err("The source file could not be found on disk.".into());
    }

    let model = find_model(app).ok_or_else(|| {
        "Speech-to-text model not found. Download a Whisper model file (e.g. ggml-base.bin or \
         ggml-large-v3.bin) and place it — together with the whisper-cli binary — in the app's \
         binaries folder (src-tauri/binaries during development, or next to the app once \
         installed). See the README (\"Pro transcription\") for links."
            .to_string()
    })?;

    // 1) Extract 16 kHz mono WAV (Whisper's expected input) via FFmpeg.
    emit(app, &job_id, Some(0.0), "Preparing audio");
    let cache = app
        .path()
        .app_cache_dir()
        .map_err(|_| "Could not resolve the cache directory.".to_string())?;
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let wav = cache.join(format!("transcribe-{stamp}.wav"));
    let _ = std::fs::remove_file(&wav);

    let ff = app
        .shell()
        .sidecar("ffmpeg")
        .map_err(|_| "FFmpeg is missing. Reinstall the app to restore it.".to_string())?
        .args([
            "-y",
            "-i",
            &input,
            "-ar",
            "16000",
            "-ac",
            "1",
            "-c:a",
            "pcm_s16le",
            &wav.to_string_lossy(),
        ]);
    let ff_out = ff
        .output()
        .await
        .map_err(|e| format!("Could not start FFmpeg: {e}"))?;
    if !ff_out.status.success() {
        let _ = std::fs::remove_file(&wav);
        return Err("Could not read audio from this file.".into());
    }

    // 2) Transcribe with whisper-cli, writing <source>.txt and <source>.srt.
    emit(app, &job_id, Some(0.0), "Transcribing");
    let out_base = {
        let p = Path::new(&input);
        let dir = p.parent().unwrap_or_else(|| Path::new("."));
        let stem = p
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "transcript".into());
        dir.join(stem)
    };
    let lang = if req.language.trim().is_empty() {
        "auto".to_string()
    } else {
        req.language.trim().to_string()
    };

    let whisper_bin = find_whisper(app).ok_or_else(|| {
        "Speech-to-text engine not found. Place the whisper-cli binary next to the app."
            .to_string()
    })?;

    let args: Vec<String> = vec![
        "-m".into(),
        model.to_string_lossy().into_owned(),
        "-f".into(),
        wav.to_string_lossy().into_owned(),
        "-l".into(),
        lang,
        "-otxt".into(),
        "-osrt".into(),
        "-of".into(),
        out_base.to_string_lossy().into_owned(),
        "--print-progress".into(),
    ];

    // Run the (optional) whisper binary as a plain child process and stream its
    // stderr for progress. Done in a blocking task so we can read line-by-line.
    let app_clone = app.clone();
    let job = job_id.clone();
    let (exit_code, stderr_tail): (Option<i32>, Vec<String>) =
        tauri::async_runtime::spawn_blocking(move || -> Result<(Option<i32>, Vec<String>), String> {
            use std::io::{BufRead, BufReader};
            use std::process::{Command, Stdio};

            let mut cmd = Command::new(&whisper_bin);
            cmd.args(&args).stdout(Stdio::null()).stderr(Stdio::piped());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
            }

            let mut child = cmd
                .spawn()
                .map_err(|e| format!("Could not start the speech-to-text engine: {e}"))?;

            let mut tail: Vec<String> = Vec::new();
            if let Some(err) = child.stderr.take() {
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    if let Some(pct) = parse_whisper_progress(&line) {
                        emit(&app_clone, &job, Some(pct.clamp(0.0, 99.0)), "Transcribing");
                    } else if !line.trim().is_empty() {
                        tail.push(line.trim().to_string());
                        if tail.len() > 20 {
                            tail.remove(0);
                        }
                    }
                }
            }
            let status = child.wait().map_err(|e| e.to_string())?;
            Ok((status.code(), tail))
        })
        .await
        .map_err(|e| format!("Transcription task failed: {e}"))??;

    let _ = std::fs::remove_file(&wav);

    if exit_code != Some(0) {
        return Err(format!(
            "Transcription failed. {}",
            stderr_tail.last().cloned().unwrap_or_default()
        )
        .trim()
        .to_string());
    }

    let txt_path = format!("{}.txt", out_base.to_string_lossy());
    let srt_path = format!("{}.srt", out_base.to_string_lossy());
    let text = std::fs::read_to_string(&txt_path)
        .map_err(|_| "Transcription finished but the text file was not found.".to_string())?
        .trim()
        .to_string();

    emit(app, &job_id, Some(100.0), "Done");
    Ok(TranscribeResult {
        text,
        txt_path,
        srt_path,
    })
}
