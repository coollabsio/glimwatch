import { cn } from "@/lib/utils";
import type { Bar, Check, InsightRow } from "@/lib/api";
import { chartTime } from "@/lib/theme";

const barColor: Record<string, string> = {
  up: "bg-[var(--ui-success)]",
  degraded: "bg-[var(--ui-warning)]",
  down: "bg-[var(--ui-error)]",
  maintenance: "bg-[var(--ui-accent)]",
  empty: "bg-[var(--ui-divider)]",
};

export function UptimeBars({ bars, className }: { bars: Bar[]; className?: string }) {
  if (bars.length === 0) {
    return <div className={cn("h-8 rounded-md bg-[var(--ui-divider)]", className)} />;
  }
  return (
    <div className={cn("flex h-8 items-stretch gap-px", className)} aria-hidden>
      {bars.map((bar) => {
        const ratio = bar.uptime_ratio == null ? "no checks" : `${(bar.uptime_ratio * 100).toFixed(2)}% up`;
        return (
          <div
            key={bar.day}
            title={`${bar.day}: ${bar.status}, ${ratio}, ${bar.total} samples`}
            className={cn("min-w-0 flex-1 rounded-[2px]", barColor[bar.status] ?? "bg-[var(--ui-divider)]")}
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
      <div className="flex h-40 items-center justify-center text-[13px] text-[var(--ui-fg-faint)]">
        Latency shows up after a couple of checks.
      </div>
    );
  }
  const width = 640;
  const height = 160;
  const pad = 12;
  const max = Math.max(...points.map((point) => point.latency_ms), 1) * 1.2;
  const coords = points.map((point, index) => {
    const x = pad + (index / (points.length - 1)) * (width - pad * 2);
    const y = height - pad - (point.latency_ms / max) * (height - pad * 2);
    return { x, y, failed: point.status === "failure", title: `${chartTime(point.checked_at)}\n${point.latency_ms} ms` };
  });
  const line = coords.map((point) => `${point.x},${point.y}`).join(" ");
  const area = `${pad},${height - pad} ${line} ${width - pad},${height - pad}`;
  return (
    <svg viewBox={`0 0 ${width} ${height}`} className="h-40 w-full" role="img" aria-label="Recent latency">
      <defs>
        <linearGradient id="latency-fill" x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stopColor="var(--ui-accent)" stopOpacity="0.18" />
          <stop offset="100%" stopColor="var(--ui-accent)" stopOpacity="0" />
        </linearGradient>
      </defs>
      {[0.25, 0.5, 0.75].map((ratio) => (
        <line key={ratio} x1={pad} x2={width - pad} y1={pad + ratio * (height - pad * 2)} y2={pad + ratio * (height - pad * 2)} stroke="var(--ui-divider)" strokeDasharray="3 4" />
      ))}
      <line x1={pad} y1={height - pad} x2={width - pad} y2={height - pad} stroke="var(--ui-divider)" />
      <polygon points={area} fill="url(#latency-fill)" />
      <polyline fill="none" stroke="var(--ui-accent)" strokeWidth="2" strokeLinejoin="round" strokeLinecap="round" points={line} />
      {coords.map((point, index) => (
        <circle
          key={index}
          cx={point.x}
          cy={point.y}
          r={point.failed ? 3.5 : 2.2}
          fill={point.failed ? "var(--ui-error)" : "var(--ui-accent)"}
        >
          <title>{point.title}</title>
        </circle>
      ))}
      <text x={pad} y={pad + 10} fill="var(--ui-fg-faint)" className="text-[11px]">
        {Math.round(max)} ms
      </text>
    </svg>
  );
}

function day(seconds: number) {
  return new Date(seconds * 1000).toISOString().slice(0, 10);
}

/* Daily counts as accent bars over a dashed grid, with the first and last day below. */
export function DailyBars({ series, field }: { series: { day: number; views?: number; clicks?: number }[]; field: "views" | "clicks" }) {
  if (series.length === 0) {
    return <p className="py-10 text-center text-[13px] text-[var(--ui-fg-faint)]">No data in this range.</p>;
  }
  const values = series.map((point) => (field === "views" ? point.views : point.clicks) ?? 0);
  const max = Math.max(...values, 1);
  return (
    <div>
      <div className="mb-2 flex justify-between font-mono text-[10px] text-[var(--ui-fg-faint)]">
        <span>{max}</span>
        <span>{values.reduce((sum, value) => sum + value, 0)} total</span>
      </div>
      <div className="relative h-40">
        <div className="pointer-events-none absolute inset-0 flex flex-col justify-between" aria-hidden>
          {[0, 1, 2, 3].map((line) => <div key={line} className="border-t border-dashed border-[var(--ui-divider)]" />)}
        </div>
        <div className="relative flex h-full items-end gap-[3px] border-b border-[var(--ui-divider)]" aria-label="Daily counts">
          {series.map((point, index) => (
            <div
              key={point.day}
              title={`${day(point.day)}: ${values[index]}`}
              className="min-w-0 flex-1 rounded-t-[3px] bg-[var(--ui-accent)] opacity-85 transition-opacity hover:opacity-100"
              style={{ height: values[index] === 0 ? "0%" : `${Math.max(4, (values[index] / max) * 100)}%` }}
            />
          ))}
        </div>
      </div>
      <div className="mt-2 flex justify-between font-mono text-[10px] text-[var(--ui-fg-faint)]">
        <span>{day(series[0].day)}</span>
        <span>{day(series[series.length - 1].day)}</span>
      </div>
    </div>
  );
}

/* Breakdown rows with a faint accent fill sized by share. */
/* `total` lets a cut list (top N) keep percentages of the full list. */
export function BarList({ rows, total: fullTotal, empty = "Nothing in this range." }: { rows: InsightRow[]; total?: number; empty?: string }) {
  if (rows.length === 0) {
    return <p className="px-4 py-10 text-center text-[13px] text-[var(--ui-fg-faint)]">{empty}</p>;
  }
  const max = Math.max(...rows.map((row) => row.count), 1);
  const total = fullTotal || rows.reduce((sum, row) => sum + row.count, 0) || 1;
  return (
    <ul className="flex flex-col gap-0.5 p-2">
      {rows.map((row) => (
        <li
          key={row.label}
          className="relative flex h-[30px] items-center gap-3 overflow-hidden rounded-[6px] px-2.5"
          title={row.visitors != null ? `${row.label}: ${row.count}, ${row.visitors} visitors` : `${row.label}: ${row.count}`}
        >
          <span
            className="absolute inset-y-0 left-0 rounded-[6px] bg-[color-mix(in_oklab,var(--ui-accent)_16%,transparent)]"
            style={{ width: `${(row.count / max) * 100}%` }}
            aria-hidden
          />
          <span className="relative min-w-0 flex-1 truncate text-[12px] font-medium text-[var(--ui-fg-dim)]">{row.label || "-"}</span>
          <span className="relative font-mono text-[11px] text-[var(--ui-fg-dim)]">{row.count}</span>
          <span className="relative w-9 text-right font-mono text-[10px] text-[var(--ui-fg-faint)]">{Math.round((row.count / total) * 100)}%</span>
        </li>
      ))}
    </ul>
  );
}

/* Big value with a small faint label; put a row of these in one card. */
export function Stat({ label, value }: { label: string; value: string | number | undefined }) {
  return (
    <div className="min-w-0">
      <p className="text-[11px] font-medium text-[var(--ui-fg-faint)]">{label}</p>
      <p className="mt-1 text-[24px] leading-8 font-semibold tracking-tight text-[var(--ui-fg)]">{value ?? "-"}</p>
    </div>
  );
}
