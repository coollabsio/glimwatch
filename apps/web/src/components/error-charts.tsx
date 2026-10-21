export function Sparkline({ values }: { values: number[] }) {
  const width = 104;
  const height = 28;
  const max = Math.max(1, ...values);
  const step = values.length > 1 ? width / (values.length - 1) : width;
  const points = values
    .map((value, index) => {
      const x = index * step;
      const y = height - 2 - (value / max) * (height - 6);
      return `${index === 0 ? "M" : "L"}${x.toFixed(1)} ${y.toFixed(1)}`;
    })
    .join(" ");
  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} className="text-[var(--ui-accent)]" aria-hidden="true">
      <path d={points} fill="none" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}

export function Histogram({ values }: { values: number[] }) {
  const max = Math.max(1, ...values);
  return (
    <div className="flex h-24 items-end gap-px">
      {values.map((value, index) => (
        <div
          key={index}
          title={`${value}`}
          className="min-w-0 flex-1 rounded-sm bg-[var(--ui-accent)]"
          style={{ height: `${Math.max(4, (value / max) * 100)}%`, opacity: value === 0 ? 0.2 : 0.85 }}
        />
      ))}
    </div>
  );
}

export function PriorityMark({ priority }: { priority: string }) {
  const rank = priority === "high" ? 3 : priority === "medium" ? 2 : 1;
  return (
    <span className="inline-flex items-end gap-0.5 text-[var(--ui-fg-dim)]" title={priority}>
      {[1, 2, 3].map((bar) => (
        <span
          key={bar}
          className={`w-[3px] rounded-sm ${bar <= rank ? "bg-current" : "bg-[var(--ui-hover)]"}`}
          style={{ height: 4 + bar * 4 }}
        />
      ))}
    </span>
  );
}
