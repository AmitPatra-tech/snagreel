import { AlertCircle, CheckCircle2, Info, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { useToastStore } from "@/lib/toast";

const ICONS = {
  error: AlertCircle,
  success: CheckCircle2,
  info: Info,
} as const;

export function Toaster() {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);

  if (toasts.length === 0) return null;

  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[100] flex w-80 max-w-[calc(100vw-2rem)] flex-col gap-2">
      {toasts.map((t) => {
        const Icon = ICONS[t.variant];
        return (
          <div
            key={t.id}
            className={cn(
              "pointer-events-auto flex items-start gap-2.5 rounded-lg border bg-card p-3 shadow-lg",
              t.variant === "error" && "border-destructive/40",
              t.variant === "success" && "border-emerald-500/40",
            )}
          >
            <Icon
              className={cn(
                "mt-0.5 h-4 w-4 shrink-0",
                t.variant === "error" && "text-destructive",
                t.variant === "success" && "text-emerald-500",
                t.variant === "info" && "text-muted-foreground",
              )}
            />
            <p className="flex-1 text-sm leading-snug">{t.message}</p>
            <button
              type="button"
              onClick={() => dismiss(t.id)}
              className="text-muted-foreground transition-colors hover:text-foreground"
              aria-label="Dismiss"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        );
      })}
    </div>
  );
}
