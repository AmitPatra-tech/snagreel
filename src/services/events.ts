import { listen } from "@tauri-apps/api/event";
import type { QueryClient } from "@tanstack/react-query";
import { useDownloadsStore } from "@/stores/downloads";
import type { ProgressPayload, StatusPayload } from "@/types";

/** Attach global Tauri event listeners once at app startup. */
export function attachDownloadListeners(queryClient: QueryClient) {
  const unlisteners: Promise<() => void>[] = [];

  unlisteners.push(
    listen<ProgressPayload>("download-progress", (event) => {
      useDownloadsStore.getState().setProgress(event.payload);
    }),
  );

  unlisteners.push(
    listen<StatusPayload>("download-status", (event) => {
      const { id, status } = event.payload;
      if (status !== "downloading") {
        useDownloadsStore.getState().clearProgress(id);
      }
      queryClient.invalidateQueries({ queryKey: ["downloads"] });
      if (status === "completed") {
        queryClient.invalidateQueries({ queryKey: ["library"] });
        queryClient.invalidateQueries({ queryKey: ["platforms"] });
      }
    }),
  );

  unlisteners.push(
    listen("queue-updated", () => {
      queryClient.invalidateQueries({ queryKey: ["downloads"] });
    }),
  );

  return () => {
    unlisteners.forEach((p) => p.then((unlisten) => unlisten()));
  };
}
