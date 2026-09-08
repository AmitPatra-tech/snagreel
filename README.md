# Snagreel

A fast, clean, modern universal media downloader for Windows, macOS and Linux.
Built with **Tauri 2 + Rust** (backend) and **React + TypeScript + Vite + TailwindCSS** (frontend), powered by a bundled **yt-dlp** sidecar (with **FFmpeg** for merging/conversion).

## Status — Version 1.0 MVP (M1–M6 complete)

| Milestone | Deliverable | Status |
|---|---|---|
| M1 | Tauri + React + Rust scaffold, routing, layout, dark-first theme | ✅ |
| M2 | yt-dlp sidecar, URL validation, metadata retrieval | ✅ |
| M3 | Download engine, progress parsing, cancellation, queue basics | ✅ |
| M4 | SQLite integration, migrations, library persistence | ✅ |
| M5 | Downloads, Library and Settings pages | ✅ |
| M6 | Queue management with Tokio workers + configurable concurrency | ✅ |
| M7 | Notifications, auto-update, logging, polished error handling | ⬜ |
| M8 | Cross-platform packaging, installer, QA, V1 release | ⬜ |

## Features

- **Paste → preview → download**: metadata preview with quality (2160p…360p / best) and container (MP4/WEBM/MKV, MP3/M4A/WAV) selection
- **Audio-only extraction** via yt-dlp `-x` + FFmpeg
- **Playlist downloads** — entire playlist or selected videos
- **Queue system** with Tokio workers, configurable concurrency (1–10), drag-to-reorder, pause/resume, cancel, retry, remove
- **Live progress**: percent, speed, ETA streamed as Tauri events
- **Media library**: card/table views, search (title, filename, website, extension), sort, platform/type filters, open file / show in folder / delete (optionally with file)
- **Smart duplicate detection** by URL (Open existing / Download again / Cancel)
- **Download folder manager**: default folder + optional automatic per-platform subfolders (YouTube/, TikTok/, …)
- **Crash recovery**: interrupted downloads return to the queue on startup and resume with `--continue`

## Pro

Optional paid tier, kept deliberately download-adjacent so the core stays a clean, fast
downloader. Activated with a key (see below); everything above remains free.

- **Clip / cutout download** — grab only a section of a long video by start/end time,
  in the download dialog (yt-dlp `--download-sections`).
- **Transcribe to text** — turn speech in any local video/audio file into text and
  subtitles (`.txt` + `.srt`) in 90+ languages, fully offline. *Tools → Transcribe.*
  Accuracy is high but, as with all speech-to-text, not guaranteed perfect.
- **Extract audio** — pull the audio out of any local video into one or more formats
  (MP3/M4A/WAV/AAC/FLAC/OGG) in a single run. *Tools → Extract audio.*
- **Remove metadata** — strip embedded metadata (location, device info, timestamps,
  encoder tags) from a local video or audio file. *Tools → Remove metadata.*
- **Add captions** — transcribe a local video and burn the captions onto a new copy of
  it, with a choice of caption styles. *Tools → Add captions.*
- **Trim & convert** downloaded items — quick cut or container change. *Library → Edit.*

The speech-to-text engine (whisper.cpp) and its default model are bundled into the
installer automatically at build time — see "Bundled speech-to-text" below. No manual
setup is needed to use Transcribe or Add captions in a released build.

### Activation

The app ships only the **peppered SHA-256 hashes** of valid keys — never the keys
themselves — so a working key cannot be recovered from the binary. Entering a key hashes it
and checks membership against the embedded set (in Rust), then stores the accepted key's hash
(`settings.pro_key_hash`). Purchase flow: pay via **Dodo Payments** → email the payment
screenshot to the support address → receive a key → enter it in **Settings › Pro**. (Note: a
fully-offline check like this can be bypassed by patching the binary; a server-side check
would be the durable fix.)

## Development

### Prerequisites

- Node.js 18+, Rust (stable), and on Windows the MSVC build tools + WebView2
- Sidecar binaries in `src-tauri/binaries/` (not committed):
  - `yt-dlp-x86_64-pc-windows-msvc.exe` — from <https://github.com/yt-dlp/yt-dlp/releases>
  - `ffmpeg-x86_64-pc-windows-msvc.exe` — e.g. from <https://www.gyan.dev/ffmpeg/builds/> (`bin/ffmpeg.exe`, renamed)
  - On macOS/Linux use the matching target-triple suffix (e.g. `yt-dlp-aarch64-apple-darwin`)
- **Speech-to-text (Whisper), for local dev:** a packaged build fetches this
  automatically (see "Bundled speech-to-text" below), but `cargo build`/`cargo test`
  require the files to exist on disk even for a plain compile — `bundle.resources` in
  `tauri.conf.json` is checked eagerly, not just at final packaging. Get them the same
  way CI does:
  ```bash
  node scripts/fetch-whisper.mjs   # ~156 MB the first time; skips if already present
  ```
  This places `whisper-cli.exe` + its DLLs + `ggml-base.bin` in `src-tauri/binaries/`
  (searched there during `tauri dev`, and beside the executable when packaged). To use a
  different/larger model instead (e.g. `ggml-large-v3.bin`, for best accuracy — the most
  accurate model present is chosen automatically), download it from
  <https://huggingface.co/ggerganov/whisper.cpp> into `src-tauri/binaries/` yourself; the
  fetch script only ever adds a model when none is present, so it won't overwrite your
  choice.

### Run

```bash
npm install
npm run tauri dev     # dev app with hot reload
npm run tauri build   # production bundle/installer (M8)
```

### Project layout

```
src/                  React frontend
  components/         UI (shadcn-style) + app components (editor/transcribe/extract/activation dialogs)
  pages/              Home, Downloads, Library, Tools, Settings
  hooks/ services/ stores/ lib/ types/
src-tauri/
  src/commands/       Tauri IPC commands
  src/database/       SQLite (rusqlite) + migrations
  src/downloader/     yt-dlp args (incl. clip sections), metadata, progress parsing, error mapping
  src/queue/          Tokio scheduler, workers, cancellation
  src/editor/         FFmpeg trim/convert + audio extraction + metadata removal (Pro)
  src/transcribe/     FFmpeg → Whisper speech-to-text (Pro)
  src/captions/       Transcribe + FFmpeg subtitles filter → burned-in captions (Pro)
  src/activation/     Pro key hashing + verification
  src/filesystem/     folders, disk space, open/reveal/delete
  src/settings/       defaults
  binaries/           yt-dlp + ffmpeg sidecars + whisper-cli/DLLs/model (fetched, not committed)
scripts/
  fetch-whisper.mjs   downloads the speech-to-text engine + model into src-tauri/binaries/
  tauri-prebuild.mjs  beforeBuildCommand: runs fetch-whisper.mjs, then the frontend build
```

## Architecture notes

- **Events**: backend emits `download-progress`, `download-status`, `queue-updated`; the frontend feeds them into Zustand (progress) and TanStack Query invalidation (lists).
- **Queue**: a single scheduler task claims queued items (priority DESC, position ASC) whenever slots free up; each worker spawns the yt-dlp sidecar, parses `--progress-template` lines, and finalizes DB state. Pause kills the process and keeps the queue row; resume re-queues and yt-dlp continues from the `.part` file.
- **Database**: SQLite in the app data dir (`app.db`), WAL mode, `PRAGMA user_version` migrations; tables `downloads`, `settings`, `queue` per the PRD.
- **Security**: URLs are validated, sidecar args are passed as an argv array (never a shell string), and folder names are sanitized. Library operations resolve file paths by DB id; the Pro local-file Tools accept a path only from the OS file-picker the user chose. Pro keys are verified against embedded hashes and Pro commands re-check activation server-side (in Rust).

## Bundled speech-to-text

Transcribe and Add captions need a Whisper model, which — until 2026-09 — was an
optional, manually-installed component (see the retired "Optional — Pro transcription"
setup step in git history). That meant public builds shipped with the feature
advertised but non-functional out of the box.

Fixed by fetching it automatically: `scripts/fetch-whisper.mjs` downloads
`whisper-cli.exe` + its 4 required DLLs (from a **pinned** whisper.cpp release —
not "latest", so an upstream rename can't silently break a release build) and
`ggml-base.bin` (~148 MB, from Hugging Face) into `src-tauri/binaries/`, skipping
anything already present. `tauri.conf.json`'s `beforeBuildCommand` runs
`scripts/tauri-prebuild.mjs`, which runs that fetch and then the normal frontend
build — so `tauri build` always has the files before FFmpeg/Rust ever runs.

The files are wired in as `bundle.resources` (object form, mapped to bare
filenames — see `tauri.conf.json`), landing flat in the installed app's directory,
the same place `externalBin` already puts `yt-dlp.exe`/`ffmpeg.exe`. This is why
`whisper-cli.exe` and its DLLs end up as *siblings* of `snagreel.exe` — required,
since Windows' DLL loader searches the executable's own directory first, and
`transcribe/mod.rs`'s existing `find_whisper()`/`find_model()` already check
`current_exe().parent()` before anything else.

**Consequence worth knowing:** `bundle.resources` is validated *eagerly* by
`tauri_build::build()` — a plain `cargo build`/`cargo test` now fails if these
files are missing, not just `tauri build`. This was already true of `yt-dlp`/
`ffmpeg` via `externalBin` (verified by temporarily removing `ffmpeg-*.exe` and
reproducing the identical "resource path ... doesn't exist" error), so this isn't
new fragility, just the existing rule extended to a third component. Run
`node scripts/fetch-whisper.mjs` once after cloning.

Installer size grew accordingly (~48 MB → ~205 MB) — a deliberate tradeoff for
"the Pro tools work the first time," made explicitly after the alternative
(fetch the model on first use instead) was weighed and rejected.

## Planned next (M7–M8)

Native notifications, auto-updater, daily rotating logs with optional yt-dlp debug output, packaging/installers, sidecar self-update.
