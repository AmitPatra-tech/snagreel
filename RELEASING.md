# Releasing Snagreel (auto-update)

Snagreel ships with the Tauri updater. Installed apps check a public manifest on
startup and prompt the user when a newer version exists.

## Repos

- **`snagreel`** (public) — the source code (this repo).
- **`snagreel-releases`** (public) — hosts installers + the update manifest
  (`latest.json`). The app's updater endpoint points here:
  `https://github.com/AmitPatra-tech/snagreel-releases/releases/latest/download/latest.json`

The source is public: activation keys are no longer embedded in it (the Pro check
lives in the `licensing/` Worker), so there is nothing in the repo to protect.
Releases are public so end users' apps can download updates.

## One-time setup

1. **Signing key** — already generated. The private key lives at
   `C:\Users\80939\.tauri\snagreel-updater.key` (NEVER commit it; keep a backup).
   The matching public key is baked into `src-tauri/tauri.conf.json`
   (`plugins.updater.pubkey`). If you lose the private key, updates can no longer
   be signed and existing installs will stop updating.

2. **Secrets on the `snagreel-releases` repo** (Settings → Secrets → Actions):
   - `TAURI_SIGNING_PRIVATE_KEY` — the full contents of the private key file above.
     ✅ Already set. Re-set with
     `gh secret set TAURI_SIGNING_PRIVATE_KEY --repo AmitPatra-tech/snagreel-releases < "$HOME/.tauri/snagreel-updater.key"`.
   - `SOURCE_TOKEN` — **no longer needed.** It existed so the workflow could check
     out a private source repo; `snagreel` is public now, so `actions/checkout`
     reads it without any token.

3. **Add the release workflow to `snagreel-releases`.** The workflow file is versioned
   here at [`docs/github-release-workflow.yml`](docs/github-release-workflow.yml). It
   could not be pushed automatically (the CLI token lacks the `workflow` scope), so add
   it once, either way:
   - **Web UI:** in `snagreel-releases`, *Add file → Create new file*, name it
     `.github/workflows/release.yml`, paste the contents, commit. **or**
   - **CLI:** `gh auth refresh -h github.com -s workflow`, then copy the file into the
     releases repo under `.github/workflows/release.yml` and push.

## Cutting a new version (e.g. v2)

1. In the private repo, bump the version in **`src-tauri/tauri.conf.json`**
   (`"version"`) and in `package.json`. Commit and push.
2. Go to the **`snagreel-releases`** repo → **Actions → Release Snagreel → Run
   workflow**, and enter the source ref to build (a tag or `main`).
3. The workflow checks out the private source, builds the signed installer,
   generates `latest.json`, and publishes a public GitHub Release.
4. Existing Snagreel installs will detect it on next launch and prompt to update.

> The updater endpoint always resolves to the **latest** release, so you only
> ever need to publish a newer version — no client changes required.

## Sidecars

The `yt-dlp` and `ffmpeg` binaries are not committed (see `.gitignore`). The
release workflow downloads them automatically before building. For local
development, place them in `src-tauri/binaries/` as described in the README.
