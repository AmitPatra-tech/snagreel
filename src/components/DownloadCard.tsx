import {
  FolderOpen,
  Music,
  Pause,
  Play,
  RotateCcw,
  Trash2,
  X,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { formatBytes, formatEta, formatSpeed, timeAgo } from "@/lib/format";
import { api } from "@/services/api";
import { useDownloadsStore } from "@/stores/downloads";
import type { Download, DownloadStatus } from "@/types";

const statusVariant: Record<
  DownloadStatus,
  "default" | "secondary" | "destructive" | "success" | "warning"
> = {
  queued: "secondary",
  downloading: "default",
  paused: "warning",
  completed: "success",
  failed: "destructive",
  cancelled: "secondary",
};

export function DownloadCard({ download }: { download: Download }) {
  const progress = useDownloadsStore((s) => s.progress[download.id]);
  const { id, status } = download;

  const percent =
    status === "completed"
      ? 100
      : progress?.percent ??
        (progress?.total_bytes
          ? (progress.downloaded_bytes / progress.total_bytes) * 100
          : null);

  return (
    <div className="flex gap-4 rounded-xl border bg-card p-4">
      <div className="relative h-20 w-32 shrink-0 overflow-hidden rounded-lg bg-secondary">
        {download.thumbnail ? (
          <img
            src={download.thumbnail}
            alt=""
            className="h-full w-full object-cover"
            loading="lazy"
          />
        ) : (
          <div className="flex h-full items-center justify-center text-muted-foreground">
            <Music className="h-6 w-6" />
          </div>
        )}
      </div>

      <div className="flex min-w-0 flex-1 flex-col gap-2">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0">
            <p className="truncate text-sm font-medium" title={download.title}>
              {download.title}
            </p>
            <div className="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
              <Badge variant="secondary" className="px-1.5 py-0">
                {download.platform}
              </Badge>
              <span>{download.format.toUpperCase()}</span>
              {download.resolution && <span>{download.resolution}</span>}
              <span>{timeAgo(download.created_at)}</span>
            </div>
          </div>
          <Badge variant={statusVariant[status]} className="shrink-0 capitalize">
            {status}
          </Badge>
        </div>

        {(status === "downloading" || status === "paused" || status === "queued") && (
          <div className="space-y-1">
            <Progress
              value={percent}
              indeterminate={status === "downloading" && percent == null}
            />
            <div className="flex justify-between text-xs text-muted-foreground">
              <span>
                {formatBytes(progress?.downloaded_bytes)}
                {progress?.total_bytes ? ` / ${formatBytes(progress.total_bytes)}` : ""}
                {percent != null ? ` (${percent.toFixed(1)}%)` : ""}
              </span>
              {status === "downloading" && (
                <span>
                  {formatSpeed(progress?.speed)} · ETA {formatEta(progress?.eta)}
                </span>
              )}
            </div>
          </div>
        )}

        {status === "failed" && download.error && (
          <p className="line-clamp-2 text-xs text-destructive" title={download.error}>
            {download.error}
          </p>
        )}

        <div className="mt-auto flex items-center gap-1">
          {status === "downloading" && (
            <>
              <Button size="icon-sm" variant="ghost" title="Pause" onClick={() => api.pauseDownload(id)}>
                <Pause />
              </Button>
              <Button size="icon-sm" variant="ghost" title="Cancel" onClick={() => api.cancelDownload(id)}>
                <X />
              </Button>
            </>
          )}
          {status === "paused" && (
            <>
              <Button size="icon-sm" variant="ghost" title="Resume" onClick={() => api.resumeDownload(id)}>
                <Play />
              </Button>
              <Button size="icon-sm" variant="ghost" title="Cancel" onClick={() => api.cancelDownload(id)}>
                <X />
              </Button>
            </>
          )}
          {status === "queued" && (
            <Button size="icon-sm" variant="ghost" title="Cancel" onClick={() => api.cancelDownload(id)}>
              <X />
            </Button>
          )}
          {(status === "failed" || status === "cancelled") && (
            <Button size="icon-sm" variant="ghost" title="Retry" onClick={() => api.retryDownload(id)}>
              <RotateCcw />
            </Button>
          )}
          {status === "completed" && (
            <Button size="icon-sm" variant="ghost" title="Show in folder" onClick={() => api.showInFolder(id)}>
              <FolderOpen />
            </Button>
          )}
          {status !== "downloading" && (
            <Button
              size="icon-sm"
              variant="ghost"
              title="Remove"
              className="text-muted-foreground hover:text-destructive"
              onClick={() => api.removeDownload(id, false)}
            >
              <Trash2 />
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
