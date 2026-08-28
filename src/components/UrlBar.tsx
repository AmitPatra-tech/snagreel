import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { ClipboardPaste, Link2, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { MetadataDialog } from "@/components/MetadataDialog";
import { BatchDialog, type BatchItem } from "@/components/BatchDialog";
import { api } from "@/services/api";
import { MAX_BATCH_LINKS, parseUrls } from "@/lib/urls";
import type { Download, MediaInfo } from "@/types";

/**
 * Links read at once while building a batch.
 *
 * Reading twelve one after another leaves the user watching a spinner for the
 * better part of a minute; reading all twelve at once means twelve yt-dlp
 * processes and a good chance the site starts refusing them. Four is quick
 * without looking like an attack.
 */
const METADATA_CONCURRENCY = 4;

export function UrlBar({ autoFocus = false }: { autoFocus?: boolean }) {
  const navigate = useNavigate();
  const [url, setUrl] = useState("");
  const [loading, setLoading] = useState(false);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [info, setInfo] = useState<MediaInfo | null>(null);
  const [batch, setBatch] = useState<BatchItem[] | null>(null);
  const [duplicate, setDuplicate] = useState<Download | null>(null);

  const fetchMetadata = async (target: string) => {
    setLoading(true);
    setError(null);
    try {
      const media = await api.fetchMetadata(target);
      setInfo(media);
    } catch (e) {
      setError(typeof e === "string" ? e : "Could not load this URL. Check that it is public and supported.");
    } finally {
      setLoading(false);
    }
  };

  /** Read every link in the batch, a few at a time, keeping the typed order. */
  const readBatch = async (targets: string[]) => {
    setLoading(true);
    setProgress({ done: 0, total: targets.length });
    const items: BatchItem[] = [];
    try {
      for (let i = 0; i < targets.length; i += METADATA_CONCURRENCY) {
        const slice = targets.slice(i, i + METADATA_CONCURRENCY);
        const read = await Promise.all(
          slice.map(async (target): Promise<BatchItem> => {
            // Best-effort: a failed duplicate check should not fail the link.
            const duplicate = await api.checkDuplicate(target).catch(() => null);
            try {
              return { url: target, info: await api.fetchMetadata(target), error: null, duplicate };
            } catch (e) {
              return {
                url: target,
                info: null,
                error: typeof e === "string" ? e : "Could not read this link.",
                duplicate,
              };
            }
          }),
        );
        items.push(...read);
        setProgress({ done: items.length, total: targets.length });
      }
      // Every link failing is an error, not a batch worth confirming.
      if (items.every((item) => item.info == null)) {
        setError("None of those links could be read. Check they are public and supported.");
        return;
      }
      setBatch(items);
    } finally {
      setLoading(false);
      setProgress(null);
    }
  };

  const handleSubmit = async () => {
    const { urls, invalid, overflow } = parseUrls(url);
    if (urls.length === 0) {
      setError(
        invalid.length > 0
          ? "That does not look like an http(s) link."
          : "Please enter a valid http(s) URL.",
      );
      return;
    }
    setError(null);

    // Tell the user what was dropped rather than silently downloading less
    // than they pasted.
    const dropped: string[] = [];
    if (invalid.length > 0) {
      dropped.push(`${invalid.length} entr${invalid.length === 1 ? "y was" : "ies were"} not a link`);
    }
    if (overflow.length > 0) {
      dropped.push(`${overflow.length} past the ${MAX_BATCH_LINKS}-link limit`);
    }
    setNotice(dropped.length > 0 ? `Skipped ${dropped.join(" and ")}.` : null);

    if (urls.length > 1) {
      await readBatch(urls);
      return;
    }

    const target = urls[0];
    try {
      const existing = await api.checkDuplicate(target);
      if (existing) {
        setDuplicate(existing);
        return;
      }
    } catch {
      // duplicate check is best-effort
    }
    await fetchMetadata(target);
  };

  const handlePaste = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text) setUrl(text.trim());
    } catch {
      // clipboard unavailable
    }
  };

  return (
    <div className="w-full">
      <div className="flex gap-2">
        <div className="relative flex-1">
          <Link2 className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            autoFocus={autoFocus}
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
            placeholder={`Paste a URL — or up to ${MAX_BATCH_LINKS}, separated by commas…`}
            className="h-11 pl-9 pr-10 text-base"
            spellCheck={false}
          />
          <button
            type="button"
            onClick={handlePaste}
            title="Paste from clipboard"
            className="absolute right-2 top-1/2 -translate-y-1/2 rounded p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground cursor-pointer"
          >
            <ClipboardPaste className="h-4 w-4" />
          </button>
        </div>
        <Button size="lg" className="h-11" onClick={handleSubmit} disabled={loading}>
          {loading ? (
            <>
              <Loader2 className="animate-spin" />
              {progress ? `Reading ${progress.done}/${progress.total}…` : "Fetching…"}
            </>
          ) : (
            "Download"
          )}
        </Button>
      </div>
      {error && <p className="mt-2 text-sm text-destructive">{error}</p>}
      {notice && !error && <p className="mt-2 text-sm text-muted-foreground">{notice}</p>}

      {info && (
        <MetadataDialog
          info={info}
          onClose={() => setInfo(null)}
          onQueued={() => {
            setInfo(null);
            setUrl("");
            navigate("/downloads");
          }}
        />
      )}

      {batch && (
        <BatchDialog
          items={batch}
          onClose={() => setBatch(null)}
          onQueued={() => {
            setBatch(null);
            setUrl("");
            setNotice(null);
            navigate("/downloads");
          }}
        />
      )}

      <Dialog open={duplicate !== null} onOpenChange={(open) => !open && setDuplicate(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Already downloaded</DialogTitle>
            <DialogDescription>
              “{duplicate?.title}” was already downloaded
              {duplicate?.completed_at ? ` on ${new Date(duplicate.completed_at).toLocaleDateString()}` : ""}.
              What would you like to do?
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setDuplicate(null)}>
              Cancel
            </Button>
            <Button
              variant="secondary"
              onClick={() => {
                const target = url.trim();
                setDuplicate(null);
                fetchMetadata(target);
              }}
            >
              Download again
            </Button>
            <Button
              onClick={() => {
                if (duplicate) api.openFile(duplicate.id).catch(() => {});
                setDuplicate(null);
              }}
            >
              Open existing
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
