import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Captions, Loader2 } from "lucide-react";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useQueryClient } from "@tanstack/react-query";
import { cn } from "@/lib/utils";
import { api } from "@/services/api";
import type { TranscribeProgress } from "@/types";

// Major languages Whisper supports well, plus auto-detect.
const LANGUAGES: { code: string; label: string }[] = [
  { code: "auto", label: "Auto-detect" },
  { code: "en", label: "English" },
  { code: "zh", label: "Chinese" },
  { code: "hi", label: "Hindi" },
  { code: "es", label: "Spanish" },
  { code: "ar", label: "Arabic" },
  { code: "fr", label: "French" },
  { code: "bn", label: "Bengali" },
  { code: "pt", label: "Portuguese" },
  { code: "ru", label: "Russian" },
  { code: "ur", label: "Urdu" },
  { code: "id", label: "Indonesian" },
  { code: "de", label: "German" },
  { code: "ja", label: "Japanese" },
  { code: "ko", label: "Korean" },
];

// Caption look templates. Each maps to a `force_style` preset on the Rust
// side (src-tauri/src/captions/mod.rs) — the classNames here only mimic the
// look for the picker; the real rendering happens in FFmpeg.
const STYLES: {
  id: string;
  label: string;
  sample: string;
  className: string;
}[] = [
  {
    id: "classic",
    label: "Classic",
    sample: "Captions",
    className: "font-bold text-white [text-shadow:0_0_3px_#000,0_0_3px_#000]",
  },
  {
    id: "yellow",
    label: "Bold Yellow",
    sample: "Captions",
    className: "font-bold text-yellow-300 [text-shadow:0_0_3px_#000,0_0_3px_#000]",
  },
  {
    id: "boxed",
    label: "Boxed",
    sample: "Captions",
    className: "rounded bg-black px-1.5 py-0.5 font-bold text-white",
  },
  {
    id: "minimal",
    label: "Minimal",
    sample: "Captions",
    className: "text-sm text-white [text-shadow:0_0_2px_#000]",
  },
];

export function AddCaptionsDialog({
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
  const [language, setLanguage] = useState("auto");
  const [style, setStyle] = useState("classic");
  const [running, setRunning] = useState(false);
  const [percent, setPercent] = useState<number | null>(null);
  const [stage, setStage] = useState("");
  const [done, setDone] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const jobId = useRef<string>(crypto.randomUUID());

  useEffect(() => {
    const un = listen<TranscribeProgress>("transcribe-progress", (e) => {
      if (e.payload.job_id === jobId.current) {
        setPercent(e.payload.percent);
        setStage(e.payload.stage);
      }
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
    setStage("Starting");
    jobId.current = crypto.randomUUID();
    try {
      await api.addCaptions(
        { source_id: sourceId ?? null, input_path: inputPath ?? null, language, style },
        jobId.current,
      );
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
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Captions className="h-5 w-5 text-primary" />
            Add captions
          </DialogTitle>
          <DialogDescription className="line-clamp-1">{title}</DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <p className="text-sm text-muted-foreground">
            Transcribes the speech in this video and burns the captions directly onto a
            new copy of it — no separate subtitle file to keep track of. Runs fully on
            your device.
          </p>

          <div className="space-y-1.5">
            <Label className="text-xs text-muted-foreground">Caption style</Label>
            <div className="grid grid-cols-4 gap-2">
              {STYLES.map((s) => (
                <button
                  key={s.id}
                  type="button"
                  disabled={running}
                  onClick={() => setStyle(s.id)}
                  className={cn(
                    "flex flex-col items-center gap-2 rounded-lg border p-3 transition-colors",
                    style === s.id
                      ? "border-primary bg-primary/10"
                      : "hover:bg-accent",
                  )}
                >
                  <span className="flex h-10 w-full items-center justify-center rounded bg-slate-800">
                    <span className={s.className}>{s.sample}</span>
                  </span>
                  <span className="text-xs font-medium">{s.label}</span>
                </button>
              ))}
            </div>
          </div>

          <div className="space-y-1.5">
            <Label className="text-xs text-muted-foreground">Spoken language</Label>
            <Select value={language} onValueChange={setLanguage} disabled={running}>
              <SelectTrigger className="w-56">
                <SelectValue />
              </SelectTrigger>
              <SelectContent className="max-h-72">
                {LANGUAGES.map((l) => (
                  <SelectItem key={l.code} value={l.code}>
                    {l.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {running && (
            <div className="space-y-1.5">
              <Progress value={percent} indeterminate={percent == null} />
              <p className="text-xs text-muted-foreground">
                {stage}
                {percent != null ? ` · ${Math.round(percent)}%` : ""}
              </p>
            </div>
          )}
          {done && !running && (
            <p className="text-sm text-emerald-600 dark:text-emerald-400">
              Done — the captioned video was added to your library.
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
              "Add captions"
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
