import { Sparkles } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ProActivation } from "@/components/ProActivation";

/** Paywall shown when a free-tier user triggers a Pro-only feature. */
export function UpgradeProDialog({
  open,
  onClose,
  feature,
}: {
  open: boolean;
  onClose: () => void;
  feature?: string;
}) {
  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Sparkles className="h-5 w-5 text-primary" />
            {feature ? `${feature} is a Pro feature` : "Unlock Pro"}
          </DialogTitle>
          <DialogDescription>
            Get the full editing suite, clip downloads and audio extraction.
          </DialogDescription>
        </DialogHeader>
        <ProActivation />
      </DialogContent>
    </Dialog>
  );
}

/** Small inline "Pro" pill. */
export function ProBadge({ className = "" }: { className?: string }) {
  return (
    <span
      className={`inline-flex items-center gap-1 rounded-full bg-primary/15 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-primary ${className}`}
    >
      <Sparkles className="h-2.5 w-2.5" /> Pro
    </span>
  );
}
