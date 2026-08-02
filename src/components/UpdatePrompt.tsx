import { useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Download, Loader2, Sparkles } from "lucide-react";
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

/**
 * On startup, quietly checks for a newer release and — if one exists — prompts
 * the user to download and install it, then relaunches. Any error (e.g. running
 * unbundled in dev, or offline) is ignored so it never disrupts normal use.
 */
export function UpdatePrompt() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [open, setOpen] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [percent, setPercent] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const found = await check();
        if (!cancelled && found) {
          setUpdate(found);
          setOpen(true);
        }
      } catch {
        /* no updater in dev / offline — ignore */
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const install = async () => {
    if (!update) return;
    setInstalling(true);
    setError(null);
    setPercent(0);
    try {
      let downloaded = 0;
      let total = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          setPercent(total > 0 ? (downloaded / total) * 100 : null);
        } else if (event.event === "Finished") {
          setPercent(100);
        }
      });
      await relaunch();
    } catch (e) {
      setError(String((e as Error)?.message ?? e));
      setInstalling(false);
    }
  };

  if (!update) return null;

  return (
    <Dialog open={open} onOpenChange={(o) => !installing && setOpen(o)}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Sparkles className="h-5 w-5 text-primary" />
            Update available
          </DialogTitle>
          <DialogDescription>
            Snagreel {update.version} is available
            {update.currentVersion ? ` (you have ${update.currentVersion})` : ""}. Update
            now to get the latest features and fixes.
          </DialogDescription>
        </DialogHeader>

        {update.body && (
          <div className="max-h-40 overflow-y-auto whitespace-pre-line rounded-lg border bg-muted/40 p-3 text-sm text-muted-foreground">
            {update.body}
          </div>
        )}

        {installing && (
          <div className="space-y-1.5">
            <Progress value={percent} indeterminate={percent == null} />
            <p className="text-xs text-muted-foreground">
              Downloading… {percent != null ? `${Math.round(percent)}%` : ""} — the app will
              restart when it's done.
            </p>
          </div>
        )}
        {error && <p className="text-sm text-destructive">{error}</p>}

        <DialogFooter>
          <Button variant="ghost" onClick={() => setOpen(false)} disabled={installing}>
            Later
          </Button>
          <Button onClick={install} disabled={installing}>
            {installing ? (
              <>
                <Loader2 className="animate-spin" /> Updating…
              </>
            ) : (
              <>
                <Download /> Update now
              </>
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
