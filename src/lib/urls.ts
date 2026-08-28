/**
 * How many links one paste may carry.
 *
 * Kept equal to `MAX_CONCURRENT_DOWNLOADS` in `src-tauri/src/models/mod.rs`:
 * the two are independent limits, but a batch that matches the worker count is
 * the easiest one to reason about — paste twelve, twelve run.
 */
export const MAX_BATCH_LINKS = 12;

function isHttpUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}

export interface ParsedUrls {
  /** Valid, de-duplicated links, in the order they were typed. */
  urls: string[];
  /** Entries that were not http(s) URLs, so the user can see what was dropped. */
  invalid: string[];
  /** Links beyond `MAX_BATCH_LINKS`, which were not queued. */
  overflow: string[];
}

/**
 * Split a pasted string into links.
 *
 * Commas are the documented separator, but people paste from notes and chat
 * apps, so newlines, tabs and plain spaces separate too. A URL cannot contain
 * an unencoded space or comma, so nothing valid is broken by splitting on them.
 */
export function parseUrls(input: string): ParsedUrls {
  const pieces = input
    .split(/[\s,]+/)
    .map((piece) => piece.trim().replace(/[,;]+$/, ""))
    .filter((piece) => piece.length > 0);

  const urls: string[] = [];
  const invalid: string[] = [];
  for (const piece of pieces) {
    if (!isHttpUrl(piece)) {
      invalid.push(piece);
    } else if (!urls.includes(piece)) {
      // A link pasted twice is one download, not two.
      urls.push(piece);
    }
  }

  return {
    urls: urls.slice(0, MAX_BATCH_LINKS),
    invalid,
    overflow: urls.slice(MAX_BATCH_LINKS),
  };
}
