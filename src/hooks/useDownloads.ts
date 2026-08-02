import { useQuery } from "@tanstack/react-query";
import { api } from "@/services/api";
import type { LibraryQuery } from "@/types";

/** Queue + in-flight + recently finished items for the Downloads page. */
export function useDownloads() {
  return useQuery({
    queryKey: ["downloads"],
    queryFn: api.listDownloads,
    refetchInterval: 5000,
  });
}

export function useLibrary(query: LibraryQuery) {
  return useQuery({
    queryKey: ["library", query],
    queryFn: () => api.searchLibrary(query),
  });
}

export function usePlatforms() {
  return useQuery({
    queryKey: ["platforms"],
    queryFn: api.listPlatforms,
  });
}
