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
import { api } from "@/services/api";
import type { Download, MediaInfo } from "@/types";

function looksLikeUrl(value: string): boolean {
  try {
    const url = new URL(value.trim());
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}

export function UrlBar({ autoFocus = false }: { autoFocus?: boolean }) {
  const navigate = useNavigate();
  const [url, setUrl] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<MediaInfo | null>(null);
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

  const handleSubmit = async () => {
    const target = url.trim();
    if (!looksLikeUrl(target)) {
      setError("Please enter a valid http(s) URL.");
      return;
    }
    setError(null);
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
            placeholder="Paste a video, audio or playlist URL…"
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
              <Loader2 className="animate-spin" /> Fetching…
            </>
          ) : (
            "Download"
          )}
        </Button>
      </div>
      {error && <p className="mt-2 text-sm text-destructive">{error}</p>}

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
