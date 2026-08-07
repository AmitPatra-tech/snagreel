import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

/**
 * The running app's version, read from the bundle rather than hardcoded — so a
 * release bump can never leave the UI advertising a stale number.
 *
 * Empty until it resolves, and stays empty outside a Tauri window (plain `vite
 * dev` in a browser), so callers should render nothing rather than a wrong
 * version.
 */
export function useAppVersion(): string {
  const [version, setVersion] = useState("");

  useEffect(() => {
    let cancelled = false;
    getVersion()
      .then((v) => {
        if (!cancelled) setVersion(v);
      })
      .catch(() => {
        /* not running under Tauri — leave it blank */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return version;
}
