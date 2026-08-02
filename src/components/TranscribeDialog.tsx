import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Check, Copy, FileText, Loader2, Languages } from "lucide-react";
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
import { api } from "@/services/api";
import type { TranscribeProgress, TranscribeResult } from "@/types";

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
  { code: "it", label: "Italian" },
  { code: "tr", label: "Turkish" },
  { code: "vi", label: "Vietnamese" },
  { code: "ta", label: "Tamil" },
  { code: "te", label: "Telugu" },
  { code: "nl", label: "Dutch" },
  { code: "pl", label: "Polish" },
  { code: "th", label: "Thai" },
  { code: "fa", label: "Persian" },
  { code: "uk", label: "Ukrainian" },
];

export function TranscribeDialog({
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
  const [language, setLanguage] = useState("auto");
  const [running, setRunning] = useState(false);
  const [percent, setPercent] = useState<number | null>(null);
  const [stage, setStage] = useState("");
  const [result, setResult] = useState<TranscribeResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
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
    setResult(null);
    setRunning(true);
    setPercent(0);
    setStage("Starting");
    jobId.current = crypto.randomUUID();
    try {
      const res = await api.transcribe(
        { source_id: sourceId ?? null, input_path: inputPath ?? null, language },
        jobId.current,
      );
      setResult(res);
    } catch (e) {
      setError(String((e as Error)?.message ?? e));
    } finally {
      setRunning(false);
    }
  };

  const copy = async () => {
    if (!result) return;
    try {
      await navigator.clipboard.writeText(result.text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      /* ignore */
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && !running && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Languages className="h-5 w-5 text-primary" />
            Transcribe to text
          </DialogTitle>
          <DialogDescription className="line-clamp-1">{title}</DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="flex items-end gap-3">
            <div className="space-y-1.5">
              <Label className="text-xs text-muted-foreground">Language</Label>
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
            <Button onClick={run} disabled={running}>
              {running ? (
                <>
                  <Loader2 className="animate-spin" /> Transcribing…
                </>
              ) : result ? (
                "Transcribe again"
              ) : (
                "Transcribe"
              )}
            </Button>
          </div>

          <p className="text-xs text-muted-foreground">
            Runs fully on your device. Accuracy is very high but, like all speech-to-text,
            depends on audio clarity, accents and background noise.
          </p>

          {running && (
            <div className="space-y-1.5">
              <Progress value={percent} indeterminate={percent == null} />
              <p className="text-xs text-muted-foreground">
                {stage}
                {percent != null ? ` · ${Math.round(percent)}%` : ""}
              </p>
            </div>
          )}

          {error && <p className="text-sm text-destructive">{error}</p>}

          {result && (
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
                  <FileText className="h-3.5 w-3.5" />
                  Saved as .txt and .srt next to the file
                </span>
                <Button size="sm" variant="outline" onClick={copy}>
                  {copied ? <Check className="text-emerald-500" /> : <Copy />}
                  {copied ? "Copied" : "Copy text"}
                </Button>
              </div>
              <textarea
                readOnly
                value={result.text}
                className="h-56 w-full resize-none rounded-lg border bg-muted/40 p-3 text-sm leading-relaxed"
              />
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={onClose} disabled={running}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
