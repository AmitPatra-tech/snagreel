import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/services/api";
import type { Settings } from "@/types";

export function useSettings() {
  return useQuery({
    queryKey: ["settings"],
    queryFn: api.getSettings,
    staleTime: Infinity,
  });
}

export function useUpdateSettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (settings: Settings) => api.updateSettings(settings),
    onSuccess: (_data, settings) => {
      queryClient.setQueryData(["settings"], settings);
    },
  });
}
