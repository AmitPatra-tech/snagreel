#!/usr/bin/env node
// Fetches the offline speech-to-text engine (whisper.cpp) and its default
// model into src-tauri/binaries/, so "Transcribe to text" and "Add captions"
// work out of the box in a *packaged build* instead of requiring the manual
// setup described in the README. Only wired into `beforeBuildCommand`
// (production installer builds) — `tauri dev` is untouched, since a
// developer working on unrelated features shouldn't eat a ~150 MB download
// just to start the app. See CLAUDE.md "Bundled speech-to-text".
//
// Skips anything already present, so a developer (or a future CI run that
// has cached `src-tauri/binaries/`) who already has a whisper-cli build or a
// different model — e.g. large-v3, for better accuracy — is never
// overwritten.
//
// On any failure this exits non-zero and fails the build loudly. A silent
// skip here would recreate exactly the bug this script exists to fix: a
// release that ships without a working speech-to-text engine.

import { existsSync, mkdirSync, readdirSync, copyFileSync, rmSync, statSync } from "node:fs";
import { writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.dirname(fileURLToPath(import.meta.url));
const BINARIES = path.join(ROOT, "..", "src-tauri", "binaries");
const TMP = path.join(ROOT, "..", ".whisper-fetch-tmp");

// Pinned to a specific release tag, not "latest": a future whisper.cpp
// release could rename or drop this asset, and a broken release build is
// worse than a stale-but-working engine. Bump deliberately, and re-verify
// the asset name and its contents (a Windows path escaping this exact zip
// layout was hand-verified when this was written — see captions/mod.rs).
const WHISPER_RELEASE =
  "https://github.com/ggml-org/whisper.cpp/releases/download/v1.8.0/whisper-bin-x64.zip";
const MODEL_URL = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";

// Exactly what whisper-cli.exe needs to run — verified by running it end to
// end on real synthesized speech with only these five files present.
// whisper-bin-x64.zip also ships SDL2.dll and several demo executables
// (stream.exe, command.exe, wchess.exe, ...) for live-microphone use cases;
// none of that is needed for transcribing a file, so it's left behind.
const NEEDED_DLLS = ["whisper.dll", "ggml.dll", "ggml-base.dll", "ggml-cpu.dll"];
const MIN_BYTES = {
  "whisper-cli.exe": 100_000,
  ...Object.fromEntries(NEEDED_DLLS.map((d) => [d, 10_000])),
};

function alreadyHasEngine() {
  return ["whisper-cli.exe", "whisper.exe", "main.exe"].some((n) =>
    existsSync(path.join(BINARIES, n)),
  );
}

function alreadyHasModel() {
  if (!existsSync(BINARIES)) return false;
  return readdirSync(BINARIES).some((f) => /^ggml-.*\.bin$/i.test(f));
}

async function download(url, dest) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url} -> HTTP ${res.status}`);
  const buf = Buffer.from(await res.arrayBuffer());
  mkdirSync(path.dirname(dest), { recursive: true });
  await writeFile(dest, buf);
  return buf.length;
}

async function fetchEngine() {
  if (alreadyHasEngine()) {
    console.log("[fetch-whisper] engine already present, skipping");
    return;
  }
  console.log("[fetch-whisper] downloading whisper.cpp engine...");
  mkdirSync(TMP, { recursive: true });
  const zipPath = path.join(TMP, "whisper.zip");
  const zipSize = await download(WHISPER_RELEASE, zipPath);
  if (zipSize < 1_000_000) {
    throw new Error(`whisper.cpp zip implausibly small (${zipSize} bytes)`);
  }

  const extractDir = path.join(TMP, "extracted");
  execFileSync("powershell", [
    "-NoProfile",
    "-Command",
    `Expand-Archive -Path '${zipPath}' -DestinationPath '${extractDir}' -Force`,
  ]);

  const releaseDir = path.join(extractDir, "Release");
  for (const file of ["whisper-cli.exe", ...NEEDED_DLLS]) {
    const src = path.join(releaseDir, file);
    if (!existsSync(src)) {
      throw new Error(`expected ${file} inside whisper-bin-x64.zip, not found`);
    }
    const size = statSync(src).size;
    if (size < MIN_BYTES[file]) {
      throw new Error(`${file} implausibly small (${size} bytes)`);
    }
    mkdirSync(BINARIES, { recursive: true });
    copyFileSync(src, path.join(BINARIES, file));
    console.log(`[fetch-whisper]   -> ${file} (${size} bytes)`);
  }
  rmSync(TMP, { recursive: true, force: true });
}

async function fetchModel() {
  if (alreadyHasModel()) {
    console.log("[fetch-whisper] a model is already present, skipping");
    return;
  }
  console.log("[fetch-whisper] downloading ggml-base.bin (~148 MB)...");
  mkdirSync(BINARIES, { recursive: true });
  const dest = path.join(BINARIES, "ggml-base.bin");
  const size = await download(MODEL_URL, dest);
  if (size < 100_000_000) {
    throw new Error(`ggml-base.bin implausibly small (${size} bytes)`);
  }
  console.log(`[fetch-whisper]   -> ggml-base.bin (${size} bytes)`);
}

async function main() {
  mkdirSync(BINARIES, { recursive: true });
  await fetchEngine();
  await fetchModel();
}

main().catch((err) => {
  console.error(`[fetch-whisper] FAILED: ${err.message}`);
  process.exit(1);
});
