import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router";
import { UptimeBars } from "@/components/charts";
import { Empty, LayerCard, PageHeader, StatusBadge } from "@/components/ui";
import { api, type Overview } from "@/lib/api";
import { formatTime, percent, stateLabel, timeAgo } from "@/lib/utils";

function tone(state: string): "success" | "warning" | "error" | "neutral" | "info" {
  if (state === "up" || state === "resolved" || state === "success") return "success";
  if (state === "degraded" || state === "minor" || state === "partial_outage") return "warning";
  if (state === "down" || state === "failure" || state === "major" || state === "critical") return "error";
  if (state === "identified" || state === "monitoring" || state === "investigating") return "info";
  return "neutral";
}

export function DashboardPage() {
  const overview = useQuery({
    queryKey: ["overview"],
    queryFn: () => api<Overview>("/api/overview"),
    refetchInterval: 10_000,
  });
  const monitors = overview.data?.monitors ?? [];
  const down = monitors.filter((monitor) => monitor.state === "down").length;
  const degraded = monitors.filter((monitor) => monitor.state === "degraded").length;
  const summary =
    monitors.length === 0
      ? { tone: "neutral" as const, title: "No monitors yet", body: "Add a check and CoolStatus will start recording it." }
      : down === monitors.length
        ? { tone: "error" as const, title: "Major outage", body: "Every monitor is down." }
        : down > 0
          ? {
              tone: "warning" as const,
              title: "Partial outage",
              body: `${down} of ${monitors.length} monitors ${down === 1 ? "is" : "are"} down.`,
            }
          : degraded > 0
            ? { tone: "warning" as const, title: "Degraded performance", body: "Some checks are slower than their threshold." }
            : { tone: "success" as const, title: "All systems operational", body: `${monitors.length} monitors reporting.` };

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Overview" description="Live checks, uptime, and the incidents customers can see." />
      <LayerCard
        title="System status"
        actions={<StatusBadge tone={summary.tone}>{summary.title}</StatusBadge>}
      >
        <p className="text-[13px] text-[var(--color-fg-dim)]">{summary.body}</p>
      </LayerCard>
      {overview.isLoading ? <p className="text-[13px] text-[var(--coollabs-subtle)]">Loading monitors…</p> : null}
      {overview.error ? <p className="field-error">{(overview.error as Error).message}</p> : null}
      {monitors.length === 0 && !overview.isLoading ? (
        <Empty
          title="Nothing is being watched"
          body="Create an HTTP or TCP monitor. The scheduler probes it on the interval you set and opens an incident after repeated failures."
          action={<Link className="auth-text-link" to="/monitors">Create a monitor</Link>}
        />
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {monitors.map((monitor) => (
            <Link key={monitor.id} to={`/monitors/${monitor.id}`}>
              <LayerCard
                title={monitor.name}
                description={monitor.url}
                actions={<StatusBadge tone={tone(monitor.state)}>{stateLabel[monitor.state]}</StatusBadge>}
              >
                <div className="mb-2 flex items-end justify-between text-[13px] text-[var(--coollabs-subtle)]">
                  <span>Today {percent(monitor.uptime_24h)}</span>
                  <span>{monitor.avg_latency_ms == null ? "-" : `${monitor.avg_latency_ms} ms`}</span>
                </div>
                <UptimeBars bars={monitor.bars} />
                <div className="mt-2 flex justify-between text-[11px] text-[var(--coollabs-subtle)]">
                  <span>30-day history</span>
                  <span>Last check {timeAgo(monitor.last_checked_at)}</span>
                </div>
              </LayerCard>
            </Link>
          ))}
        </div>
      )}
      <div className="grid gap-4 lg:grid-cols-2">
        <LayerCard title="Open incidents">
          <div className="space-y-2">
            {(overview.data?.incidents.length ?? 0) === 0 ? (
              <p className="text-[13px] text-[var(--coollabs-subtle)]">No open incidents.</p>
            ) : (
              overview.data?.incidents.map((incident) => (
                <Link key={incident.id} to={`/incidents/${incident.id}`} className="flex items-center justify-between gap-3 rounded-md px-2 py-2 hover:bg-black/[0.03] dark:hover:bg-white/[0.04]">
                  <span>
                    <span className="block text-[13px] font-medium text-[var(--color-fg)]">{incident.title}</span>
                    <span className="text-[11px] text-[var(--coollabs-subtle)]">{formatTime(incident.started_at)}</span>
                  </span>
                  <StatusBadge tone={tone(incident.status)}>{stateLabel[incident.status]}</StatusBadge>
                </Link>
              ))
            )}
          </div>
        </LayerCard>
        <LayerCard title="Activity">
          <ul className="space-y-2 text-[13px]">
            {(overview.data?.activity.length ?? 0) === 0 ? <li className="text-[var(--coollabs-subtle)]">No activity yet.</li> : null}
            {overview.data?.activity.map((event) => (
              <li key={event.id} className="flex justify-between gap-3">
                <span className="text-[var(--color-fg-dim)]">{event.action.replaceAll(".", " ")}</span>
                <span className="text-[var(--coollabs-subtle)]">{timeAgo(event.occurred_at)}</span>
              </li>
            ))}
          </ul>
        </LayerCard>
      </div>
    </div>
  );
}
