import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router";
import { UptimeBars } from "@/components/charts";
import { Badge, Card, Empty } from "@/components/ui";
import { api, type Overview } from "@/lib/api";
import { formatTime, percent, stateLabel, stateTone, timeAgo } from "@/lib/utils";

export function DashboardPage() {
  const overview = useQuery({
    queryKey: ["overview"],
    queryFn: () => api<Overview>("/api/overview"),
    refetchInterval: 10_000,
  });
  const monitors = overview.data?.monitors ?? [];
  const down = monitors.filter((monitor) => monitor.state === "down").length;
  const degraded = monitors.filter((monitor) => monitor.state === "degraded").length;
  const banner =
    monitors.length === 0
      ? { tone: "bg-muted", title: "No monitors yet", body: "Add a check and CoolStatus will start recording it." }
      : down === monitors.length
        ? { tone: "bg-rose-500/15 text-rose-800 dark:text-rose-100", title: "Major outage", body: "Every monitor is down." }
        : down > 0
          ? {
              tone: "bg-amber-500/15 text-amber-950 dark:text-amber-50",
              title: "Partial outage",
              body: `${down} of ${monitors.length} monitors ${down === 1 ? "is" : "are"} down.`,
            }
          : degraded > 0
            ? { tone: "bg-amber-500/10 text-amber-950 dark:text-amber-50", title: "Degraded performance", body: "Some checks are slower than their threshold." }
            : { tone: "bg-emerald-500/15 text-emerald-950 dark:text-emerald-50", title: "All systems operational", body: `${monitors.length} monitors reporting.` };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Overview</h1>
        <p className="text-sm text-muted-foreground">Live checks, uptime, and the incidents customers can see.</p>
      </div>
      <div className={`rounded-2xl border px-5 py-6 ${banner.tone}`}>
        <div className="text-xl font-semibold">{banner.title}</div>
        <p className="mt-1 text-sm opacity-80">{banner.body}</p>
      </div>
      {overview.isLoading ? <p className="text-sm text-muted-foreground">Loading monitors…</p> : null}
      {overview.error ? <p className="text-sm text-destructive">{(overview.error as Error).message}</p> : null}
      {monitors.length === 0 && !overview.isLoading ? (
        <Empty
          title="Nothing is being watched"
          body="Create an HTTP or TCP monitor. The scheduler probes it on the interval you set and opens an incident after repeated failures."
          action={<Link className="text-sm font-medium text-primary" to="/monitors">Create a monitor</Link>}
        />
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {monitors.map((monitor) => (
            <Link key={monitor.id} to={`/monitors/${monitor.id}`}>
              <Card className="h-full p-4 transition hover:-translate-y-0.5">
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <div className="font-medium">{monitor.name}</div>
                    <div className="text-xs text-muted-foreground">{monitor.url}</div>
                  </div>
                  <Badge className={stateTone(monitor.state)}>{stateLabel[monitor.state]}</Badge>
                </div>
                <div className="mt-4 flex items-end justify-between text-sm">
                  <span className="text-muted-foreground">Today {percent(monitor.uptime_24h)}</span>
                  <span className="text-muted-foreground">{monitor.avg_latency_ms == null ? "—" : `${monitor.avg_latency_ms} ms`}</span>
                </div>
                <UptimeBars bars={monitor.bars} className="mt-2" />
                <div className="mt-2 flex justify-between text-xs text-muted-foreground">
                  <span>30-day history</span>
                  <span>Last check {timeAgo(monitor.last_checked_at)}</span>
                </div>
              </Card>
            </Link>
          ))}
        </div>
      )}
      <div className="grid gap-4 lg:grid-cols-2">
        <Card className="p-4">
          <h2 className="font-medium">Open incidents</h2>
          <div className="mt-3 space-y-3">
            {(overview.data?.incidents.length ?? 0) === 0 ? (
              <p className="text-sm text-muted-foreground">No open incidents.</p>
            ) : (
              overview.data?.incidents.map((incident) => (
                <Link key={incident.id} to={`/incidents/${incident.id}`} className="block rounded-lg border px-3 py-2">
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-medium">{incident.title}</span>
                    <Badge className={stateTone(incident.status)}>{stateLabel[incident.status]}</Badge>
                  </div>
                  <div className="mt-1 text-xs text-muted-foreground">{formatTime(incident.started_at)}</div>
                </Link>
              ))
            )}
          </div>
        </Card>
        <Card className="p-4">
          <h2 className="font-medium">Activity</h2>
          <ul className="mt-3 space-y-2 text-sm">
            {(overview.data?.activity.length ?? 0) === 0 ? <li className="text-muted-foreground">No activity yet.</li> : null}
            {overview.data?.activity.map((event) => (
              <li key={event.id} className="flex justify-between gap-3">
                <span>{event.action.replaceAll(".", " ")}</span>
                <span className="text-muted-foreground">{timeAgo(event.occurred_at)}</span>
              </li>
            ))}
          </ul>
        </Card>
      </div>
    </div>
  );
}
