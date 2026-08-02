import * as React from "react";
import { cn } from "@/lib/utils";

interface ProgressProps extends React.HTMLAttributes<HTMLDivElement> {
  value?: number | null;
  indeterminate?: boolean;
}

const Progress = React.forwardRef<HTMLDivElement, ProgressProps>(
  ({ className, value, indeterminate, ...props }, ref) => (
    <div
      ref={ref}
      className={cn(
        "relative h-2 w-full overflow-hidden rounded-full bg-secondary",
        className,
      )}
      {...props}
    >
      {indeterminate ? (
        <div className="h-full w-1/3 animate-pulse rounded-full bg-primary" />
      ) : (
        <div
          className="h-full rounded-full bg-primary transition-all duration-300"
          style={{ width: `${Math.min(100, Math.max(0, value ?? 0))}%` }}
        />
      )}
    </div>
  ),
);
Progress.displayName = "Progress";

export { Progress };
