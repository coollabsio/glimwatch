import type { HTMLAttributes } from "react";
import { cn } from "@/lib/utils";

/* Neutral status pill with a colored dot (same anatomy as StatusBadge). */
const tones: Record<string, string> = {
  fatal: "error",
  error: "error",
  unresolved: "error",
  warning: "warning",
  info: "info",
  resolved: "success",
};

export function Badge({
  tone = "neutral",
  className,
  children,
  ...props
}: HTMLAttributes<HTMLSpanElement> & { tone?: string }) {
  return (
    <span className={cn("status-badge", tones[tone] ?? "neutral", className)} {...props}>
      <i />
      <span className="truncate first-letter:uppercase">{children}</span>
    </span>
  );
}
