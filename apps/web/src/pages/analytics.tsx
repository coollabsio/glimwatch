import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Link, useParams } from "react-router";
import { Button, Dialog, Empty, ErrorText, Field, Input, LayerCard, PageHeader } from "@/components/ui";
import { api, type InsightOverview, type InsightRow, type InsightSite } from "@/lib/api";

const ranges = [
  { id: "24h", label: "24 hours", seconds: 86_400 },
  { id: "7d", label: "7 days", seconds: 7 * 86_400 },
  { id: "30d", label: "30 days", seconds: 30 * 86_400 },
  { id: "90d", label: "90 days", seconds: 90 * 86_400 },
];

const tabs = ["Overview", "Realtime", "Performance", "Pages", "Referrers", "Countries", "Devices", "Events", "Visitors", "Tracker"] as const;

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
      {loading ? <p className="text-[13px] text-[var(--coollabs-subtle)]">Loading sites…</p> : null}
      <ErrorText error={error} />
      {sites.length === 0 && !loading ? (
        <Empty title="No sites yet" body="Add the site you want to measure, then paste the tracker snippet." action={<Button variant="primary" onClick={() => setOpen(true)}>New site</Button>} />
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {sites.map((site) => (
            <Link key={site.id} to={`/analytics/${site.id}`}>
              <LayerCard title={site.name} description={site.domain}>
                <p className="text-[13px] text-[var(--coollabs-subtle)]">Public id {site.public_id}</p>
              </LayerCard>
            </Link>
          ))}
        </div>
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
        description={site ? `${site.domain} · no cookies, no stored IP addresses` : "Loading site…"}
        actions={
          <div className="flex flex-wrap gap-2">
            {ranges.map((range) => (
              <Button key={range.id} variant={seconds === range.seconds ? "primary" : "default"} onClick={() => setSeconds(range.seconds)}>
                {range.label}
              </Button>
            ))}
          </div>
        }
      />
      <div className="flex flex-wrap gap-2">
        {tabs.map((item) => (
          <button key={item} type="button" className={item === tab ? "button button-highlighted" : "button"} onClick={() => setTab(item)}>
            {item}
          </button>
        ))}
      </div>
      {tab === "Overview" ? <OverviewPanel siteId={siteId} query={query} /> : null}
      {tab === "Realtime" ? <RealtimePanel siteId={siteId} /> : null}
      {tab === "Performance" ? <VitalsPanel siteId={siteId} query={query} /> : null}
      {tab === "Pages" ? <BreakdownPanel siteId={siteId} dimension="path" query={query} title="Pages" /> : null}
      {tab === "Referrers" ? <BreakdownPanel siteId={siteId} dimension="referrer" query={query} title="Referrers" /> : null}
      {tab === "Countries" ? <BreakdownPanel siteId={siteId} dimension="country" query={query} title="Countries" /> : null}
      {tab === "Devices" ? <DevicesPanel siteId={siteId} query={query} /> : null}
      {tab === "Events" ? <EventsPanel siteId={siteId} query={query} /> : null}
      {tab === "Visitors" ? <VisitorsPanel siteId={siteId} query={query} /> : null}
      {tab === "Tracker" && site ? <TrackerPanel site={site} /> : null}
    </div>
  );
}

function OverviewPanel({ siteId, query }: { siteId: string; query: string }) {
  const overview = useQuery({
    queryKey: ["site-overview", siteId, query],
    queryFn: () => api<InsightOverview>(`/api/sites/${siteId}/overview?${query}`),
  });
  const pages = useQuery({
    queryKey: ["site-path", siteId, query],
    queryFn: () => api<{ rows: InsightRow[] }>(`/api/sites/${siteId}/breakdown/path?${query}`),
  });
  const data = overview.data;
  return (
    <div className="flex flex-col gap-4">
      <ErrorText error={overview.error} />
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <Stat label="Pageviews" value={data?.pageviews} />
        <Stat label="Visitors" value={data?.visitors} />
        <Stat label="Visits" value={data?.visits} />
        <Stat label="Bounce rate" value={data && data.visits ? `${Math.round((data.bounces / data.visits) * 100)}%` : "—"} />
      </div>
      <LayerCard title="Pageviews">
        <Series series={data?.series ?? []} field="views" />
      </LayerCard>
      <LayerCard title="Top pages" flush>
        <Rows rows={pages.data?.rows ?? []} />
      </LayerCard>
    </div>
  );
}

function RealtimePanel({ siteId }: { siteId: string }) {
  const realtime = useQuery({
    queryKey: ["site-realtime", siteId],
    queryFn: () => api<{ current_visitors: number; activity: { timestamp: number; type: string; path: string | null; country: string | null; browser: string | null }[] }>(`/api/sites/${siteId}/realtime`),
    refetchInterval: 10_000,
  });
  return (
    <LayerCard title="Right now" description={`${realtime.data?.current_visitors ?? 0} visitors in the last 5 minutes`}>
      <ErrorText error={realtime.error} />
      <ul className="space-y-2 text-[13px]">
        {(realtime.data?.activity ?? []).map((item, index) => (
          <li key={`${item.timestamp}-${index}`} className="flex justify-between gap-4">
            <span>{item.path ?? item.type}</span>
            <span className="text-[var(--coollabs-subtle)]">{[item.country, item.browser].filter(Boolean).join(" · ")}</span>
          </li>
        ))}
      </ul>
    </LayerCard>
  );
}

function VitalsPanel({ siteId, query }: { siteId: string; query: string }) {
  const vitals = useQuery({
    queryKey: ["site-vitals", siteId, query],
    queryFn: () => api<{ rows: { metric: string; device: string; p75: number | null; samples: number }[] }>(`/api/sites/${siteId}/vitals?${query}`),
  });
  const rows = vitals.data?.rows ?? [];
  const metrics = ["lcp", "inp", "cls", "fcp", "ttfb"];
  return (
    <LayerCard title="Core Web Vitals" description="p75 from real visits. Times are milliseconds, except CLS.">
      <ErrorText error={vitals.error} />
      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-5">
        {metrics.map((metric) => {
          const sample = rows.find((row) => row.metric === metric && row.device === "Desktop") ?? rows.find((row) => row.metric === metric);
          return <Stat key={metric} label={metric.toUpperCase()} value={sample?.p75 == null ? "—" : metric === "cls" ? sample.p75.toFixed(3) : Math.round(sample.p75)} />;
        })}
      </div>
    </LayerCard>
  );
}

function BreakdownPanel({ siteId, dimension, query, title }: { siteId: string; dimension: string; query: string; title: string }) {
  const data = useQuery({
    queryKey: ["site-breakdown", siteId, dimension, query],
    queryFn: () => api<{ rows: InsightRow[] }>(`/api/sites/${siteId}/breakdown/${dimension}?${query}`),
  });
  return (
    <LayerCard title={title} flush>
      <ErrorText error={data.error} />
      <Rows rows={data.data?.rows ?? []} />
    </LayerCard>
  );
}

function DevicesPanel({ siteId, query }: { siteId: string; query: string }) {
  return (
    <div className="grid gap-4 lg:grid-cols-3">
      <BreakdownPanel siteId={siteId} dimension="browser" query={query} title="Browsers" />
      <BreakdownPanel siteId={siteId} dimension="os" query={query} title="Operating systems" />
      <BreakdownPanel siteId={siteId} dimension="device" query={query} title="Devices" />
    </div>
  );
}

function EventsPanel({ siteId, query }: { siteId: string; query: string }) {
  const events = useQuery({
    queryKey: ["site-events", siteId, query],
    queryFn: () => api<{ total: number; rows: { name: string; count: number; visitors: number }[] }>(`/api/sites/${siteId}/events?${query}`),
  });
  return (
    <LayerCard title="Custom events" description={`${events.data?.total ?? 0} events in this range. Call haruvoEvent(name, data) from the tracker.`} flush>
      <ErrorText error={events.error} />
      <table className="w-full text-left text-[13px]">
        <tbody>
          {(events.data?.rows ?? []).map((row) => (
            <tr key={row.name} className="border-t border-[var(--coollabs-hairline)]">
              <td className="px-4 py-2">{row.name}</td>
              <td className="px-4 py-2 text-right">{row.count}</td>
              <td className="px-4 py-2 text-right text-[var(--coollabs-subtle)]">{row.visitors} visitors</td>
            </tr>
          ))}
        </tbody>
      </table>
    </LayerCard>
  );
}

function VisitorsPanel({ siteId, query }: { siteId: string; query: string }) {
  const visitors = useQuery({
    queryKey: ["site-visitors", siteId, query],
    queryFn: () => api<{ rows: { id: string; views: number; events: number; last_seen: number }[] }>(`/api/sites/${siteId}/visitors?${query}`),
  });
  return (
    <LayerCard title="Visitors" description="Identity resets each month. The list shows the hash, not an address." flush>
      <ErrorText error={visitors.error} />
      <table className="w-full text-left text-[13px]">
        <tbody>
          {(visitors.data?.rows ?? []).map((row) => (
            <tr key={row.id} className="border-t border-[var(--coollabs-hairline)]">
              <td className="px-4 py-2 font-mono text-[12px]"><Link className="auth-text-link" to={`/analytics/${siteId}/visitors/${row.id}`}>{row.id}</Link></td>
              <td className="px-4 py-2 text-right">{row.views} views</td>
              <td className="px-4 py-2 text-right">{row.events} events</td>
            </tr>
          ))}
        </tbody>
      </table>
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
  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Visitor" description={[visitor?.country, visitor?.browser, visitor?.os, visitor?.device].filter(Boolean).join(" · ") || "Loading…"} />
      <ErrorText error={detail.error} />
      <LayerCard title="Activity" flush>
        <ul className="divide-y divide-[var(--coollabs-hairline)] text-[13px]">
          {(detail.data?.activity ?? []).map((item, index) => (
            <li key={`${item.timestamp}-${index}`} className="flex justify-between gap-4 px-4 py-2">
              <span>{item.path ?? item.type}</span>
              <span className="text-[var(--coollabs-subtle)]">{item.type}</span>
            </li>
          ))}
        </ul>
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
  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <LayerCard title="Tracker" description="Paste this before </body>. It sets no cookies.">
        <pre className="overflow-x-auto rounded-lg bg-[var(--coollabs-recessed)] p-3 text-[12px]">{snippet}</pre>
        <p className="mt-3 text-[13px] text-[var(--coollabs-subtle)]">Custom events: <code>haruvoEvent("signup", {"{"} plan: "hobby" {"}"})</code></p>
      </LayerCard>
      <LayerCard title="Public dashboard" description="Share aggregate numbers without a login.">
        <form className="space-y-3" onSubmit={(event: FormEvent) => { event.preventDefault(); save.mutate(); }}>
          <label className="flex items-center gap-2 text-[13px]">
            <input type="checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} />
            Public page enabled
          </label>
          {site.public_enabled ? <p className="text-[13px]">Share path <Link className="auth-text-link" to={`/share/sites/${site.public_slug}`}>/share/sites/{site.public_slug}</Link></p> : null}
          <ErrorText error={save.error} />
          <Button type="submit" variant="primary" disabled={save.isPending}>Save sharing</Button>
        </form>
      </LayerCard>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string | number | undefined }) {
  return (
    <LayerCard title={label}>
      <p className="text-2xl font-semibold tracking-tight">{value ?? "—"}</p>
    </LayerCard>
  );
}

function Series({ series, field }: { series: { day: number; views?: number; clicks?: number }[]; field: "views" | "clicks" }) {
  const values = series.map((point) => (field === "views" ? point.views : point.clicks) ?? 0);
  const max = Math.max(...values, 1);
  if (series.length === 0) return <p className="text-[13px] text-[var(--coollabs-subtle)]">No data in this range.</p>;
  return (
    <div className="flex h-28 items-end gap-1" aria-label="Daily counts">
      {series.map((point, index) => (
        <div
          key={point.day}
          title={`${new Date(point.day * 1000).toISOString().slice(0, 10)}: ${values[index]}`}
          className="min-w-0 flex-1 rounded-sm bg-[var(--color-brand)]"
          style={{ height: values[index] === 0 ? "0%" : `${Math.max(8, (values[index] / max) * 100)}%` }}
        />
      ))}
    </div>
  );
}

function Rows({ rows }: { rows: InsightRow[] }) {
  if (rows.length === 0) return <p className="px-4 py-6 text-[13px] text-[var(--coollabs-subtle)]">Nothing in this range.</p>;
  const max = Math.max(...rows.map((row) => row.count), 1);
  return (
    <ul className="divide-y divide-[var(--coollabs-hairline)] text-[13px]">
      {rows.map((row) => (
        <li key={row.label} className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-4 px-4 py-2">
          <div>
            <div className="truncate">{row.label}</div>
            <div className="mt-1 h-1.5 rounded-full bg-[var(--coollabs-fill)]">
              <div className="h-1.5 rounded-full bg-[var(--color-brand)]" style={{ width: `${(row.count / max) * 100}%` }} />
            </div>
          </div>
          <div className="text-right">
            <div>{row.count}</div>
            {row.visitors != null ? <div className="text-[11px] text-[var(--coollabs-subtle)]">{row.visitors} visitors</div> : null}
          </div>
        </li>
      ))}
    </ul>
  );
}
