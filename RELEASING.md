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

1. In the source repo, bump the version in **all three** of
   `src-tauri/tauri.conf.json`, `package.json` and `src-tauri/Cargo.toml`, run
   `cargo test --lib` so `Cargo.lock` follows, and commit + push.
2. Go to the **`snagreel-releases`** repo → **Actions → Release Snagreel → Run
   workflow**, and enter the source ref to build (a tag, `main`, or a **full**
   commit SHA).
3. The workflow checks out the source, builds the signed installer,
   generates `latest.json`, and publishes a public GitHub Release.
4. Existing Snagreel installs will detect it on next launch and prompt to update.

> The updater endpoint always resolves to the **latest** release, so you only
> ever need to publish a newer version — no client changes required.

### From the CLI

```powershell
git push origin main
$sha = git rev-parse main
gh workflow run "Release Snagreel" --repo AmitPatra-tech/snagreel-releases -f ref=$sha
gh run list --repo AmitPatra-tech/snagreel-releases --workflow "Release Snagreel" --limit 1
gh run watch <run-id> --repo AmitPatra-tech/snagreel-releases --exit-status --interval 30
```

A clean build takes **~10 minutes**.

### Gotchas

- **The `ref` input must be a branch, a tag, or a *full* 40-character SHA.**
  `actions/checkout` does not resolve short SHAs: it expands whatever you give
  it into `+refs/heads/<ref>*:…` / `+refs/tags/<ref>*:…`, matches nothing, and
  fails after three fetch attempts (~41s) with only
  `The process '…git.exe' failed with exit code 1`. Use `git rev-parse main`,
  never the 7-character hash you see in `git log --oneline`.
- **The tag and release name come from the built source**, not from the input:
  the workflow uses `tagName: v__VERSION__`, which `tauri-action` fills from
  `tauri.conf.json`. Forgetting the version bump republishes the existing tag.
- `releaseDraft: false` — the release is **public the moment the job succeeds**.
- Always verify the endpoint clients actually poll, not just the release page:

  ```powershell
  Invoke-RestMethod "https://github.com/AmitPatra-tech/snagreel-releases/releases/latest/download/latest.json"
  ```

  `version` must be the new one and `platforms.windows-x86_64.signature` must be
  non-empty, or installed apps will not update.
- A `Node.js 20 is deprecated` annotation on the run is expected and harmless
  (`actions/checkout@v4`, `actions/setup-node@v4` are forced onto Node 24).

## Sidecars

The `yt-dlp` and `ffmpeg` binaries are not committed (see `.gitignore`). The
release workflow downloads them automatically before building. For local
development, place them in `src-tauri/binaries/` as described in the README.
