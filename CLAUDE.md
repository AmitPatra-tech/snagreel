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

Current version: **1.1.0**.

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
