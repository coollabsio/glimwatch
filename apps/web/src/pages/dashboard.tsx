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
      ? { tone: "neutral" as const, title: "No monitors yet", body: "Add a check and Owlee will start recording it." }
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

  const incidents = overview.data?.incidents ?? [];
  const activity = overview.data?.activity ?? [];

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Overview" description="Live checks, uptime, and the incidents customers can see." />
      <LayerCard title="System status" actions={<StatusBadge tone={summary.tone}>{summary.title}</StatusBadge>}>
        <p className="text-[13px] text-[var(--ui-fg-dim)]">{summary.body}</p>
      </LayerCard>
      {overview.error ? <p className="field-error">{(overview.error as Error).message}</p> : null}
      {monitors.length === 0 && !overview.isLoading ? (
        <Empty
          title="Nothing is being watched"
          body="Create an HTTP or TCP monitor. The scheduler probes it on the interval you set and opens an incident after repeated failures."
          action={<Link className="auth-text-link" to="/monitors">Create a monitor</Link>}
        />
      ) : (
        <LayerCard
          title="Monitors"
          description="30-day history"
          actions={<Link className="text-[11px] font-medium text-[var(--ui-fg-faint)] hover:text-[var(--ui-fg)]" to="/monitors">View all</Link>}
          flush
        >
          {overview.isLoading ? (
            <CardText title="Loading monitors…" />
          ) : (
            <div className="overflow-x-auto">
              <table className="cool-table">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>State</th>
                    <th className="hidden lg:table-cell">History</th>
                    <th className="hidden sm:table-cell">Today</th>
                    <th className="hidden md:table-cell">Latency</th>
                    <th>Last check</th>
                  </tr>
                </thead>
                <tbody>
                  {monitors.map((monitor) => {
                    const to = `/monitors/${monitor.id}`;
                    return (
                      <tr key={monitor.id} className="clickable-row">
                        <td className="cell-link max-w-[16rem]">
                          <Link className="row-link" to={to}>{monitor.name}</Link>
                          {rowCover(to)}
                          <div className="truncate text-[11px] text-[var(--ui-fg-faint)]">{monitor.url}</div>
                        </td>
                        <td className="cell-link">
                          {rowCover(to)}
                          <StatusBadge tone={tone(monitor.state)}>{stateLabel[monitor.state]}</StatusBadge>
                        </td>
                        <td className="cell-link hidden w-[30%] lg:table-cell">
                          {rowCover(to)}
                          <UptimeBars bars={monitor.bars} className="h-6" />
                        </td>
                        <td className="cell-link hidden sm:table-cell">
                          {rowCover(to)}
                          {percent(monitor.uptime_24h)}
                        </td>
                        <td className="cell-link hidden md:table-cell">
                          {rowCover(to)}
                          {monitor.avg_latency_ms == null ? "-" : `${monitor.avg_latency_ms} ms`}
                        </td>
                        <td className="cell-link whitespace-nowrap">
                          {rowCover(to)}
                          {timeAgo(monitor.last_checked_at)}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </LayerCard>
      )}
      <div className="grid items-start gap-6 lg:grid-cols-2">
        <LayerCard title="Open incidents" flush>
          {incidents.length === 0 ? (
            <CardText title="No open incidents" body="Incidents open here after repeated failures or when you report one." />
          ) : (
            <table className="cool-table">
              <tbody>
                {incidents.map((incident) => {
                  const to = `/incidents/${incident.id}`;
                  return (
                    <tr key={incident.id} className="clickable-row">
                      <td className="cell-link">
                        <Link className="row-link" to={to}>{incident.title}</Link>
                        {rowCover(to)}
                        <div className="text-[11px] text-[var(--ui-fg-faint)]">{formatTime(incident.started_at)}</div>
                      </td>
                      <td className="cell-link text-right">
                        {rowCover(to)}
                        <StatusBadge tone={tone(incident.status)}>{stateLabel[incident.status]}</StatusBadge>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </LayerCard>
        <LayerCard title="Activity" flush>
          {activity.length === 0 ? (
            <CardText title="No activity yet" body="Changes to monitors, incidents, and channels show up here." />
          ) : (
            <table className="cool-table">
              <tbody>
                {activity.map((event) => (
                  <tr key={event.id}>
                    <td>{event.action.replaceAll(".", " ")}</td>
                    <td className="whitespace-nowrap text-right text-[var(--ui-fg-faint)]">{timeAgo(event.occurred_at)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </LayerCard>
      </div>
    </div>
  );
}

function rowCover(to: string) {
  return <Link className="cell-link-anchor" to={to} tabIndex={-1} aria-hidden="true" />;
}

function CardText({ title, body }: { title: string; body?: string }) {
  return (
    <div className="flex min-h-32 flex-col items-center justify-center px-4 py-8 text-center">
      <p className="text-[14px] font-medium text-[var(--ui-fg)]">{title}</p>
      {body ? <p className="mx-auto mt-1 max-w-[28rem] text-[13px] text-[var(--ui-fg-faint)]">{body}</p> : null}
    </div>
  );
}
