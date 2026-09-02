import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Eraser, Loader2 } from "lucide-react";
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
import { Progress } from "@/components/ui/progress";
import { api } from "@/services/api";
import type { EditProgress } from "@/types";

export function StripMetadataDialog({
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
  const [running, setRunning] = useState(false);
  const [percent, setPercent] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState(false);
  const jobId = useRef<string>(crypto.randomUUID());

  useEffect(() => {
    const un = listen<EditProgress>("edit-progress", (e) => {
      if (e.payload.job_id === jobId.current) setPercent(e.payload.percent);
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  const run = async () => {
    setError(null);
    setDone(false);
    setRunning(true);
    setPercent(0);
    jobId.current = crypto.randomUUID();
    try {
      await api.stripMetadata(
        { source_id: sourceId ?? null, input_path: inputPath ?? null },
        jobId.current,
      );
      setPercent(100);
      setDone(true);
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
            <Eraser className="h-5 w-5 text-primary" />
            Remove metadata
          </DialogTitle>
          <DialogDescription className="line-clamp-1">{title}</DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <p className="text-sm text-muted-foreground">
            Removes embedded metadata — location, device info, timestamps, and
            encoder/software tags — from the file and saves a clean copy to your
            library. The audio and video streams themselves are copied as-is, so
            quality is unchanged.
          </p>

          {running && (
            <div className="space-y-1.5">
              <Progress value={percent} indeterminate={percent == null} />
              <p className="text-xs text-muted-foreground">
                Removing metadata… {percent != null ? `${Math.round(percent)}%` : ""}
              </p>
            </div>
          )}
          {done && !running && (
            <p className="text-sm text-emerald-600 dark:text-emerald-400">
              Done — the cleaned file was added to your library.
            </p>
          )}
          {error && <p className="text-sm text-destructive">{error}</p>}
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={onClose} disabled={running}>
            {done ? "Close" : "Cancel"}
          </Button>
          <Button onClick={run} disabled={running}>
            {running ? (
              <>
                <Loader2 className="animate-spin" /> Working…
              </>
            ) : done ? (
              "Run again"
            ) : (
              "Remove metadata"
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
