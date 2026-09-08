#!/usr/bin/env node
// Runs before `tauri build` (wired as `beforeBuildCommand` in
// tauri.conf.json). Deliberately a single Node script rather than a
// shell-chained command string (`"a && b"`) in the config — Tauri's docs
// don't specify which shell parses that string on which platform, and this
// removes the question entirely.
import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.dirname(fileURLToPath(import.meta.url));
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm";

execFileSync(process.execPath, [path.join(ROOT, "fetch-whisper.mjs")], { stdio: "inherit" });
execFileSync(npmCmd, ["run", "build"], { stdio: "inherit" });
