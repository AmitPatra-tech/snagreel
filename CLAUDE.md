# CLAUDE.md — Snagreel (All-in-One Download)

Desktop media downloader (Tauri + React/TypeScript + Rust). Ships as **Snagreel by
HutZon**. This file is project-specific context — do not confuse it with
`D:\Software\CLAUDE.md`, which is an unrelated guide for building HutZon marketing
product pages.

---

## Repos

- **`snagreel`** (this repo, currently **private**) — source code.
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

Current version: **1.0.3**.

## Sidecars

`yt-dlp` and `ffmpeg` binaries are gitignored, not committed. Release workflow
downloads them automatically. For local dev, place them in
`src-tauri/binaries/` (see README).

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
