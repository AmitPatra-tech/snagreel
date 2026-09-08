# CLAUDE.md — Snagreel (All-in-One Download)

Desktop media downloader (Tauri + React/TypeScript + Rust). Ships as **Snagreel by
HutZon**. This file is project-specific context — do not confuse it with
`D:\Software\CLAUDE.md`, which is an unrelated guide for building HutZon marketing
product pages.

---

## Repos

- **`snagreel`** (this repo, **public** since 2026-08-08) — source code.
  `https://github.com/AmitPatra-tech/snagreel`
- **`snagreel-releases`** (public) — hosts installers + the updater manifest
  (`latest.json`). Release workflow: `.github/workflows/release.yml` in that repo,
  triggered manually (Actions → Release Snagreel → Run workflow → source ref).
  See [RELEASING.md](RELEASING.md) for the full cut-a-release process.

## Versioning

Bump **all three** in lockstep when releasing, then run the release workflow in
`snagreel-releases`:
- `package.json` (`version`)
- `src-tauri/tauri.conf.json` (`version` — this is what the updater compares)
- `src-tauri/Cargo.toml` (`version`)

Then run `cargo test --lib` so `src-tauri/Cargo.lock` picks up the new version
and gets committed with them, and update "Current version" just below.

Current version: **1.3.1**.

Release in one go (see [RELEASING.md](RELEASING.md) for the gotchas):

```powershell
git push origin main
$sha = git rev-parse main          # MUST be the full 40 chars
gh workflow run "Release Snagreel" --repo AmitPatra-tech/snagreel-releases -f ref=$sha
gh run watch <run-id> --repo AmitPatra-tech/snagreel-releases --exit-status --interval 30
Invoke-RestMethod "https://github.com/AmitPatra-tech/snagreel-releases/releases/latest/download/latest.json"
```

## Sidecars

`yt-dlp` and `ffmpeg` binaries are gitignored, not committed. Release workflow
downloads them automatically. For local dev, place them in
`src-tauri/binaries/` (see README).

**The dev and installed binaries have different names**, which matters when
reproducing a bug:

| | dev tree | installed app |
|---|---|---|
| folder | `src-tauri/binaries/` | `C:\Users\80939\AppData\Local\Snagreel\` |
| names | `yt-dlp-x86_64-pc-windows-msvc.exe`, `ffmpeg-…msvc.exe` | `yt-dlp.exe`, `ffmpeg.exe` |

`sidecar_dir()` in `downloader/mod.rs` only passes `--ffmpeg-location` when
**`ffmpeg.exe`** exists in that folder, so in a dev tree it is never passed.
Point `--ffmpeg-location` at a folder holding a copy literally named
`ffmpeg.exe` when reproducing by hand, or yt-dlp downloads both streams and
silently skips the merge — leaving `.f<id>.mp4` + `.f<id>.m4a` and **exit code
0**. The explanation ("ffmpeg is not installed. The formats won't be merged")
is a *warning*, and the app passes `--no-warnings`, so nothing surfaces. Drop
`--no-warnings` first whenever a download "succeeds" with no output file.

## Debugging a failed download

The app stores only the *friendly* message, so start from its database rather
than from what the UI shows. Copy it first — the app holds a WAL lock.

```powershell
$dst = "$env:TEMP\snagreel-dbg"; New-Item -ItemType Directory -Force $dst | Out-Null
Copy-Item "$env:APPDATA\com.snagreel.app\app.db*" $dst -Force
python -c "import sqlite3;c=sqlite3.connect(r'$dst\app.db');[print(r) for r in c.execute('select id,status,length(title),substr(title,1,60),file_path,error,created_at from downloads order by id desc limit 8')];[print(r) for r in c.execute('select download_path,filename_template,organize_by_platform from settings where id=1')]"
```

`settings` gives the three inputs that decide the output path
(`download_path`, `filename_template`, `organize_by_platform`); `downloads.error`
on **older** rows often still holds the raw yt-dlp text, which is the real
evidence. There is no `sqlite3` CLI on this machine — `python` is on PATH.

Which build is actually running (the UI shows no version in a failure):

```powershell
Get-ItemProperty "HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*" |
  Where-Object DisplayName -like "*Snagreel*" | Select DisplayName,DisplayVersion,InstallLocation
```

Reproducing a path-length bug **requires matching the user's folder length** —
`D:\Saved Videos` is 15 characters, so test in a folder of the same length
(e.g. `D:\SnagreelTest`), not in a deep temp path where the budget maths differ
and the bug hides. For deterministic offline runs, feed a crafted title through
`--load-info-json` (write the JSON as UTF-8 **without BOM**, or yt-dlp dies on
`Unexpected UTF-8 BOM`):

```powershell
[System.IO.File]::WriteAllText("$p\i.json", $json, (New-Object System.Text.UTF8Encoding($false)))
& $yt --load-info-json "$p\i.json" --simulate --print filename -o "D:\SnagreelTest\%(title)s.%(ext)s" --windows-filenames
```

Facebook captions are **live** — the view/reaction counter changes between runs,
so the same URL yields a different title (and a different filename) minutes
apart. Never conclude "fixed" from one passing run; check whether the title that
passed contained a dot (see the filename-length section below).

## Video download format selection (fixed 2026-08-07)

`src-tauri/src/downloader/mod.rs` — `video_format_selector()`. Was previously
`bestvideo+bestaudio/best` forced into `--merge-output-format mp4`, which often
produced an MP4 carrying an **Opus** audio track. FFmpeg muxes that without
error, but Windows Media Player, Movies & TV, QuickTime and most phone players
can't decode Opus-in-MP4 — video plays with no sound, silently, no error
anywhere in the pipeline. Fixed by making the `-f` selector container-aware:
prefer AVC+AAC for mp4, VP9+Opus for webm, mkv stays unconstrained, all tiers
fall back to `bestvideo+bestaudio/best` so nothing fails to download. Covered by
unit tests in the same file.

## Output filename length (fixed 2026-08-13, properly fixed 2026-08-24)

`src-tauri/src/downloader/mod.rs` — `bounded_template()`. Facebook, Instagram
and TikTok return the whole post caption as `%(title)s`, so `%(title)s.%(ext)s`
produced paths past the Windows limits (255 per component, 260 total) and the
download died with `unable to open for writing: [Errno 22] Invalid argument`.
`--windows-filenames` does not help: it only removes illegal characters, and
nothing in yt-dlp shortens a name.

**`--trim-filenames` does not fix this on its own, and fails in a way that looks
random.** yt-dlp implements it as

```python
no_ext, *ext = filename.rsplit('.', 2)
filename = join_nonempty(no_ext[:trim_file_name], *ext, delim='.')
```

It splits the **whole rendered path** on the last two dots and truncates only
the part before them, then glues the rest back untouched. One dot anywhere in
the caption — `3.9M views`, `1.2M reactions`, `TalkyParrot 2.0` — leaves almost
nothing left of that split, so the truncation is a no-op. The same reel
downloads fine while the counter reads `4M views` and fails the moment it ticks
to `4.1M views`. 1.0.3 shipped with only this option and did not fix the bug.

The cap that holds is **output-template precision**: `bounded_template()`
rewrites `%(title)s` to `%(title).<N>s`, which truncates the field before any
path assembly, so no caption can defeat it. `--trim-filenames` is still passed
as a backstop for other unbounded fields. `N` is computed from `out_dir`, and
the budget is deliberately 200 rather than 260: yt-dlp counts characters while
Windows counts UTF-16 units, and these captions are full of emoji that cost two
apiece.

Verify a change here against a **real** caption with a dot in it — a synthetic
title without one takes the working path and proves nothing.

## Multi-link pastes (added 2026-08-24)

Paste up to **12** links separated by commas (newlines, tabs and spaces work
too — a valid URL cannot contain an unencoded space or comma, so splitting on
them never breaks one).

- `src/lib/urls.ts` — `parseUrls()`, the only place input is split. Returns
  `urls` / `invalid` / `overflow` so the UI can *say* what it skipped instead of
  silently downloading fewer than were pasted. De-duplicates.
- `src/components/BatchDialog.tsx` — shown for 2+ links. Two modes: **Choose per
  link** (the default; every row is pre-filled, so accepting all is still one
  click) and **Same for all**. A playlist link inside a batch expands to one
  download per entry, matching the single-link dialog. Links that failed to read
  are listed but cannot be ticked; links already in the library are ticked off by
  default.
- `src/components/UrlBar.tsx` — one link keeps the original single-link path
  untouched. Batches read metadata `METADATA_CONCURRENCY` (4) at a time: reading
  12 sequentially is a ~40s spinner, reading 12 at once spawns 12 yt-dlp
  processes and invites throttling.

**Two limits, both 12, deliberately kept equal but defined separately** —
`MAX_BATCH_LINKS` in `src/lib/urls.ts` (links per paste) and
`MAX_CONCURRENT_DOWNLOADS` in `src-tauri/src/models/mod.rs` (workers). The Rust
one is the real ceiling: `update_settings` and the queue scheduler both clamp to
it, so a hand-edited database cannot exceed it. `Settings.tsx` mirrors it in the
zod schema and the dropdown — change all three together.

Schema v5 moves existing installs from the old default of 3 to 12, but **only
where the stored value is still exactly 3**, so anyone who picked their own
number keeps it.

## Remove metadata (added 2026-09-02)

Pro tool on the Tools page: strips embedded metadata (title/author, GPS,
timestamps, encoder/software tags, chapter markers) from any video or audio
file by remuxing without re-encoding — `ffmpeg -map_metadata -1
-map_chapters -1 -c copy`, so quality is bit-for-bit unchanged and it runs in
roughly the time it takes to read the file.

- `src-tauri/src/editor/mod.rs` — `run_strip_metadata()`. Same `source_id` /
  `input_path` shape as `ExtractAudioRequest`/`TranscribeRequest`, so it works
  both on an already-downloaded library item and on a file picked from disk.
  Reuses `unique_output`/`insert_completed_local` from the trim/convert path.
- `src/components/StripMetadataDialog.tsx` — modeled on
  `ExtractAudioDialog.tsx`, no options to pick, just run/progress/done.
- Wired into `src/pages/Tools.tsx` only (not into `EditorDialog.tsx` — that
  dialog's `EditRequest` requires a library `source_id`, and this feature's
  primary use is a freshly picked local file, which the Tools page already
  handles for Transcribe/Extract audio the same way).

**What this does and does not do.** `-map_metadata -1` clears whatever
FFmpeg's demuxer surfaces as metadata (verified against a real MP4 tagged with
`title`/`comment`/a custom location field — all three gone after stripping,
and even the MP4 track's `handler_name` gets reset to FFmpeg's own generic
value rather than keeping an encoder-supplied string). It is **general**
privacy metadata removal — the same class of thing EXIF-scrubbing tools do for
photos — not a tool aimed at defeating any specific provenance/watermarking
scheme (C2PA manifests, invisible watermarks), and it was scoped that way on
purpose. Keep the UI copy and this doc describing it in those general terms.

## Add captions (added 2026-09-08)

Pro tool on the Tools page: transcribes the speech in a video, then burns the
generated captions onto a new copy of it — one video in, one captioned video
out, no separate subtitle file to manage.

- `src-tauri/src/captions/mod.rs` — `run_add_captions()`. Deliberately a thin
  second stage on top of the existing pipeline rather than a rewrite:
  - **Transcribe**: calls `transcribe::run_transcribe()` directly (same
    whisper model lookup, 16 kHz WAV extraction, progress) instead of
    duplicating it. This is also why it inherits that feature's requirement —
    the `whisper-cli` binary and a `ggml-*.bin` model must be present, exactly
    as documented in `transcribe/mod.rs`; neither ships in the dev tree.
  - **Burn**: a second FFmpeg pass with the `subtitles` filter over the `.srt`
    that step produced, `-c:v` re-encoded (burning changes pixels, so `-c
    copy` isn't an option) via the same `encode_args()` used by trim/convert.
  - Both stages emit on the *same* `transcribe-progress` event/`job_id`, so
    `AddCaptionsDialog.tsx` reuses `TranscribeProgress` and needs no new event
    type — it shows whichever stage is currently running.
  - Rejects audio-only input up front ("no video track") rather than letting
    FFmpeg fail confusingly on a file with nothing to draw text onto.

- Four caption-style presets (`force_style()`): Classic, Bold Yellow, Boxed,
  Minimal. **Two things here are easy to get subtly wrong and were verified
  against the shipped FFmpeg binary, not assumed:**
  1. **ASS colours are `&HAABBGGRR`** (alpha, blue, green, red) — backwards
     from the usual RRGGBB order. Confirmed by rendering an actual frame for
     each preset (`&H0000FFFF` → yellow) and reading it back as an image.
  2. **A bare Windows path breaks the `subtitles` filter.** `:` is the
     filter's own option separator, so a drive letter (`C:\...`) truncates the
     argument silently — first attempt at this failed with `Option not found:
     ''` from a plain `-replace ':', '\:'`; every backslash also needs
     doubling *before* the colon escape, or the count is wrong. The working
     form is `escape_for_filter()`: backslash → `\\`, colon → `\:`, wrapped in
     single quotes. Re-verify this specific mechanism (real Windows path,
     extracted frame) if this filter string is ever touched.
  `caption_font_size()` scales the text to a library item's known resolution;
  a freshly picked local file has none, so it falls back to a 720p-tuned size
  rather than probing the file just for this.

- `src/components/AddCaptionsDialog.tsx` — style buttons show a CSS
  approximation of each preset as a live preview; the real rendering is
  entirely server-side (FFmpeg), the CSS is only there so the picker isn't
  four identical unlabeled buttons.

- Wired into `src/pages/Tools.tsx` only, same reasoning as Remove metadata:
  `EditRequest` requires a library `source_id`, and picking a fresh local file
  is the primary use case here.

## Bundled speech-to-text (fixed 2026-09-08)

**1.3.0 shipped "Add captions" advertised as a Pro feature that could not
actually run** — Transcribe and Add captions both need a Whisper engine +
model, which was an optional, manually-installed component (never fetched by
the release workflow). A public user hit "Speech-to-text model not found" on
a completely stock install. This was a release-process gap, not a captions
bug — see `git log -- 'src-tauri/src/captions/'` for that feature's own
verification, which was thorough for the burn mechanism and just didn't
check whether the *engine* would exist for anyone but a dev with it manually
installed.

Fixed by bundling everything (the user's explicit choice — the alternative,
fetching the model on first use, was offered and rejected): the engine +
default model are now fetched automatically at build time and shipped in the
installer, so a fresh install works immediately.

- `scripts/fetch-whisper.mjs` — downloads `whisper-cli.exe` + 4 DLLs
  (`whisper.dll`, `ggml.dll`, `ggml-base.dll`, `ggml-cpu.dll` — **not**
  `SDL2.dll`, which the upstream zip also ships for the live-mic demos only;
  confirmed unnecessary by running real synthesized speech through
  whisper-cli with just these 5 files present) from a **pinned** whisper.cpp
  release tag, and `ggml-base.bin` (~148 MB) from Hugging Face, into
  `src-tauri/binaries/`. Skips anything already present, so a developer's own
  whisper-cli build or a bigger model is never overwritten. Fails the build
  loudly on any error — a silent skip here would recreate this exact bug.
- `scripts/tauri-prebuild.mjs` — `beforeBuildCommand` in `tauri.conf.json`.
  One Node script rather than a shell-chained command string
  (`"a && b"`); reading Tauri's own CLI source (`dev.rs`/`build.rs`,
  `run_hook`) confirmed the hook runs via `cmd /S /C` on Windows so `&&`
  would in fact work, but the script avoids depending on that.
- `tauri.conf.json` — `bundle.resources` (object form, source → bare
  destination filename, no subdirectory) for the 6 fetched files. **Read
  Tauri's own bundler source to confirm this, not assumed**: object-form
  `resources` do not preserve directory structure, and land in `$RESOURCES`,
  which on Windows is the same install root `externalBin` already uses —
  confirmed via `crates/tauri-bundler/.../nsis/mod.rs`'s
  `generate_resource_data()`, and consistent with the already-observed fact
  that `ffmpeg.exe`/`yt-dlp.exe` sit directly next to `snagreel.exe` in a
  real install. This placement is *required*, not cosmetic: Windows' DLL
  loader searches the launching executable's own directory first, and
  `transcribe/mod.rs`'s `find_whisper()` already checks
  `current_exe().parent()` before anything else — so the DLLs have to be
  siblings of `whisper-cli.exe`, which has to be a sibling of `snagreel.exe`.

**`beforeBuildCommand` runs with cwd = the frontend/repo root, not
`src-tauri/`** — confirmed by reading `dirs.frontend` in Tauri's own
`run_hook` call, after a relative-path guess (`../scripts/...`, assuming
`src-tauri/`-relative like `externalBin` paths) was wrong and caught before
shipping. `beforeDevCommand` behaves the same way, for the same reason.

**`bundle.resources` is validated eagerly by `tauri_build::build()`** — a
plain `cargo build`/`cargo test` now fails outright if these files are
missing, not just `tauri build`/bundling. Verified this is not new fragility:
temporarily removing `ffmpeg-x86_64-pc-windows-msvc.exe` reproduces the
*identical* `resource path ... doesn't exist` failure via `externalBin`, so
`yt-dlp`/`ffmpeg` already required this. Run `node scripts/fetch-whisper.mjs`
once after cloning, same as already placing yt-dlp/ffmpeg was always required.

**Verified for real, twice, at increasing levels of rigor:**
1. `whisper-cli.exe` + exactly those 4 DLLs + `ggml-base.bin`, invoked
   directly, correctly transcribed real Windows-TTS-synthesized speech
   word-for-word.
2. The actual `tauri dev` build, with these exact fetched files in place,
   driven through the real UI on a real test video (TTS speech muxed onto a
   synthetic video track): "Add captions" → Boxed style → produced a real
   output file, frame-extracted and read back as an image showing correctly
   burned, boxed captions. Caught and worked around a genuine hazard during
   this pass: **two Snagreel windows were running simultaneously** (the
   user's already-open installed v1.3.0 alongside the freshly-launched dev
   build), and a taskbar click landed on the wrong one at first, silently
   re-testing the *old, broken* build instead of the fix. Resolved by
   targeting the dev build's window by PID via `SetForegroundWindow`
   (deterministic) rather than by screen position (ambiguous whenever more
   than one instance is running) — recheck this if computer-use is ever
   used to test Snagreel again.

Installer size: **~48 MB → ~205 MB**. Known, deliberate tradeoff.

## Pro licensing architecture (rebuilt 2026-08-07/08)

**Do not reintroduce the old embedded-hash scheme described below as "the
current design" — it was replaced.** History for context:

- **v1.0.0–1.0.1 (old, retired):** `src-tauri/src/activation/mod.rs` embedded a
  pepper + the peppered SHA-256 hashes of all 48 valid Pro keys directly in the
  binary. Worked, but keys could never be revoked, one key could be shared
  indefinitely, and the hash list would be visible in a public repo.
- **Current (server-side):** a Cloudflare Worker (`licensing/`) is the source of
  truth. The desktop client carries only an **Ed25519 public key**
  (`LICENSE_PUBLIC_KEY` in `activation/mod.rs`) and exchanges an activation key
  for a short-lived signed token via `POST /v1/activate`. The token is bound to a
  per-install `device_id`, verified offline via signature on every `is_pro()`
  call (works without network), and renewed in the background once fewer than
  14 days remain (30-day token lifetime = offline grace period). A revoked key
  simply fails to renew and lapses at expiry rather than being instantly cut off.

### Where things live
- `licensing/` — the Worker source (`src/index.ts`), `wrangler.toml`,
  `schema.sql`, and its own `README.md` with full endpoint docs, deploy steps,
  and the "mint a key when an invoice arrives" curl command.
- Cloudflare D1 database `snagreel-licensing` (uuid
  `9593f3da-f69d-40a1-9a59-c1f0c32f7fde`) — `keys` and `activations` tables.
  Seeded with all 48 hashes from the old v1.0.0 key list (checksum-verified), so
  existing customer keys still activate.
- Ed25519 signing keypair: private key at
  `C:\Users\80939\.snagreel\licensing-signing.key` (**never commit; back it up —
  losing it means every installed app stops renewing**, since the matching
  public key is compiled into shipped binaries). Public key hex:
  `d3346ceed64ead90e28fdf3f00b339c694f34b1a7527c0d37e49cdf863f7c877`.

### Deployed endpoint
Live at **`https://snagreel-licensing.hutzonsnagreel.workers.dev`** (deployed
2026-08-08). `DEFAULT_LICENSE_URL` in `activation/mod.rs` points at it. Override
with the `SNAGREEL_LICENSE_URL` env var when testing against `wrangler dev`.
The workers.dev subdomain is `hutzonsnagreel` (plain `hutzon` was taken/invalid
at registration time). Cloudflare account id: `6a0edf50083315f2d919ae71b5acd415`.

### Existing Pro users on old installs
`ActivationState.needs_reactivation` + Settings → Pro UI
(`src/components/ProActivation.tsx`) detect a pre-1.1 `pro_key_hash` with no
`pro_token`, and prompt a one-time re-entry of their existing key (which still
works — seeded in D1) rather than silently downgrading them to Free.

### The deliberate limitation
None of this stops someone who **rebuilds from source** with the licence check
removed — no offline-capable check can prevent that. It only raises the floor
for casual key-sharing (revocation, per-key seat limits, no key material in the
repo). This was an informed tradeoff discussed explicitly with the user before
building it — see conversation history if the reasoning needs to be
re-justified.

### Repo visibility
Repo went **public** on 2026-08-08 (`https://github.com/AmitPatra-tech/snagreel`),
after the licensing rewrite removed all key material from the source. Git history
was audited for secrets first and was clean. Consequence the user accepted
knowingly: anyone can rebuild without the licence check. Never commit the signing
key, `licensing/.dev.vars`, or the ADMIN_TOKEN — this repo is now world-readable.

Side effect: `SOURCE_TOKEN` is no longer needed on `snagreel-releases`, because
`actions/checkout` can read a public repo without a PAT.

## Displayed version

The sidebar and Settings → About read the version via `useAppVersion()`
(`src/hooks/useAppVersion.ts`, backed by Tauri's `getVersion()`). Do not hardcode
version strings in the UI — they previously drifted and shipped "v1.0.0" on a
1.0.1 build.
