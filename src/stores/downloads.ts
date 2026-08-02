import { create } from "zustand";
import type { ProgressPayload } from "@/types";

interface DownloadsState {
  progress: Record<number, ProgressPayload>;
  setProgress: (payload: ProgressPayload) => void;
  clearProgress: (id: number) => void;
}

export const useDownloadsStore = create<DownloadsState>((set) => ({
  progress: {},
  setProgress: (payload) =>
    set((state) => ({
      progress: { ...state.progress, [payload.id]: payload },
    })),
  clearProgress: (id) =>
    set((state) => {
      const next = { ...state.progress };
      delete next[id];
      return { progress: next };
    }),
}));
