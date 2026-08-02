import { useState } from "react";
import { open as openExternal } from "@tauri-apps/plugin-shell";
import { BadgeCheck, Check, Copy, CreditCard, KeyRound, Loader2, Mail } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import {
  DODO_CHECKOUT_URL,
  PRO_FEATURES,
  PRO_SUPPORT_EMAIL,
} from "@/lib/pro";
import {
  useActivation,
  useActivatePro,
  useDeactivatePro,
} from "@/hooks/useActivation";

/**
 * Self-contained Pro activation panel: purchase steps, key entry, and current
 * status. Reused in Settings and in the upgrade dialog.
 */
export function ProActivation({ compact = false }: { compact?: boolean }) {
  const { data: activation } = useActivation();
  const activate = useActivatePro();
  const deactivate = useDeactivatePro();
  const [key, setKey] = useState("");
  const [copied, setCopied] = useState(false);

  const isPro = activation?.is_pro ?? false;

  const submit = () => {
    if (!key.trim() || activate.isPending) return;
    activate.mutate(key.trim(), { onSuccess: () => setKey("") });
  };

  const copyEmail = async () => {
    try {
      await navigator.clipboard.writeText(PRO_SUPPORT_EMAIL);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      /* clipboard may be unavailable; ignore */
    }
  };

  if (isPro) {
    return (
      <div className="space-y-3">
        <div className="flex items-center gap-2 rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-4 py-3 text-sm">
          <BadgeCheck className="h-5 w-5 text-emerald-500" />
          <div>
            <p className="font-medium text-emerald-600 dark:text-emerald-400">
              Pro is active
            </p>
            <p className="text-xs text-muted-foreground">
              All editing, clip and audio-extraction features are unlocked.
            </p>
          </div>
        </div>
        <Button
          variant="ghost"
          size="sm"
          className="text-muted-foreground"
          onClick={() => deactivate.mutate()}
          disabled={deactivate.isPending}
        >
          Remove activation from this device
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-5">
      {!compact && (
        <ul className="space-y-1.5">
          {PRO_FEATURES.map((f) => (
            <li key={f} className="flex items-start gap-2 text-sm text-muted-foreground">
              <Check className="mt-0.5 h-4 w-4 shrink-0 text-primary" />
              <span>{f}</span>
            </li>
          ))}
        </ul>
      )}

      <ol className="space-y-3">
        <Step n={1} icon={<CreditCard className="h-4 w-4" />} title="Pay with Dodo Payments">
          <Button
            size="sm"
            className="mt-1"
            onClick={() => openExternal(DODO_CHECKOUT_URL).catch(() => {})}
          >
            <CreditCard /> Open checkout
          </Button>
        </Step>
        <Step n={2} icon={<Mail className="h-4 w-4" />} title="Email your payment screenshot">
          <p className="text-xs text-muted-foreground">
            Send the payment screenshot to{" "}
            <button
              type="button"
              onClick={copyEmail}
              className="inline-flex items-center gap-1 font-medium text-foreground underline decoration-dotted underline-offset-2"
              title="Copy email"
            >
              {PRO_SUPPORT_EMAIL}
              {copied ? <Check className="h-3 w-3 text-emerald-500" /> : <Copy className="h-3 w-3" />}
            </button>
            . We will respond within 5 minutes with your activation key.
          </p>
        </Step>
        <Step n={3} icon={<KeyRound className="h-4 w-4" />} title="Enter your activation key">
          <div className="mt-1 flex gap-2">
            <Input
              value={key}
              onChange={(e) => setKey(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && submit()}
              placeholder="Paste your key here"
              spellCheck={false}
              autoComplete="off"
              className={cn(activate.isError && "border-destructive")}
            />
            <Button onClick={submit} disabled={!key.trim() || activate.isPending}>
              {activate.isPending ? <Loader2 className="animate-spin" /> : "Activate"}
            </Button>
          </div>
          {activate.isError && (
            <p className="mt-1.5 text-xs text-destructive">
              {String((activate.error as Error)?.message ?? activate.error)}
            </p>
          )}
        </Step>
      </ol>
    </div>
  );
}

function Step({
  n,
  icon,
  title,
  children,
}: {
  n: number;
  icon: React.ReactNode;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <li className="flex gap-3">
      <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-primary/15 text-primary">
        {icon}
      </div>
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium">
          <span className="text-muted-foreground">Step {n}.</span> {title}
        </p>
        {children}
      </div>
    </li>
  );
}
