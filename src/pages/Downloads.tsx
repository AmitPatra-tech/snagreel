import { useEffect, useState } from "react";
import { Reorder } from "framer-motion";
import { Download as DownloadIcon, GripVertical } from "lucide-react";
import { DownloadCard } from "@/components/DownloadCard";
import { useDownloads } from "@/hooks/useDownloads";
import { api } from "@/services/api";
import type { Download } from "@/types";

export function DownloadsPage() {
  const { data: downloads, isLoading } = useDownloads();

  const active = (downloads ?? []).filter(
    (d) => d.status === "downloading" || d.status === "paused",
  );
  const queued = (downloads ?? []).filter((d) => d.status === "queued");
  const finished = (downloads ?? []).filter(
    (d) =>
      d.status === "completed" || d.status === "failed" || d.status === "cancelled",
  );

  // Local mirror of the queued list so drag-to-reorder feels instant.
  const [queueOrder, setQueueOrder] = useState<Download[]>(queued);
  useEffect(() => {
    setQueueOrder(queued);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [JSON.stringify(queued.map((d) => d.id))]);

  const commitOrder = (items: Download[]) => {
    setQueueOrder(items);
    api.reorderQueue(items.map((d) => d.id)).catch(() => {});
  };

  if (!isLoading && (downloads ?? []).length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-muted-foreground">
        <DownloadIcon className="h-10 w-10" />
        <p className="text-sm">No downloads yet. Paste a URL on the Home page.</p>
      </div>
    );
  }

  return (
    <div className="h-full space-y-6 overflow-y-auto p-6">
      <h1 className="text-xl font-semibold">Downloads</h1>

      {active.length > 0 && (
        <section className="space-y-3">
          <h2 className="text-sm font-semibold text-muted-foreground">Active</h2>
          {active.map((d) => (
            <DownloadCard key={d.id} download={d} />
          ))}
        </section>
      )}

      {queueOrder.length > 0 && (
        <section className="space-y-3">
          <h2 className="text-sm font-semibold text-muted-foreground">
            Queued · drag to reorder
          </h2>
          <Reorder.Group
            axis="y"
            values={queueOrder}
            onReorder={commitOrder}
            className="space-y-3"
          >
            {queueOrder.map((d) => (
              <Reorder.Item key={d.id} value={d} className="flex items-center gap-1">
                <GripVertical className="h-4 w-4 shrink-0 cursor-grab text-muted-foreground active:cursor-grabbing" />
                <div className="flex-1">
                  <DownloadCard download={d} />
                </div>
              </Reorder.Item>
            ))}
          </Reorder.Group>
        </section>
      )}

      {finished.length > 0 && (
        <section className="space-y-3">
          <h2 className="text-sm font-semibold text-muted-foreground">Finished</h2>
          {finished.map((d) => (
            <DownloadCard key={d.id} download={d} />
          ))}
        </section>
      )}
    </div>
  );
}
