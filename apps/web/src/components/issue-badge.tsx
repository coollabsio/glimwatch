import type { HTMLAttributes } from "react";
import { cn } from "@/lib/utils";

const tones: Record<string, string> = {
  fatal: "bg-rose-500/15 text-rose-700 dark:text-rose-300",
  error: "bg-rose-500/15 text-rose-700 dark:text-rose-300",
  warning: "bg-amber-500/15 text-amber-800 dark:text-amber-200",
  info: "bg-sky-500/15 text-sky-800 dark:text-sky-200",
  debug: "bg-black/5 text-[var(--color-fg-dim)] dark:bg-white/5",
  unresolved: "bg-rose-500/10 text-rose-700 dark:text-rose-300",
  resolved: "bg-emerald-500/15 text-emerald-700 dark:text-emerald-300",
  ignored: "bg-black/5 text-[var(--color-fg-dim)] dark:bg-white/8",
  neutral: "bg-black/5 text-[var(--color-fg-dim)] dark:bg-white/8",
};

export function Badge({
  tone = "neutral",
  className,
  ...props
}: HTMLAttributes<HTMLSpanElement> & { tone?: string }) {
  return (
    <span
      className={cn(
        "inline-flex h-5 items-center rounded px-1.5 text-[11px] font-medium capitalize",
        tones[tone] ?? tones.neutral,
        className,
      )}
      {...props}
    />
  );
}
