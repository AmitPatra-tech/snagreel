import { useMemo, useState } from "react";
import { AlertCircle, Check, Film, ListVideo, Loader2, Music } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { formatDuration } from "@/lib/format";
import { api } from "@/services/api";
import type { Download, MediaInfo } from "@/types";

const VIDEO_CONTAINERS = ["mp4", "webm", "mkv"] as const;
const AUDIO_CONTAINERS = ["mp3", "m4a", "wav"] as const;
const FALLBACK_HEIGHTS = [2160, 1440, 1080, 720, 480, 360];

/** One pasted link, once we know whether it resolved. */
export interface BatchItem {
  url: string;
  info: MediaInfo | null;
  /** Why this link could not be read, if it could not. */
  error: string | null;
  /** A completed download of the same URL, if there is one. */
  duplicate: Download | null;
}

/** What to download for one link (or, in "same for all" mode, for every link). */
interface Choice {
  audioOnly: boolean;
  quality: string;
  container: string;
}

const DEFAULT_CHOICE: Choice = { audioOnly: false, quality: "best", container: "mp4" };

function heightsFor(info: MediaInfo | null): number[] {
  const fromFormats = [
    ...new Set(
      (info?.formats ?? [])
        .map((f) => f.height)
        .filter((h): h is number => h != null && h > 0),
    ),
  ].sort((a, b) => b - a);
  return fromFormats.length > 0 ? fromFormats : FALLBACK_HEIGHTS;
}

/** How many downloads one row will actually queue (a playlist expands). */
function itemCount(item: BatchItem): number {
  if (!item.info) return 0;
  return item.info.kind === "playlist" ? item.info.entries.length : 1;
}

function ChoiceControls({
  choice,
  heights,
  onChange,
  compact = false,
}: {
  choice: Choice;
  heights: number[];
  onChange: (next: Choice) => void;
  compact?: boolean;
}) {
  const containers = choice.audioOnly ? AUDIO_CONTAINERS : VIDEO_CONTAINERS;
  const setMode = (audioOnly: boolean) =>
    onChange({ ...choice, audioOnly, container: audioOnly ? "mp3" : "mp4" });

  return (
    <div className={cn("flex flex-wrap items-center gap-2", compact && "gap-1.5")}>
      <div className="flex overflow-hidden rounded-md border">
        <button
          type="button"
          onClick={() => setMode(false)}
          className={cn(
            "flex items-center gap-1.5 px-2.5 py-1.5 text-xs font-medium transition-colors cursor-pointer",
            !choice.audioOnly ? "bg-primary/10 text-primary" : "hover:bg-accent",
          )}
        >
          <Film className="h-3.5 w-3.5" /> Video
        </button>
        <button
          type="button"
          onClick={() => setMode(true)}
          className={cn(
            "flex items-center gap-1.5 border-l px-2.5 py-1.5 text-xs font-medium transition-colors cursor-pointer",
            choice.audioOnly ? "bg-primary/10 text-primary" : "hover:bg-accent",
          )}
        >
          <Music className="h-3.5 w-3.5" /> Audio
        </button>
      </div>

      {!choice.audioOnly && (
        <Select
          value={choice.quality}
          onValueChange={(quality) => onChange({ ...choice, quality })}
        >
          <SelectTrigger className={cn("h-8 w-[130px] text-xs", compact && "w-[112px]")}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="best">Best available</SelectItem>
            {heights.map((h) => (
              <SelectItem key={h} value={String(h)}>
                {h}p
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      )}

      <Select
        value={choice.container}
        onValueChange={(container) => onChange({ ...choice, container })}
      >
        <SelectTrigger className={cn("h-8 w-[92px] text-xs", compact && "w-[84px]")}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {containers.map((c) => (
            <SelectItem key={c} value={c}>
              {c.toUpperCase()}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

interface Props {
  items: BatchItem[];
  onClose: () => void;
  onQueued: () => void;
}

export function BatchDialog({ items, onClose, onQueued }: Props) {
  const readable = useMemo(
    () => items.map((item, i) => ({ item, i })).filter(({ item }) => item.info != null),
    [items],
  );

  // Links that failed to load can never be queued; ones already in the library
  // start unticked so a re-paste of a mixed list does not silently redownload.
  const [selected, setSelected] = useState<Set<number>>(
    () =>
      new Set(
        readable.filter(({ item }) => item.duplicate == null).map(({ i }) => i),
      ),
  );
  // Per-link is the default: every row arrives pre-filled, so accepting all of
  // them is still one click, but changing just one costs nothing.
  const [perLink, setPerLink] = useState(true);
  const [shared, setShared] = useState<Choice>(DEFAULT_CHOICE);
  const [choices, setChoices] = useState<Choice[]>(() => items.map(() => DEFAULT_CHOICE));
  const [submitting, setSubmitting] = useState(false);
  const [failed, setFailed] = useState<string[]>([]);

  const queueCount = readable
    .filter(({ i }) => selected.has(i))
    .reduce((sum, { item }) => sum + itemCount(item), 0);

  const toggle = (index: number) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });

  const setChoiceAt = (index: number, next: Choice) =>
    setChoices((prev) => prev.map((c, i) => (i === index ? next : c)));

  const handleDownload = async () => {
    setSubmitting(true);
    const problems: string[] = [];
    try {
      for (const { item, i } of readable) {
        if (!selected.has(i) || !item.info) continue;
        const choice = perLink ? choices[i] : shared;
        const base = {
          audio_only: choice.audioOnly,
          quality: choice.audioOnly || choice.quality === "best" ? null : choice.quality,
          container: choice.container,
        };
        // A playlist link expands into one download per entry, matching what
        // the single-link dialog does.
        const targets =
          item.info.kind === "playlist"
            ? item.info.entries.map((e) => ({
                url: e.url,
                title: e.title,
                thumbnail: e.thumbnail,
                duration: e.duration,
              }))
            : [
                {
                  url: item.info.url,
                  title: item.info.title,
                  thumbnail: item.info.thumbnail,
                  duration: item.info.duration,
                },
              ];
        for (const target of targets) {
          try {
            await api.startDownload({
              ...base,
              ...target,
              platform: item.info.platform,
            });
          } catch {
            // One rejected link must not strand the other eleven.
            problems.push(target.title || target.url);
          }
        }
      }
      if (problems.length > 0) {
        setFailed(problems);
        return;
      }
      onQueued();
    } finally {
      setSubmitting(false);
    }
  };

  const allSelected = selected.size === readable.length && readable.length > 0;

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>
            {items.length} link{items.length === 1 ? "" : "s"} ready
          </DialogTitle>
          <DialogDescription>
            They are queued in the order you pasted them and download together, up to
            your simultaneous-downloads setting.
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-wrap items-center justify-between gap-2 rounded-lg border p-2">
          <span className="pl-1 text-xs font-medium text-muted-foreground">
            Quality &amp; format
          </span>
          <div className="flex overflow-hidden rounded-md border">
            <button
              type="button"
              onClick={() => setPerLink(false)}
              className={cn(
                "px-3 py-1.5 text-xs font-medium transition-colors cursor-pointer",
                !perLink ? "bg-primary/10 text-primary" : "hover:bg-accent",
              )}
            >
              Same for all
            </button>
            <button
              type="button"
              onClick={() => setPerLink(true)}
              className={cn(
                "border-l px-3 py-1.5 text-xs font-medium transition-colors cursor-pointer",
                perLink ? "bg-primary/10 text-primary" : "hover:bg-accent",
              )}
            >
              Choose per link
            </button>
          </div>
        </div>

        {!perLink && (
          <div className="rounded-lg border p-3">
            {/* The quality list comes from the first readable link. Links that
                cannot offer the chosen height fall back to their best, so an
                unavailable pick degrades rather than failing. */}
            <ChoiceControls
              choice={shared}
              heights={heightsFor(readable[0]?.item.info ?? null)}
              onChange={setShared}
            />
          </div>
        )}

        <div className="max-h-[46vh] overflow-y-auto rounded-lg border">
          {readable.length > 1 && (
            <label className="flex cursor-pointer items-center gap-2 border-b bg-muted/50 px-3 py-2 text-sm font-medium">
              <input
                type="checkbox"
                checked={allSelected}
                onChange={(e) =>
                  setSelected(
                    e.target.checked ? new Set(readable.map(({ i }) => i)) : new Set(),
                  )
                }
              />
              Select all ({selected.size}/{readable.length})
            </label>
          )}

          {items.map((item, i) => {
            const unreadable = item.info == null;
            return (
              <div
                key={`${item.url}-${i}`}
                className={cn(
                  "border-b px-3 py-2.5 last:border-b-0",
                  unreadable && "bg-destructive/5",
                )}
              >
                <div className="flex items-start gap-2.5">
                  <input
                    type="checkbox"
                    className="mt-1"
                    disabled={unreadable}
                    checked={selected.has(i)}
                    onChange={() => toggle(i)}
                  />
                  <div className="min-w-0 flex-1">
                    <p
                      className={cn(
                        "truncate text-sm font-medium",
                        unreadable && "text-muted-foreground",
                      )}
                    >
                      {item.info?.title ?? item.url}
                    </p>
                    <div className="mt-0.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
                      {item.info && (
                        <Badge variant="secondary" className="text-[10px]">
                          {item.info.platform}
                        </Badge>
                      )}
                      {item.info?.kind === "playlist" && (
                        <span className="inline-flex items-center gap-1">
                          <ListVideo className="h-3 w-3" />
                          {item.info.entries.length} items
                        </span>
                      )}
                      {item.info?.duration != null && (
                        <span>{formatDuration(item.info.duration)}</span>
                      )}
                      {item.duplicate && (
                        <span className="inline-flex items-center gap-1 text-amber-600 dark:text-amber-500">
                          <Check className="h-3 w-3" /> Already downloaded
                        </span>
                      )}
                      {unreadable && (
                        <span className="inline-flex items-center gap-1 text-destructive">
                          <AlertCircle className="h-3 w-3" />
                          {item.error ?? "Could not read this link"}
                        </span>
                      )}
                    </div>

                    {perLink && !unreadable && selected.has(i) && (
                      <div className="mt-2">
                        <ChoiceControls
                          compact
                          choice={choices[i]}
                          heights={heightsFor(item.info)}
                          onChange={(next) => setChoiceAt(i, next)}
                        />
                      </div>
                    )}
                  </div>
                </div>
              </div>
            );
          })}
        </div>

        {failed.length > 0 && (
          <p className="text-sm text-destructive">
            Could not queue {failed.length} of them: {failed.slice(0, 3).join(", ")}
            {failed.length > 3 ? "…" : ""}
          </p>
        )}

        <DialogFooter>
          <Button variant="ghost" onClick={onClose} disabled={submitting}>
            Cancel
          </Button>
          <Button onClick={handleDownload} disabled={submitting || queueCount === 0}>
            {submitting ? (
              <>
                <Loader2 className="animate-spin" /> Adding…
              </>
            ) : (
              `Download ${queueCount} item${queueCount === 1 ? "" : "s"}`
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
