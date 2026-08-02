import { useState } from "react";
import { Link } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { ArrowRight, FolderOpen, Library as LibraryIcon, Music, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { UrlBar } from "@/components/UrlBar";
import { useLibrary } from "@/hooks/useDownloads";
import { formatBytes, timeAgo } from "@/lib/format";
import { api } from "@/services/api";
import { openItem, toast } from "@/lib/toast";
import type { Download } from "@/types";

type Confirm = { mode: "one"; item: Download } | { mode: "all" };

export function HomePage() {
  const queryClient = useQueryClient();
  const [confirm, setConfirm] = useState<Confirm | null>(null);
  const [deleteFiles, setDeleteFiles] = useState(false);

  const { data: recent } = useLibrary({
    query: "",
    platform: null,
    kind: "all",
    sort: "date",
    descending: true,
    limit: 5,
  });

  const refresh = () => {
    queryClient.invalidateQueries({ queryKey: ["library"] });
    queryClient.invalidateQueries({ queryKey: ["downloads"] });
    queryClient.invalidateQueries({ queryKey: ["platforms"] });
  };

  const closeConfirm = () => {
    setConfirm(null);
    setDeleteFiles(false);
  };

  const runConfirm = async () => {
    if (!confirm) return;
    try {
      if (confirm.mode === "one") {
        await api.removeDownload(confirm.item.id, deleteFiles);
      } else {
        await api.clearHistory(deleteFiles);
      }
      closeConfirm();
      refresh();
      if (deleteFiles) toast.success("Removed, including the file(s) on disk.");
    } catch (e) {
      toast.error(String((e as Error)?.message ?? e));
    }
  };

  return (
    <div className="mx-auto flex h-full max-w-3xl flex-col gap-8 overflow-y-auto p-8">
      <div className="mt-10 text-center">
        <h1 className="text-3xl font-bold tracking-tight">
          Download from <span className="text-primary">thousands</span> of sites
        </h1>
        <p className="mt-2 text-sm text-muted-foreground">
          Paste a link to a video, song or playlist — pick a quality, and it lands in
          your library.
        </p>
      </div>

      <UrlBar autoFocus />

      <div className="flex flex-wrap justify-center gap-2">
        <Button variant="outline" size="sm" onClick={() => api.openDownloadFolder()}>
          <FolderOpen /> Open download folder
        </Button>
        <Button variant="outline" size="sm" asChild>
          <Link to="/library">
            <LibraryIcon /> Browse library
          </Link>
        </Button>
      </div>

      {recent && recent.length > 0 && (
        <div>
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-sm font-semibold text-muted-foreground">
              Recent downloads
            </h2>
            <div className="flex items-center gap-1">
              <Button
                variant="ghost"
                size="sm"
                className="text-muted-foreground"
                onClick={() => setConfirm({ mode: "all" })}
              >
                Clear all
              </Button>
              <Button variant="link" size="sm" asChild>
                <Link to="/library">
                  View all <ArrowRight />
                </Link>
              </Button>
            </div>
          </div>
          <Card>
            <CardContent className="divide-y p-0">
              {recent.map((item) => (
                <div
                  key={item.id}
                  className="group flex items-center gap-3 px-4 py-3 transition-colors hover:bg-accent/50"
                >
                  <button
                    type="button"
                    onClick={() => openItem(api.openFile, item.id)}
                    className="flex min-w-0 flex-1 items-center gap-3 text-left cursor-pointer"
                  >
                    <div className="h-10 w-16 shrink-0 overflow-hidden rounded bg-secondary">
                      {item.thumbnail ? (
                        <img src={item.thumbnail} alt="" className="h-full w-full object-cover" loading="lazy" />
                      ) : (
                        <div className="flex h-full items-center justify-center text-muted-foreground">
                          <Music className="h-4 w-4" />
                        </div>
                      )}
                    </div>
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm font-medium">{item.title}</p>
                      <p className="text-xs text-muted-foreground">
                        {item.platform} · {formatBytes(item.file_size)} ·{" "}
                        {timeAgo(item.completed_at)}
                      </p>
                    </div>
                  </button>
                  <button
                    type="button"
                    title="Remove from history"
                    aria-label="Remove from history"
                    onClick={() => setConfirm({ mode: "one", item })}
                    className="shrink-0 rounded-md p-1.5 text-muted-foreground opacity-0 transition-opacity hover:bg-accent hover:text-foreground group-hover:opacity-100"
                  >
                    <X className="h-4 w-4" />
                  </button>
                </div>
              ))}
            </CardContent>
          </Card>
        </div>
      )}

      <Dialog open={confirm !== null} onOpenChange={(o) => !o && closeConfirm()}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {confirm?.mode === "one"
                ? "Remove from history?"
                : "Clear download history?"}
            </DialogTitle>
            <DialogDescription>
              {confirm?.mode === "one"
                ? `“${confirm.item.title}” will be removed from your library list.`
                : "All items will be removed from your library list."}{" "}
              By default the downloaded file{confirm?.mode === "one" ? "" : "s"} stay on disk.
            </DialogDescription>
          </DialogHeader>
          <label className="flex cursor-pointer items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={deleteFiles}
              onChange={(e) => setDeleteFiles(e.target.checked)}
            />
            Also delete the file{confirm?.mode === "one" ? "" : "s"} from disk
          </label>
          <DialogFooter>
            <Button variant="ghost" onClick={closeConfirm}>
              Cancel
            </Button>
            <Button variant="destructive" onClick={runConfirm}>
              {confirm?.mode === "one" ? "Remove" : "Clear all"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
