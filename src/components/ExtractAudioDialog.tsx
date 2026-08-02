import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Check, Loader2, Music } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";
import { api } from "@/services/api";
import type { EditProgress } from "@/types";

const FORMATS = ["mp3", "m4a", "wav", "aac", "flac", "ogg"];

export function ExtractAudioDialog({
  title,
  sourceId,
  inputPath,
  onClose,
}: {
  title: string;
  sourceId?: number;
  inputPath?: string;
  onClose: () => void;
}) {
  const queryClient = useQueryClient();
  const [selected, setSelected] = useState<Set<string>>(() => new Set(["mp3"]));
  const [running, setRunning] = useState(false);
  const [percent, setPercent] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [doneCount, setDoneCount] = useState<number | null>(null);
  const jobId = useRef<string>(crypto.randomUUID());

  useEffect(() => {
    const un = listen<EditProgress>("extract-progress", (e) => {
      if (e.payload.job_id === jobId.current) setPercent(e.payload.percent);
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  const toggle = (fmt: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(fmt)) next.delete(fmt);
      else next.add(fmt);
      return next;
    });

  const run = async () => {
    if (selected.size === 0) return;
    setError(null);
    setDoneCount(null);
    setRunning(true);
    setPercent(0);
    jobId.current = crypto.randomUUID();
    try {
      const created = await api.extractAudio(
        {
          source_id: sourceId ?? null,
          input_path: inputPath ?? null,
          formats: [...selected],
        },
        jobId.current,
      );
      setPercent(100);
      setDoneCount(created.length);
      queryClient.invalidateQueries({ queryKey: ["library"] });
      queryClient.invalidateQueries({ queryKey: ["platforms"] });
    } catch (e) {
      setError(String((e as Error)?.message ?? e));
    } finally {
      setRunning(false);
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && !running && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Music className="h-5 w-5 text-primary" />
            Extract audio
          </DialogTitle>
          <DialogDescription className="line-clamp-1">{title}</DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-2">
            <Label className="text-xs text-muted-foreground">
              Output formats (pick one or more)
            </Label>
            <div className="flex flex-wrap gap-2">
              {FORMATS.map((fmt) => {
                const on = selected.has(fmt);
                return (
                  <button
                    key={fmt}
                    type="button"
                    disabled={running}
                    onClick={() => toggle(fmt)}
                    className={cn(
                      "rounded-full border px-3.5 py-1.5 text-sm font-medium transition-colors",
                      on
                        ? "border-primary bg-primary/10 text-primary"
                        : "text-muted-foreground hover:bg-accent",
                    )}
                  >
                    {on && <Check className="mr-1 inline h-3.5 w-3.5" />}
                    {fmt.toUpperCase()}
                  </button>
                );
              })}
            </div>
          </div>

          {running && (
            <div className="space-y-1.5">
              <Progress value={percent} indeterminate={percent == null} />
              <p className="text-xs text-muted-foreground">
                Extracting… {percent != null ? `${Math.round(percent)}%` : ""}
              </p>
            </div>
          )}
          {doneCount != null && !running && (
            <p className="text-sm text-emerald-600 dark:text-emerald-400">
              Done — {doneCount} file{doneCount === 1 ? "" : "s"} added to your library.
            </p>
          )}
          {error && <p className="text-sm text-destructive">{error}</p>}
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={onClose} disabled={running}>
            {doneCount != null ? "Close" : "Cancel"}
          </Button>
          <Button onClick={run} disabled={running || selected.size === 0}>
            {running ? (
              <>
                <Loader2 className="animate-spin" /> Working…
              </>
            ) : (
              `Extract ${selected.size || ""}`.trim()
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
