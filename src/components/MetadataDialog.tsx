import { useMemo, useState } from "react";
import { Clock, Film, ListVideo, Loader2, Music, Scissors, User } from "lucide-react";
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
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";
import { ProBadge, UpgradeProDialog } from "@/components/UpgradeProDialog";
import { useIsPro } from "@/hooks/useActivation";
import { cn } from "@/lib/utils";
import { formatDuration } from "@/lib/format";
import { api } from "@/services/api";
import type { MediaInfo } from "@/types";

/** Parse "90", "1:30" or "1:02:03" into seconds. Empty → null. */
function parseClock(input: string): number | null {
  const t = input.trim();
  if (!t) return null;
  if (t.includes(":")) {
    const parts = t.split(":").map((p) => Number(p));
    if (parts.some((n) => Number.isNaN(n))) return null;
    return parts.reduce((acc, n) => acc * 60 + n, 0);
  }
  const n = Number(t);
  return Number.isNaN(n) ? null : n;
}

const VIDEO_CONTAINERS = ["mp4", "webm", "mkv"] as const;
const AUDIO_CONTAINERS = ["mp3", "m4a", "wav"] as const;
const FALLBACK_HEIGHTS = [2160, 1440, 1080, 720, 480, 360];

interface Props {
  info: MediaInfo;
  onClose: () => void;
  onQueued: () => void;
}

export function MetadataDialog({ info, onClose, onQueued }: Props) {
  const isPlaylist = info.kind === "playlist";
  const [audioOnly, setAudioOnly] = useState(false);
  const [quality, setQuality] = useState<string>("best");
  const [container, setContainer] = useState<string>("mp4");
  const [submitting, setSubmitting] = useState(false);
  const [selected, setSelected] = useState<Set<number>>(
    () => new Set(info.entries.map((_, i) => i)),
  );
  const isPro = useIsPro();
  const [clipEnabled, setClipEnabled] = useState(false);
  const [clipStart, setClipStart] = useState("");
  const [clipEnd, setClipEnd] = useState("");
  const [showUpgrade, setShowUpgrade] = useState(false);

  const heights = useMemo(() => {
    const fromFormats = [
      ...new Set(
        info.formats
          .map((f) => f.height)
          .filter((h): h is number => h != null && h > 0),
      ),
    ].sort((a, b) => b - a);
    return fromFormats.length > 0 ? fromFormats : FALLBACK_HEIGHTS;
  }, [info.formats]);

  const containers = audioOnly ? AUDIO_CONTAINERS : VIDEO_CONTAINERS;

  const switchMode = (audio: boolean) => {
    setAudioOnly(audio);
    setContainer(audio ? "mp3" : "mp4");
  };

  const toggleEntry = (index: number) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(index)) next.delete(index);
      else next.add(index);
      return next;
    });
  };

  const handleDownload = async () => {
    setSubmitting(true);
    try {
      const clip =
        !isPlaylist && clipEnabled && isPro
          ? { clip_start: parseClock(clipStart) ?? 0, clip_end: parseClock(clipEnd) }
          : {};
      const base = {
        audio_only: audioOnly,
        quality: audioOnly || quality === "best" ? null : quality,
        container,
        ...clip,
      };
      if (isPlaylist) {
        const entries = info.entries.filter((_, i) => selected.has(i));
        for (const entry of entries) {
          await api.startDownload({
            ...base,
            url: entry.url,
            title: entry.title,
            platform: info.platform,
            thumbnail: entry.thumbnail,
            duration: entry.duration,
          });
        }
      } else {
        await api.startDownload({
          ...base,
          url: info.url,
          title: info.title,
          platform: info.platform,
          thumbnail: info.thumbnail,
          duration: info.duration,
        });
      }
      onQueued();
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle className="line-clamp-2 pr-6">{info.title}</DialogTitle>
          <DialogDescription className="flex flex-wrap items-center gap-x-3 gap-y-1">
            <Badge variant="secondary">{info.platform}</Badge>
            {info.uploader && (
              <span className="inline-flex items-center gap-1">
                <User className="h-3.5 w-3.5" /> {info.uploader}
              </span>
            )}
            {info.duration != null && (
              <span className="inline-flex items-center gap-1">
                <Clock className="h-3.5 w-3.5" /> {formatDuration(info.duration)}
              </span>
            )}
            {isPlaylist && (
              <span className="inline-flex items-center gap-1">
                <ListVideo className="h-3.5 w-3.5" /> {info.entries.length} items
              </span>
            )}
          </DialogDescription>
        </DialogHeader>

        {info.thumbnail && !isPlaylist && (
          <img
            src={info.thumbnail}
            alt=""
            className="max-h-56 w-full rounded-lg border object-cover"
          />
        )}

        {isPlaylist && (
          <div className="max-h-56 overflow-y-auto rounded-lg border">
            <label className="flex cursor-pointer items-center gap-2 border-b bg-muted/50 px-3 py-2 text-sm font-medium">
              <input
                type="checkbox"
                checked={selected.size === info.entries.length}
                onChange={(e) =>
                  setSelected(
                    e.target.checked
                      ? new Set(info.entries.map((_, i) => i))
                      : new Set(),
                  )
                }
              />
              Select all ({selected.size}/{info.entries.length})
            </label>
            {info.entries.map((entry, i) => (
              <label
                key={`${entry.url}-${i}`}
                className="flex cursor-pointer items-center gap-2 border-b px-3 py-2 text-sm last:border-b-0 hover:bg-accent/50"
              >
                <input
                  type="checkbox"
                  checked={selected.has(i)}
                  onChange={() => toggleEntry(i)}
                />
                <span className="flex-1 truncate">{entry.title}</span>
                <span className="text-xs text-muted-foreground">
                  {formatDuration(entry.duration)}
                </span>
              </label>
            ))}
          </div>
        )}

        <div className="grid grid-cols-2 gap-3">
          <button
            type="button"
            onClick={() => switchMode(false)}
            className={cn(
              "flex items-center justify-center gap-2 rounded-lg border p-3 text-sm font-medium transition-colors cursor-pointer",
              !audioOnly
                ? "border-primary bg-primary/10 text-primary"
                : "hover:bg-accent",
            )}
          >
            <Film className="h-4 w-4" /> Video
          </button>
          <button
            type="button"
            onClick={() => switchMode(true)}
            className={cn(
              "flex items-center justify-center gap-2 rounded-lg border p-3 text-sm font-medium transition-colors cursor-pointer",
              audioOnly
                ? "border-primary bg-primary/10 text-primary"
                : "hover:bg-accent",
            )}
          >
            <Music className="h-4 w-4" /> Audio only
          </button>
        </div>

        <div className="grid grid-cols-2 gap-3">
          {!audioOnly && (
            <div className="space-y-1.5">
              <span className="text-xs font-medium text-muted-foreground">Quality</span>
              <Select value={quality} onValueChange={setQuality}>
                <SelectTrigger>
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
            </div>
          )}
          <div className={cn("space-y-1.5", audioOnly && "col-span-2")}>
            <span className="text-xs font-medium text-muted-foreground">Format</span>
            <Select value={container} onValueChange={setContainer}>
              <SelectTrigger>
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
        </div>

        {!isPlaylist && (
          <div className="rounded-lg border p-3">
            <label className="flex cursor-pointer items-center justify-between gap-3">
              <span className="flex items-center gap-2 text-sm font-medium">
                <Scissors className="h-4 w-4 text-primary" />
                Download only a section
                <ProBadge />
              </span>
              <Switch
                checked={clipEnabled}
                onCheckedChange={(v) => {
                  if (v && !isPro) {
                    setShowUpgrade(true);
                    return;
                  }
                  setClipEnabled(v);
                }}
              />
            </label>
            {clipEnabled && isPro && (
              <div className="mt-3 grid grid-cols-2 gap-3">
                <div className="space-y-1.5">
                  <span className="text-xs font-medium text-muted-foreground">
                    Start (s or mm:ss)
                  </span>
                  <Input
                    value={clipStart}
                    onChange={(e) => setClipStart(e.target.value)}
                    placeholder="0:00"
                  />
                </div>
                <div className="space-y-1.5">
                  <span className="text-xs font-medium text-muted-foreground">
                    End (blank = to end)
                  </span>
                  <Input
                    value={clipEnd}
                    onChange={(e) => setClipEnd(e.target.value)}
                    placeholder="1:30"
                  />
                </div>
              </div>
            )}
          </div>
        )}

        <UpgradeProDialog
          open={showUpgrade}
          onClose={() => setShowUpgrade(false)}
          feature="Clip download"
        />

        <DialogFooter>
          <Button variant="ghost" onClick={onClose} disabled={submitting}>
            Cancel
          </Button>
          <Button
            onClick={handleDownload}
            disabled={submitting || (isPlaylist && selected.size === 0)}
          >
            {submitting ? (
              <>
                <Loader2 className="animate-spin" /> Adding…
              </>
            ) : isPlaylist ? (
              `Download ${selected.size} item${selected.size === 1 ? "" : "s"}`
            ) : (
              "Download"
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
