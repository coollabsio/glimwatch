import { cn } from "@/lib/utils";
import type { Bar, Check } from "@/lib/api";

const barColor: Record<string, string> = {
  up: "bg-emerald-500",
  degraded: "bg-amber-400",
  down: "bg-rose-500",
  maintenance: "bg-sky-400",
  empty: "bg-foreground/10",
};

export function UptimeBars({ bars, className }: { bars: Bar[]; className?: string }) {
  if (bars.length === 0) {
    return <div className={cn("h-8 rounded-md bg-muted", className)} />;
  }
  return (
    <div className={cn("flex h-8 items-stretch gap-px", className)} aria-hidden>
      {bars.map((bar) => {
        const ratio = bar.uptime_ratio == null ? "no checks" : `${(bar.uptime_ratio * 100).toFixed(2)}% up`;
        return (
          <div
            key={bar.day}
            title={`${bar.day}: ${bar.status}, ${ratio}, ${bar.total} samples`}
            className={cn("min-w-0 flex-1 rounded-[2px]", barColor[bar.status] ?? "bg-foreground/15")}
          />
        );
      })}
    </div>
  );
}

export function LatencyChart({ checks }: { checks: Check[] }) {
  const points = [...checks].reverse();
  if (points.length < 2) {
    return (
      <div className="flex h-40 items-center justify-center rounded-xl border border-dashed text-sm text-muted-foreground">
        Latency shows up after a couple of checks.
      </div>
    );
  }
  const width = 640;
  const height = 160;
  const pad = 12;
  const max = Math.max(...points.map((point) => point.latency_ms), 1);
  const coords = points.map((point, index) => {
    const x = pad + (index / (points.length - 1)) * (width - pad * 2);
    const y = height - pad - (point.latency_ms / max) * (height - pad * 2);
    return { x, y, failed: point.status === "failure" };
  });
  const line = coords.map((point) => `${point.x},${point.y}`).join(" ");
  return (
    <svg viewBox={`0 0 ${width} ${height}`} className="h-40 w-full" role="img" aria-label="Recent latency">
      <line x1={pad} y1={height - pad} x2={width - pad} y2={height - pad} className="stroke-border" />
      <polyline fill="none" stroke="currentColor" strokeWidth="2" className="text-primary" points={line} />
      {coords.map((point, index) => (
        <circle
          key={index}
          cx={point.x}
          cy={point.y}
          r={point.failed ? 3.5 : 2.2}
          className={point.failed ? "fill-rose-500" : "fill-primary"}
        />
      ))}
      <text x={pad} y={16} className="fill-muted-foreground text-[11px]">
        {max} ms
      </text>
    </svg>
  );
}
