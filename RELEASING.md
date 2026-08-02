# Releasing Snagreel (auto-update)

Snagreel ships with the Tauri updater. Installed apps check a public manifest on
startup and prompt the user when a newer version exists.

## Repos

- **`snagreel`** (private) — the source code (this repo).
- **`snagreel-releases`** (public) — hosts installers + the update manifest
  (`latest.json`). The app's updater endpoint points here:
  `https://github.com/AmitPatra-tech/snagreel-releases/releases/latest/download/latest.json`

Keeping source private protects the code and the Pro activation scheme; releases
must be public so end users' apps can download updates.

## One-time setup

1. **Signing key** — already generated. The private key lives at
   `C:\Users\80939\.tauri\snagreel-updater.key` (NEVER commit it; keep a backup).
   The matching public key is baked into `src-tauri/tauri.conf.json`
   (`plugins.updater.pubkey`). If you lose the private key, updates can no longer
   be signed and existing installs will stop updating.

2. **Secrets on the `snagreel-releases` repo** (Settings → Secrets → Actions):
   - `TAURI_SIGNING_PRIVATE_KEY` — the full contents of the private key file above.
     (Set automatically during setup; re-set with
     `gh secret set TAURI_SIGNING_PRIVATE_KEY --repo AmitPatra-tech/snagreel-releases < "$HOME/.tauri/snagreel-updater.key"`.)
   - `SOURCE_TOKEN` — a GitHub **fine-grained PAT** with **read** access to the
     private `snagreel` repo (so the release workflow can check out the source).

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
