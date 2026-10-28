import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, type ReactNode, useState } from "react";
import { Link, useParams } from "react-router";
import { BarList, DailyBars, Stat } from "@/components/charts";
import { Button, Dialog, Empty, ErrorText, Field, Input, LayerCard, PageHeader, Select } from "@/components/ui";
import { api, type InsightOverview, type InsightRow, type InsightSite } from "@/lib/api";
import { cn } from "@/lib/utils";

const ranges = [
  { id: "24h", label: "24 hours", seconds: 86_400 },
  { id: "7d", label: "7 days", seconds: 7 * 86_400 },
  { id: "30d", label: "30 days", seconds: 30 * 86_400 },
  { id: "90d", label: "90 days", seconds: 90 * 86_400 },
];

const tabs = ["Overview", "Realtime", "Events", "Visitors", "Tracker"] as const;

function rangeQuery(seconds: number) {
  const to = Math.floor(Date.now() / 1000);
  return `from=${to - seconds}&to=${to}`;
}

export function AnalyticsPage() {
  const { siteId } = useParams();
  const sites = useQuery({ queryKey: ["sites"], queryFn: () => api<InsightSite[]>("/api/sites") });
  if (!siteId) return <SiteList sites={sites.data ?? []} loading={sites.isLoading} error={sites.error} />;
  const site = sites.data?.find((item) => item.id === siteId);
  return <SiteDetail siteId={siteId} site={site} />;
}

function rowCover(to: string) {
  return <Link className="cell-link-anchor" to={to} tabIndex={-1} aria-hidden="true" />;
}

function CardEmpty({ title, body }: { title: string; body?: string }) {
  return (
    <div className="px-4 py-10 text-center">
      <p className="text-[14px] font-medium text-[var(--ui-fg)]">{title}</p>
      {body ? <p className="mx-auto mt-1 max-w-[28rem] text-[13px] text-[var(--ui-fg-faint)]">{body}</p> : null}
    </div>
  );
}

function SiteList({ sites, loading, error }: { sites: InsightSite[]; loading: boolean; error: unknown }) {
  const client = useQueryClient();
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [domain, setDomain] = useState("");
  const create = useMutation({
    mutationFn: () => api<InsightSite>("/api/sites", { method: "POST", body: JSON.stringify({ name, domain }) }),
    onSuccess: async () => {
      setOpen(false);
      setName("");
      setDomain("");
      await client.invalidateQueries({ queryKey: ["sites"] });
    },
  });
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Sites"
        description="Cookieless page views. Visitors are a daily hash of address and browser, and the address is not stored."
        actions={<Button variant="primary" onClick={() => setOpen(true)}>New site</Button>}
      />
      <ErrorText error={error} />
      {sites.length === 0 && !loading ? (
        <Empty title="No sites yet" body="Add the site you want to measure, then paste the tracker snippet." action={<Button variant="primary" onClick={() => setOpen(true)}>New site</Button>} />
      ) : (
        <LayerCard title="Sites" description="Open a site to see its traffic." flush>
          {loading ? <CardEmpty title="Loading sites..." /> : (
            <table className="cool-table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Domain</th>
                  <th className="hidden sm:table-cell">Public id</th>
                </tr>
              </thead>
              <tbody>
                {sites.map((site) => {
                  const to = `/analytics/${site.id}`;
                  return (
                    <tr key={site.id} className="clickable-row">
                      <td className="cell-link"><Link className="row-link" to={to}>{site.name}</Link>{rowCover(to)}</td>
                      <td className="cell-link">{rowCover(to)}{site.domain}</td>
                      <td className="cell-link hidden font-mono text-[12px] sm:table-cell">{rowCover(to)}{site.public_id}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </LayerCard>
      )}
      <Dialog open={open} title="New site" description="The domain is used to ignore your own referrer." onClose={() => setOpen(false)}>
        <form className="space-y-4" onSubmit={(event) => { event.preventDefault(); create.mutate(); }}>
          <Field label="Name"><Input required value={name} onChange={(event) => setName(event.target.value)} placeholder="coolLabs website" /></Field>
          <Field label="Domain"><Input required value={domain} onChange={(event) => setDomain(event.target.value)} placeholder="coolify.io" /></Field>
          <ErrorText error={create.error} />
          <Button variant="primary" type="submit" disabled={create.isPending}>Create site</Button>
        </form>
      </Dialog>
    </div>
  );
}

function SiteDetail({ siteId, site }: { siteId: string; site?: InsightSite }) {
  const [tab, setTab] = useState<(typeof tabs)[number]>("Overview");
  const [seconds, setSeconds] = useState(ranges[2].seconds);
  const query = rangeQuery(seconds);
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title={site?.name ?? "Site"}
        keepTitle
        description={site ? `${site.domain} · no cookies, no stored IP addresses` : "Loading site..."}
        actions={
          <Select
            className="w-36"
            aria-label="Range"
            value={String(seconds)}
            options={ranges.map((range) => ({ value: String(range.seconds), label: range.label }))}
            onChange={(value) => setSeconds(Number(value))}
          />
        }
      />
      {/* The rule lives on the wrapper. The nav overlaps it by 1px so the
          active underline covers that rule. Putting the overlap on the
          buttons, inside overflow-x-auto, computes overflow-y to auto and
          the extra pixel shows a scrollbar even when the tabs fit. */}
      <div className="-mt-2 min-w-0 border-b border-[var(--ui-divider)]">
        <nav className="-mb-px flex gap-1 overflow-x-auto overflow-y-hidden" aria-label="Site sections">
          {tabs.map((item) => (
            <button
              key={item}
              type="button"
              aria-current={item === tab ? "page" : undefined}
              className={cn(
                "h-9 shrink-0 border-b-2 px-2.5 text-[13px] font-medium transition-colors",
                item === tab ? "border-[var(--ui-accent)] text-[var(--ui-fg)]" : "border-transparent text-[var(--ui-fg-faint)] hover:text-[var(--ui-fg)]",
              )}
              onClick={() => setTab(item)}
            >
              {item}
            </button>
          ))}
        </nav>
      </div>
      {tab === "Overview" ? <OverviewPanel siteId={siteId} query={query} /> : null}
      {tab === "Realtime" ? <RealtimePanel siteId={siteId} /> : null}
      {tab === "Events" ? <EventsPanel siteId={siteId} query={query} /> : null}
      {tab === "Visitors" ? <VisitorsPanel siteId={siteId} query={query} /> : null}
      {tab === "Tracker" && site ? <TrackerPanel site={site} /> : null}
    </div>
  );
}

function StatGrid({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={cn("grid gap-6 sm:grid-cols-2", className)}>{children}</div>;
}

function OverviewPanel({ siteId, query }: { siteId: string; query: string }) {
  const overview = useQuery({
    queryKey: ["site-overview", siteId, query],
    queryFn: () => api<InsightOverview>(`/api/sites/${siteId}/overview?${query}`),
  });
  const data = overview.data;
  return (
    <div className="flex flex-col gap-6">
      <ErrorText error={overview.error} />
      <LayerCard title="Summary" description="Totals for the selected range.">
        <StatGrid className="xl:grid-cols-4">
          <Stat label="Pageviews" value={data?.pageviews} />
          <Stat label="Visitors" value={data?.visitors} />
          <Stat label="Visits" value={data?.visits} />
          <Stat label="Bounce rate" value={data && data.visits ? `${Math.round((data.bounces / data.visits) * 100)}%` : "-"} />
        </StatGrid>
      </LayerCard>
      <LayerCard title="Pageviews" description="Views per day.">
        <DailyBars series={data?.series ?? []} field="views" />
      </LayerCard>
      <div className="grid items-start gap-6 lg:grid-cols-2 xl:grid-cols-3">
        <BreakdownCard siteId={siteId} query={query} title="Pages" views={[{ dimension: "path", label: "Path" }]} />
        <BreakdownCard siteId={siteId} query={query} title="Referrers" views={[{ dimension: "referrer", label: "Referrers" }]} />
        <BreakdownCard siteId={siteId} query={query} title="Locations" views={[{ dimension: "country", label: "Countries" }]} />
        <BreakdownCard
          siteId={siteId}
          query={query}
          title="Technology"
          views={[
            { dimension: "browser", label: "Browsers" },
            { dimension: "os", label: "OS" },
            { dimension: "device", label: "Devices" },
          ]}
        />
        <VitalsCard siteId={siteId} query={query} />
      </div>
    </div>
  );
}

function RealtimePanel({ siteId }: { siteId: string }) {
  const realtime = useQuery({
    queryKey: ["site-realtime", siteId],
    queryFn: () => api<{ current_visitors: number; activity: { timestamp: number; type: string; path: string | null; country: string | null; browser: string | null }[] }>(`/api/sites/${siteId}/realtime`),
    refetchInterval: 10_000,
  });
  const activity = realtime.data?.activity ?? [];
  return (
    <div className="flex flex-col gap-6">
      <ErrorText error={realtime.error} />
      <LayerCard title="Right now" description="Visitors in the last 5 minutes.">
        <Stat label="Current visitors" value={realtime.data?.current_visitors ?? 0} />
      </LayerCard>
      <LayerCard title="Activity" flush>
        {activity.length === 0 ? <CardEmpty title="No activity" body="New page views and events show up here." /> : (
          <table className="cool-table">
            <thead>
              <tr>
                <th>Page</th>
                <th>Type</th>
                <th className="hidden sm:table-cell">Country</th>
                <th className="hidden sm:table-cell">Browser</th>
              </tr>
            </thead>
            <tbody>
              {activity.map((item, index) => (
                <tr key={`${item.timestamp}-${index}`}>
                  <td className="font-medium text-[var(--ui-fg)]">{item.path ?? item.type}</td>
                  <td>{item.type}</td>
                  <td className="hidden sm:table-cell">{item.country ?? "-"}</td>
                  <td className="hidden sm:table-cell">{item.browser ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </LayerCard>
    </div>
  );
}

function VitalsCard({ siteId, query }: { siteId: string; query: string }) {
  const vitals = useQuery({
    queryKey: ["site-vitals", siteId, query],
    queryFn: () => api<{ rows: { metric: string; device: string; p75: number | null; samples: number }[] }>(`/api/sites/${siteId}/vitals?${query}`),
  });
  const rows = vitals.data?.rows ?? [];
  const metrics = ["lcp", "inp", "cls", "fcp", "ttfb"];
  return (
    <LayerCard title="Web vitals" description="p75 from real visits, in ms (CLS has no unit)." flush>
      {vitals.error ? <div className="px-4 pt-3"><ErrorText error={vitals.error} /></div> : null}
      <ul className="flex flex-col gap-0.5 p-2">
        {metrics.map((metric) => {
          const sample = rows.find((row) => row.metric === metric && row.device === "Desktop") ?? rows.find((row) => row.metric === metric);
          const value = sample?.p75 == null ? "-" : metric === "cls" ? sample.p75.toFixed(3) : Math.round(sample.p75);
          return (
            <li key={metric} className="flex h-[30px] items-center justify-between rounded-[6px] px-2.5 text-[12px]">
              <span className="font-medium text-[var(--ui-fg-dim)]">{metric.toUpperCase()}</span>
              <span className="font-mono text-[11px] text-[var(--ui-fg-dim)]">{value}</span>
            </li>
          );
        })}
      </ul>
    </LayerCard>
  );
}

const BREAKDOWN_ROWS = 8;

function BreakdownCard({
  siteId,
  query,
  title,
  views,
}: {
  siteId: string;
  query: string;
  title: string;
  views: { dimension: string; label: string }[];
}) {
  const [dimension, setDimension] = useState(views[0].dimension);
  const [expanded, setExpanded] = useState(false);
  const data = useQuery({
    queryKey: ["site-breakdown", siteId, dimension, query],
    queryFn: () => api<{ rows: InsightRow[] }>(`/api/sites/${siteId}/breakdown/${dimension}?${query}`),
  });
  const rows = data.data?.rows ?? [];
  return (
    <LayerCard
      title={title}
      flush
      actions={
        <div className="flex items-center gap-3">
          {views.length > 1 ? (
            <span className="inline-flex rounded-md border border-[var(--ui-control-border)] p-0.5">
              {views.map((view) => (
                <button
                  key={view.dimension}
                  type="button"
                  aria-pressed={view.dimension === dimension}
                  className={cn(
                    "h-5 rounded px-1.5 text-[11px] font-medium",
                    view.dimension === dimension ? "bg-[var(--ui-hover)] text-[var(--ui-fg)]" : "text-[var(--ui-fg-faint)] hover:text-[var(--ui-fg)]",
                  )}
                  onClick={() => {
                    setDimension(view.dimension);
                    setExpanded(false);
                  }}
                >
                  {view.label}
                </button>
              ))}
            </span>
          ) : null}
          {rows.length > BREAKDOWN_ROWS ? (
            <button type="button" className="text-[11px] font-medium text-[var(--ui-fg-faint)] hover:text-[var(--ui-fg)]" onClick={() => setExpanded((value) => !value)}>
              {expanded ? "Show less" : "View all"}
            </button>
          ) : null}
        </div>
      }
    >
      {data.error ? <div className="px-4 pt-3"><ErrorText error={data.error} /></div> : null}
      <BarList rows={expanded ? rows : rows.slice(0, BREAKDOWN_ROWS)} total={rows.reduce((sum, row) => sum + row.count, 0)} />
    </LayerCard>
  );
}

function EventsPanel({ siteId, query }: { siteId: string; query: string }) {
  const events = useQuery({
    queryKey: ["site-events", siteId, query],
    queryFn: () => api<{ total: number; rows: { name: string; count: number; visitors: number }[] }>(`/api/sites/${siteId}/events?${query}`),
  });
  const rows = events.data?.rows ?? [];
  return (
    <LayerCard title="Custom events" description={`${events.data?.total ?? 0} events in this range. Call glimwatchEvent(name, data) from the tracker.`} flush>
      {events.error ? <div className="px-4 pt-3"><ErrorText error={events.error} /></div> : null}
      {rows.length === 0 ? <CardEmpty title="No events yet" body="Events you send from the tracker show up here." /> : (
        <table className="cool-table">
          <thead>
            <tr>
              <th>Name</th>
              <th className="text-right!">Count</th>
              <th className="text-right!">Visitors</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.name}>
                <td className="font-medium text-[var(--ui-fg)]">{row.name}</td>
                <td className="text-right font-mono text-[12px]">{row.count}</td>
                <td className="text-right font-mono text-[12px]">{row.visitors}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </LayerCard>
  );
}

function VisitorsPanel({ siteId, query }: { siteId: string; query: string }) {
  const visitors = useQuery({
    queryKey: ["site-visitors", siteId, query],
    queryFn: () => api<{ rows: { id: string; views: number; events: number; last_seen: number }[] }>(`/api/sites/${siteId}/visitors?${query}`),
  });
  const rows = visitors.data?.rows ?? [];
  return (
    <LayerCard title="Visitors" description="Identity resets each month. The list shows the hash, not an address." flush>
      {visitors.error ? <div className="px-4 pt-3"><ErrorText error={visitors.error} /></div> : null}
      {rows.length === 0 ? <CardEmpty title="No visitors" body="Visitors in this range show up here." /> : (
        <table className="cool-table">
          <thead>
            <tr>
              <th>Visitor</th>
              <th className="text-right!">Views</th>
              <th className="text-right!">Events</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => {
              const to = `/analytics/${siteId}/visitors/${row.id}`;
              return (
                <tr key={row.id} className="clickable-row">
                  <td className="cell-link"><Link className="row-link font-mono text-[12px]" to={to}>{row.id}</Link>{rowCover(to)}</td>
                  <td className="cell-link text-right font-mono text-[12px]">{rowCover(to)}{row.views}</td>
                  <td className="cell-link text-right font-mono text-[12px]">{rowCover(to)}{row.events}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </LayerCard>
  );
}

export function VisitorPage() {
  const { siteId, visitorId } = useParams();
  const detail = useQuery({
    queryKey: ["visitor", siteId, visitorId],
    queryFn: () => api<{ visitor: { id: string; browser: string | null; os: string | null; country: string | null; device: string | null }; activity: { timestamp: number; type: string; path: string | null; title: string | null }[] }>(`/api/sites/${siteId}/visitors/${visitorId}`),
    enabled: Boolean(siteId && visitorId),
  });
  const visitor = detail.data?.visitor;
  const activity = detail.data?.activity ?? [];
  return (
    <div className="flex flex-col gap-6">
      <PageHeader keepTitle title="Visitor" description={visitor ? `Hash ${visitor.id}` : "Loading..."} />
      <ErrorText error={detail.error} />
      <LayerCard title="Details">
        <StatGrid className="xl:grid-cols-4">
          <Stat label="Country" value={visitor?.country ?? "-"} />
          <Stat label="Browser" value={visitor?.browser ?? "-"} />
          <Stat label="Operating system" value={visitor?.os ?? "-"} />
          <Stat label="Device" value={visitor?.device ?? "-"} />
        </StatGrid>
      </LayerCard>
      <LayerCard title="Activity" flush>
        {activity.length === 0 ? <CardEmpty title="No activity" /> : (
          <table className="cool-table">
            <thead>
              <tr>
                <th>Page</th>
                <th>Title</th>
                <th className="text-right!">Type</th>
              </tr>
            </thead>
            <tbody>
              {activity.map((item, index) => (
                <tr key={`${item.timestamp}-${index}`}>
                  <td className="font-medium text-[var(--ui-fg)]">{item.path ?? item.type}</td>
                  <td>{item.title || "-"}</td>
                  <td className="text-right">{item.type}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </LayerCard>
    </div>
  );
}

function TrackerPanel({ site }: { site: InsightSite }) {
  const client = useQueryClient();
  const [enabled, setEnabled] = useState(site.public_enabled);
  const snippet = `<script defer src="${window.location.origin}/data.js" data-site="${site.public_id}"></script>`;
  const save = useMutation({
    mutationFn: () =>
      api(`/api/sites/${site.id}/public`, {
        method: "PUT",
        body: JSON.stringify({
          enabled,
          slug: site.public_slug,
          threshold: site.public_threshold,
          sections: site.public_sections,
        }),
      }),
    onSuccess: () => client.invalidateQueries({ queryKey: ["sites"] }),
  });
  const code = "rounded-lg border border-[var(--ui-control-border)] bg-[var(--ui-control-bg)] font-mono text-[12px]";
  return (
    <div className="grid gap-6 lg:grid-cols-2">
      <LayerCard title="Tracker" description="Paste this before </body>. It sets no cookies.">
        <pre className={cn(code, "overflow-x-auto p-3 text-[var(--ui-fg)]")}>{snippet}</pre>
        <p className="mt-4 mb-1.5 text-[11px] font-medium text-[var(--ui-fg-faint)]">Custom events</p>
        <pre className={cn(code, "overflow-x-auto p-3 text-[var(--ui-fg)]")}>{`glimwatchEvent("signup", { plan: "hobby" })`}</pre>
      </LayerCard>
      <LayerCard title="Public dashboard" description="Share aggregate numbers without a login.">
        <form className="space-y-4" onSubmit={(event: FormEvent) => { event.preventDefault(); save.mutate(); }}>
          <label className="flex items-center gap-2 text-[13px] font-medium text-[var(--ui-fg-dim)]">
            <input type="checkbox" className="accent-[var(--ui-accent)]" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} />
            Public page enabled
          </label>
          {site.public_enabled ? (
            <p className="text-[13px] text-[var(--ui-fg-faint)]">
              Share path <Link className="auth-text-link font-mono text-[12px]" to={`/share/sites/${site.public_slug}`}>/share/sites/{site.public_slug}</Link>
            </p>
          ) : null}
          <ErrorText error={save.error} />
          <Button type="submit" variant="primary" disabled={save.isPending}>Save sharing</Button>
        </form>
      </LayerCard>
    </div>
  );
}
