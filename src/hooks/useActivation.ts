import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/services/api";
import type { ActivationState } from "@/types";

export function useActivation() {
  return useQuery({
    queryKey: ["activation"],
    queryFn: api.getActivation,
    staleTime: Infinity,
  });
}

/** Convenience: is the Pro tier active? */
export function useIsPro(): boolean {
  const { data } = useActivation();
  return data?.is_pro ?? false;
}

export function useActivatePro() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (key: string) => api.activatePro(key),
    onSuccess: (state: ActivationState) => {
      queryClient.setQueryData(["activation"], state);
    },
  });
}

export function useDeactivatePro() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => api.deactivatePro(),
    onSuccess: (state: ActivationState) => {
      queryClient.setQueryData(["activation"], state);
    },
  });
}
