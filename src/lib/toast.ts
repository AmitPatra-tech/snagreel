import { create } from "zustand";

export type ToastVariant = "error" | "success" | "info";

export interface Toast {
  id: number;
  message: string;
  variant: ToastVariant;
}

interface ToastState {
  toasts: Toast[];
  push: (message: string, variant?: ToastVariant) => void;
  dismiss: (id: number) => void;
}

export const useToastStore = create<ToastState>((set) => ({
  toasts: [],
  push: (message, variant = "info") => {
    const id = Date.now() + Math.random();
    set((s) => ({ toasts: [...s.toasts, { id, message, variant }] }));
    setTimeout(() => {
      set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
    }, 4000);
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

/** Fire a toast from anywhere (components or plain functions). */
export const toast = {
  error: (m: string) => useToastStore.getState().push(m, "error"),
  success: (m: string) => useToastStore.getState().push(m, "success"),
  info: (m: string) => useToastStore.getState().push(m, "info"),
};

/** Open a library item, surfacing a toast if the file is gone. */
export async function openItem(
  open: (id: number) => Promise<void>,
  id: number,
) {
  try {
    await open(id);
  } catch (e) {
    toast.error(String((e as Error)?.message ?? e));
  }
}
