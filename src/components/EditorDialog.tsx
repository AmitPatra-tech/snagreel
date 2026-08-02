import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { Film, Loader2, Scissors } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
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
import type { Download, EditOperation, EditProgress, EditRequest } from "@/types";

const VIDEO_FORMATS = ["mp4", "mkv", "webm"];
const AUDIO_FORMATS = ["mp3", "m4a", "wav"];

/** Parse "90", "1:30" or "1:02:03" into seconds. Empty → null. */
function parseTime(input: string): number | null {
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

export function EditorDialog({
  item,
  onClose,
}: {
  item: Download;
  onClose: () => void;
}) {
  const queryClient = useQueryClient();
  const isAudio = item.audio_only;
  // Known duration lets us offer sliders; otherwise fall back to typed times.
  const duration =
    item.duration && item.duration > 0 ? Math.floor(item.duration) : null;

  const [op, setOp] = useState<EditOperation>("trim");
  const [startSec, setStartSec] = useState(0);
  const [endSec, setEndSec] = useState(duration ?? 0);
  const [startText, setStartText] = useState("");
  const [endText, setEndText] = useState("");
  const [format, setFormat] = useState(isAudio ? "mp3" : "mp4");

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

  const buildRequest = (): EditRequest | string => {
    const base: EditRequest = { source_id: item.id, operation: op };
    if (op === "trim") {
      if (duration != null) {
        if (endSec <= startSec) return "End must be after start.";
        return { ...base, start: startSec, end: endSec };
      }
      const s = parseTime(startText) ?? 0;
      const e = parseTime(endText);
      if (e != null && e <= s) return "End must be after start.";
      return { ...base, start: s, end: e };
    }
    return { ...base, output_format: format };
  };

  const run = async () => {
    const req = buildRequest();
    if (typeof req === "string") {
      setError(req);
      return;
    }
    setError(null);
    setDone(false);
    setRunning(true);
    setPercent(0);
    jobId.current = crypto.randomUUID();
    try {
      await api.runEdit(req, jobId.current);
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

  const modes: { id: EditOperation; label: string; icon: React.ElementType }[] = [
    { id: "trim", label: "Trim / cut", icon: Scissors },
    { id: "convert", label: "Convert format", icon: Film },
  ];

  return (
    <Dialog open onOpenChange={(o) => !o && !running && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>Edit</DialogTitle>
          <DialogDescription className="line-clamp-1">{item.title}</DialogDescription>
        </DialogHeader>

        <div className="grid grid-cols-2 gap-2">
          {modes.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              type="button"
              disabled={running}
              onClick={() => {
                setOp(id);
                setError(null);
                setDone(false);
              }}
              className={cn(
                "flex items-center justify-center gap-2 rounded-lg border p-3 text-sm font-medium transition-colors",
                op === id
                  ? "border-primary bg-primary/10 text-primary"
                  : "hover:bg-accent",
              )}
            >
              <Icon className="h-4 w-4" /> {label}
            </button>
          ))}
        </div>

        <div className="min-h-24 space-y-4">
          {op === "trim" && duration != null && (
            <div className="space-y-5">
              <RangeRow
                label="Start"
                value={startSec}
                display={formatDuration(startSec)}
                min={0}
                max={duration}
                disabled={running}
                onChange={(v) => setStartSec(Math.max(0, Math.min(v, endSec - 1)))}
              />
              <RangeRow
                label="End"
                value={endSec}
                display={formatDuration(endSec)}
                min={0}
                max={duration}
                disabled={running}
                onChange={(v) => setEndSec(Math.min(duration, Math.max(v, startSec + 1)))}
              />
              <p className="text-xs text-muted-foreground">
                Clip length: {formatDuration(Math.max(0, endSec - startSec))}
              </p>
            </div>
          )}

          {op === "trim" && duration == null && (
            <div className="grid grid-cols-2 gap-3">
              <Field label="Start (s or mm:ss)">
                <Input value={startText} onChange={(e) => setStartText(e.target.value)} placeholder="0:00" />
              </Field>
              <Field label="End (blank = to end)">
                <Input value={endText} onChange={(e) => setEndText(e.target.value)} placeholder="1:30" />
              </Field>
            </div>
          )}

          {op === "convert" && (
            <Field label="Convert to">
              <Select value={format} onValueChange={setFormat}>
                <SelectTrigger className="w-40">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {(isAudio ? AUDIO_FORMATS : VIDEO_FORMATS).map((o) => (
                    <SelectItem key={o} value={o}>
                      {o.toUpperCase()}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>
          )}

          {running && (
            <div className="space-y-1.5">
              <Progress value={percent} indeterminate={percent == null} />
              <p className="text-xs text-muted-foreground">
                Processing… {percent != null ? `${Math.round(percent)}%` : ""}
              </p>
            </div>
          )}
          {done && !running && (
            <p className="text-sm text-emerald-600 dark:text-emerald-400">
              Done — the edited file was added to your library.
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
              "Edit again"
            ) : (
              "Apply"
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function RangeRow({
  label,
  value,
  display,
  min,
  max,
  disabled,
  onChange,
}: {
  label: string;
  value: number;
  display: string;
  min: number;
  max: number;
  disabled?: boolean;
  onChange: (v: number) => void;
}) {
  return (
    <div>
      <div className="mb-1 flex justify-between text-xs font-medium">
        <span className="text-muted-foreground">{label}</span>
        <span className="tabular-nums text-foreground">{display}</span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={1}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value))}
        className="h-2 w-full cursor-pointer appearance-none rounded-full bg-secondary accent-primary"
      />
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="space-y-1.5">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      {children}
    </div>
  );
}
