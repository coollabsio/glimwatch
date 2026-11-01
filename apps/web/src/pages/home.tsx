import { useQuery } from "@tanstack/react-query";
import { type ReactNode } from "react";
import { Link } from "react-router";
import { Sparkline } from "@/components/error-charts";
import { ErrorText, LayerCard, PageHeader, StatusBadge } from "@/components/ui";
import { api, type Dashboard, type DashboardIncident, type DashboardPageRow } from "@/lib/api";
import { duration, percent, stateLabel, timeAgo } from "@/lib/utils";

function tone(state: string): "success" | "warning" | "error" | "neutral" | "info" {
  if (state === "up" || state === "operational" || state === "resolved") return "success";
  if (state === "degraded" || state === "partial_outage" || state === "minor") return "warning";
  if (state === "down" || state === "major_outage" || state === "major" || state === "critical" || state === "fatal") return "error";
  if (state === "investigating" || state === "identified" || state === "monitoring") return "info";
  return "neutral";
}

function isFresh(data: Dashboard) {
  const count = data.monitors.up + data.monitors.down + data.monitors.degraded + data.monitors.pending + data.monitors.paused;
  return count === 0 && data.pages.length === 0 && data.incidents.open === 0 && data.errors.projects === 0 && data.analytics.sites === 0 && data.analytics.links === 0;
}

export function HomePage() {
  const home = useQuery({
    queryKey: ["dashboard"],
    queryFn: () => api<Dashboard>("/api/dashboard"),
    refetchInterval: 15_000,
  });
  const data = home.data;

  if (!data) {
    return (
      <div className="flex flex-col gap-6">
        <PageHeader title="Dashboard" description="Monitors, incidents, errors, and traffic." />
        {home.error ? <ErrorText error={home.error} /> : <LayerCard><p className="text-[13px] text-[var(--ui-fg-faint)]">Loading the overview…</p></LayerCard>}
      </div>
    );
  }

  if (isFresh(data)) {
    return (
      <div className="flex flex-col gap-6">
        <PageHeader title="Dashboard" description="Monitors, incidents, errors, and traffic." />
        <LayerCard>
          <div className="flex min-h-48 flex-col items-center justify-center px-2 py-8 text-center">
            <h2 className="text-[14px] font-medium text-[var(--ui-fg)]">Nothing to report yet</h2>
            <p className="mx-auto mt-1 max-w-[28rem] text-[13px] text-[var(--ui-fg-faint)]">
              Create a monitor, a site, or an error project. This page fills in as soon as one of them has data.
            </p>
            <div className="mt-4 flex flex-wrap items-center justify-center gap-2">
              <Link className="button button-highlighted" to="/monitors">Create a monitor</Link>
              <Link className="button" to="/analytics">Add a site</Link>
              <Link className="button" to="/error-projects">Create a project</Link>
            </div>
          </div>
        </LayerCard>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Dashboard" description="Monitors, incidents, errors, and traffic." />
      <ErrorText error={home.error} />
      <QuotaNotice hits={data.quota_hits} />
      <MonitorsCard data={data} />
      <div className="grid items-start gap-6 lg:grid-cols-2">
        <IncidentsCard incidents={data.incidents} />
        <PagesCard pages={data.pages} />
      </div>
      <div className="grid items-start gap-6 lg:grid-cols-2">
        <ErrorsCard errors={data.errors} />
        <TrafficCard analytics={data.analytics} />
      </div>
    </div>
  );
}

function resetWhen(iso: string) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "00:00 UTC";
  return `${date.toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", timeZone: "UTC" })} UTC`;
}

function QuotaNotice({ hits }: { hits: Dashboard["quota_hits"] }) {
  if (hits.length === 0) return null;
  return (
    <LayerCard title="Ingest stopped" description="A project or site reached its daily cap. New events are rejected until the counter resets.">
      <ul className="space-y-2">
        {hits.map((hit) => {
          const to = hit.kind === "error" ? `/error-projects/${hit.id}` : `/analytics/${hit.id}`;
          const noun = hit.kind === "error" ? "errors" : "events";
          return (
            <li key={`${hit.kind}-${hit.id}`} className="flex flex-wrap items-baseline gap-x-2 text-[13px]">
              <StatusBadge tone="error">Cap reached</StatusBadge>
              <Link className="font-medium text-[var(--ui-fg)] hover:underline" to={to}>{hit.name}</Link>
              <span className="text-[var(--ui-fg-faint)]">
                {hit.used.toLocaleString()} / {hit.quota.toLocaleString()} {noun}. Resets {resetWhen(hit.resets_at)}.
              </span>
            </li>
          );
        })}
      </ul>
    </LayerCard>
  );
}

function viewAll(to: string, label = "View all") {
  return <Link className="text-[11px] font-medium text-[var(--ui-fg-faint)] hover:text-[var(--ui-fg)]" to={to}>{label}</Link>;
}

function rowCover(to: string) {
  return <Link className="cell-link-anchor" to={to} tabIndex={-1} aria-hidden="true" />;
}

function Note({ children }: { children: ReactNode }) {
  return <p className="px-4 py-8 text-center text-[13px] text-[var(--ui-fg-faint)]">{children}</p>;
}

function Metric({ label, value, previous, emphasize }: { label: string; value: string; previous?: { current: number; previous: number }; emphasize?: boolean }) {
  return (
    <div className="min-w-0">
      <p className="text-[11px] font-medium text-[var(--ui-fg-faint)]">{label}</p>
      <p className={emphasize ? "mt-1 text-[24px] leading-8 font-semibold tracking-tight text-[var(--ui-error)]" : "mt-1 text-[24px] leading-8 font-semibold tracking-tight text-[var(--ui-fg)]"}>{value}</p>
      {previous ? (
        <p className="mt-0.5 text-[12px] font-medium">
          <Delta current={previous.current} previous={previous.previous} />
        </p>
      ) : null}
    </div>
  );
}

function Delta({ current, previous }: { current: number; previous: number }) {
  if (current === 0 && previous === 0) return null;
  const label = previous === 0 ? "new" : `${current > previous ? "+" : ""}${Math.round(((current - previous) / previous) * 100)}%`;
  const className = current > previous ? "text-[var(--ui-success)]" : current < previous ? "text-[var(--ui-error)]" : "text-[var(--ui-fg-faint)]";
  return <span className={`text-[12px] font-medium ${className}`}>{label}</span>;
}

function summary(data: Dashboard) {
  const { up, down, degraded, pending, paused } = data.monitors;
  const active = up + down + degraded + pending;
  if (active === 0 && paused === 0) return { tone: "neutral" as const, title: "No monitors" };
  if (down > 0 && down === active) return { tone: "error" as const, title: "Major outage" };
  if (down > 0) return { tone: "warning" as const, title: "Partial outage" };
  if (degraded > 0) return { tone: "warning" as const, title: "Degraded" };
  if (pending > 0 && up === 0) return { tone: "neutral" as const, title: "Waiting for checks" };
  return { tone: "success" as const, title: "All up" };
}

function MonitorsCard({ data }: { data: Dashboard }) {
  const { monitors } = data;
  const count = monitors.up + monitors.down + monitors.degraded + monitors.pending + monitors.paused;
  const failing = monitors.down + monitors.degraded;
  const status = summary(data);
  const extra = [
    monitors.pending > 0 ? `${monitors.pending} pending` : null,
    monitors.paused > 0 ? `${monitors.paused} paused` : null,
  ].filter(Boolean);
  return (
    <LayerCard
      title="Monitors"
      description={failing > monitors.failing.length ? `Showing ${monitors.failing.length} of ${failing} failing` : undefined}
      actions={<><StatusBadge tone={status.tone}>{status.title}</StatusBadge>{viewAll("/monitors")}</>}
      flush
    >
      {count === 0 ? (
        <Note>No monitors yet. <Link className="auth-text-link" to="/monitors">Create a monitor</Link></Note>
      ) : (
        <>
          <div className="grid grid-cols-2 gap-6 border-b border-[var(--ui-divider)] px-4 py-4 sm:grid-cols-3 xl:grid-cols-5">
            <Metric label="Up" value={String(monitors.up)} />
            <Metric label="Down" value={String(monitors.down)} emphasize={monitors.down > 0} />
            <Metric label="Degraded" value={String(monitors.degraded)} />
            <Metric label="Uptime today" value={percent(monitors.uptime_today)} />
            <Metric label="Response today" value={monitors.avg_latency_ms == null ? "-" : `${monitors.avg_latency_ms} ms`} />
          </div>
          {extra.length > 0 ? <p className="border-b border-[var(--ui-divider)] px-4 py-2 text-[12px] text-[var(--ui-fg-faint)]">{extra.join(" · ")}</p> : null}
          {monitors.failing.length === 0 ? (
            <Note>Nothing is failing.</Note>
          ) : (
            <div className="overflow-x-auto">
              <table className="cool-table">
                <thead>
                  <tr>
                    <th>Failing</th>
                    <th>State</th>
                    <th>For</th>
                    <th className="hidden sm:table-cell">Today</th>
                    <th className="hidden md:table-cell">Latency</th>
                  </tr>
                </thead>
                <tbody>
                  {monitors.failing.map((monitor) => {
                    const to = `/monitors/${monitor.id}`;
                    return (
                      <tr key={monitor.id} className="clickable-row">
                        <td className="cell-link max-w-[16rem]">
                          <Link className="row-link" to={to}>{monitor.name}</Link>
                          {rowCover(to)}
                          <div className="truncate text-[11px] text-[var(--ui-fg-faint)]">{monitor.url}</div>
                        </td>
                        <td className="cell-link">{rowCover(to)}<StatusBadge tone={tone(monitor.state)}>{stateLabel[monitor.state] ?? monitor.state}</StatusBadge></td>
                        <td className="cell-link whitespace-nowrap">{rowCover(to)}{duration(monitor.since)}</td>
                        <td className="cell-link hidden sm:table-cell">{rowCover(to)}{percent(monitor.uptime_today)}</td>
                        <td className="cell-link hidden md:table-cell">{rowCover(to)}{monitor.avg_latency_ms == null ? "-" : `${monitor.avg_latency_ms} ms`}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </>
      )}
    </LayerCard>
  );
}

function IncidentsCard({ incidents }: { incidents: Dashboard["incidents"] }) {
  return (
    <LayerCard title="Open incidents" description={incidents.open > 0 ? `${incidents.open} open` : undefined} actions={viewAll("/incidents")} flush>
      {incidents.items.length === 0 ? (
        <Note>No open incidents.</Note>
      ) : (
        <table className="cool-table">
          <tbody>
            {incidents.items.map((incident) => <IncidentRow key={incident.id} incident={incident} />)}
          </tbody>
        </table>
      )}
    </LayerCard>
  );
}

function IncidentRow({ incident }: { incident: DashboardIncident }) {
  const to = `/incidents/${incident.id}`;
  return (
    <tr className="clickable-row">
      <td className="cell-link">
        <Link className="row-link" to={to}>{incident.title}</Link>
        {rowCover(to)}
        <div className="text-[11px] text-[var(--ui-fg-faint)]">{stateLabel[incident.impact] ?? incident.impact} · {duration(incident.started_at)}</div>
      </td>
      <td className="cell-link text-right">{rowCover(to)}<StatusBadge tone={tone(incident.status)}>{stateLabel[incident.status] ?? incident.status}</StatusBadge></td>
    </tr>
  );
}

function PagesCard({ pages }: { pages: DashboardPageRow[] }) {
  return (
    <LayerCard title="Status pages" actions={viewAll("/pages")} flush>
      {pages.length === 0 ? (
        <Note>No status pages. <Link className="auth-text-link" to="/pages">Create a page</Link></Note>
      ) : (
        <table className="cool-table">
          <tbody>
            {pages.map((page) => {
              const to = `/pages/${page.id}`;
              return (
                <tr key={page.id} className="clickable-row">
                  <td className="cell-link">
                    <Link className="row-link" to={to}>{page.title}</Link>
                    {rowCover(to)}
                    <div className="truncate text-[11px] text-[var(--ui-fg-faint)]">/status/{page.slug}</div>
                  </td>
                  <td className="cell-link text-right">
                    {rowCover(to)}
                    <span className="inline-flex flex-wrap items-center justify-end gap-1">
                      {!page.published ? <StatusBadge tone="neutral">Draft</StatusBadge> : null}
                      <StatusBadge tone={tone(page.banner)}>{stateLabel[page.banner] ?? page.banner}</StatusBadge>
                    </span>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </LayerCard>
  );
}

function ErrorsCard({ errors }: { errors: Dashboard["errors"] }) {
  const events = errors.events_24h.reduce((sum, value) => sum + value, 0);
  return (
    <LayerCard title="Errors" description="Last 24 hours" actions={viewAll("/errors")} flush>
      {errors.projects === 0 ? (
        <Note>No error projects. <Link className="auth-text-link" to="/error-projects">Create a project</Link></Note>
      ) : (
        <>
          <div className="flex items-end justify-between gap-4 border-b border-[var(--ui-divider)] px-4 py-4">
            <div className="grid flex-1 grid-cols-3 gap-4">
              <Metric label="Unresolved" value={String(errors.unresolved)} emphasize={errors.unresolved > 0} />
              <Metric label="New" value={String(errors.new_24h)} />
              <Metric label="Events" value={events.toLocaleString()} />
            </div>
            <div aria-label="Events over the last 24 hours">
              <Sparkline values={errors.events_24h} />
            </div>
          </div>
          {errors.issues.length === 0 ? (
            <Note>No unresolved issues.</Note>
          ) : (
            <table className="cool-table">
              <tbody>
                {errors.issues.map((issue) => {
                  const to = `/errors/${issue.short_id || issue.id}`;
                  return (
                    <tr key={issue.id} className="clickable-row">
                      <td className="cell-link max-w-[18rem]">
                        <Link className="row-link" to={to}>{issue.title}</Link>
                        {rowCover(to)}
                        <div className="truncate text-[11px] text-[var(--ui-fg-faint)]">{issue.short_id ? `${issue.short_id} · ` : ""}{issue.project_name}</div>
                      </td>
                      <td className="cell-link whitespace-nowrap text-right">
                        {rowCover(to)}
                        <div className="font-mono text-[12px]">{issue.event_count.toLocaleString()}</div>
                        <div className="text-[11px] text-[var(--ui-fg-faint)]">{timeAgo(issue.last_seen)}</div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </>
      )}
    </LayerCard>
  );
}

function TrafficCard({ analytics }: { analytics: Dashboard["analytics"] }) {
  return (
    <LayerCard title="Traffic" description={`Last ${analytics.days} days, against the ${analytics.days} days before`} actions={viewAll("/analytics")} flush>
      {analytics.sites === 0 && analytics.links === 0 ? (
        <Note>No sites or short links. <Link className="auth-text-link" to="/analytics">Add a site</Link></Note>
      ) : (
        <>
          <div className="grid grid-cols-3 gap-4 border-b border-[var(--ui-divider)] px-4 py-4">
            <Metric label="Visitors" value={analytics.visitors.toLocaleString()} previous={{ current: analytics.visitors, previous: analytics.previous_visitors }} />
            <Metric label="Pageviews" value={analytics.pageviews.toLocaleString()} previous={{ current: analytics.pageviews, previous: analytics.previous_pageviews }} />
            <Metric label="Link clicks" value={analytics.clicks.toLocaleString()} previous={{ current: analytics.clicks, previous: analytics.previous_clicks }} />
          </div>
          {analytics.top_sites.length === 0 ? (
            <Note>No sites yet. <Link className="auth-text-link" to="/analytics">Add a site</Link></Note>
          ) : (
            <table className="cool-table">
              <thead>
                <tr>
                  <th>Site</th>
                  <th className="text-right">Visitors</th>
                  <th className="text-right">Views</th>
                </tr>
              </thead>
              <tbody>
                {analytics.top_sites.map((site) => {
                  const to = `/analytics/${site.id}`;
                  return (
                    <tr key={site.id} className="clickable-row">
                      <td className="cell-link">
                        <Link className="row-link" to={to}>{site.name}</Link>
                        {rowCover(to)}
                        <div className="truncate text-[11px] text-[var(--ui-fg-faint)]">{site.domain}</div>
                      </td>
                      <td className="cell-link text-right font-mono">{rowCover(to)}{site.visitors.toLocaleString()}</td>
                      <td className="cell-link text-right font-mono">{rowCover(to)}{site.pageviews.toLocaleString()}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </>
      )}
    </LayerCard>
  );
}
