import { useQuery } from "@tanstack/react-query";
import { useParams } from "react-router";
import { Mark } from "@/app/Shell";
import { UptimeBars } from "@/components/charts";
import { Badge } from "@/components/ui";
import { api, type PublicPage } from "@/lib/api";
import { formatTime, percent, stateLabel, stateTone } from "@/lib/utils";

const bannerCopy: Record<PublicPage["banner"], { title: string; className: string }> = {
  operational: { title: "All systems operational", className: "bg-emerald-500/15 text-emerald-950 dark:text-emerald-50" },
  degraded: { title: "Degraded performance", className: "bg-amber-400/20 text-amber-950 dark:text-amber-50" },
  partial_outage: { title: "Partial system outage", className: "bg-amber-500/20 text-amber-950 dark:text-amber-50" },
  major_outage: { title: "Major outage", className: "bg-rose-500/20 text-rose-950 dark:text-rose-50" },
};

export function PublicStatusPage() {
  const { slug = "" } = useParams();
  const query = useQuery({
    queryKey: ["public", slug],
    queryFn: () => api<PublicPage>(`/api/public/${slug}`),
    refetchInterval: 15_000,
  });
  if (query.isLoading) {
    return <div className="grid min-h-svh place-items-center text-sm text-muted-foreground">Loading status…</div>;
  }
  if (query.error || !query.data) {
    return (
      <div className="grid min-h-svh place-items-center px-6 text-center">
        <div>
          <h1 className="text-xl font-semibold">Status page not found</h1>
          <p className="mt-2 text-sm text-muted-foreground">Check the slug, or publish the page from the dashboard.</p>
        </div>
      </div>
    );
  }
  const page = query.data;
  const banner = bannerCopy[page.banner];
  const active = page.incidents.filter((incident) => incident.status !== "resolved");
  const history = page.incidents.filter((incident) => incident.status === "resolved");
  return (
    <div className="min-h-svh bg-background">
      <div className="mx-auto max-w-3xl px-4 py-12 sm:py-16">
        <header className="mb-8 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Mark />
            <div>
              <h1 className="text-2xl font-semibold tracking-tight">{page.title}</h1>
              <p className="text-sm text-muted-foreground">{page.description}</p>
            </div>
          </div>
          {page.homepage_url ? (
            <a className="text-sm text-primary" href={page.homepage_url}>Homepage</a>
          ) : null}
        </header>
        <div className={`rounded-2xl border px-5 py-6 text-lg font-semibold ${banner.className}`}>{banner.title}</div>
        {page.maintenances.filter((item) => item.active || new Date(item.starts_at).getTime() > Date.now()).length > 0 ? (
          <div className="mt-4 rounded-xl border border-sky-500/30 bg-sky-500/10 px-4 py-3 text-sm">
            {page.maintenances
              .filter((item) => item.active || new Date(item.starts_at).getTime() > Date.now())
              .map((item) => (
                <div key={item.id}>
                  <span className="font-medium">{item.active ? "Maintenance in progress" : "Scheduled maintenance"}:</span> {item.title}. {item.message}{" "}
                  <span className="text-muted-foreground">{formatTime(item.starts_at)} – {formatTime(item.ends_at)}</span>
                </div>
              ))}
          </div>
        ) : null}
        <section className="mt-8 space-y-3">
          {page.components.map((component) => (
            <div key={component.id} className="rounded-xl border bg-card p-4">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <div className="font-medium">{component.name}</div>
                  {component.description ? <p className="text-sm text-muted-foreground">{component.description}</p> : null}
                </div>
                <div className="text-right">
                  <Badge className={stateTone(component.state)}>{stateLabel[component.state]}</Badge>
                  <div className="mt-1 text-xs text-muted-foreground">{percent(component.uptime_90d)} uptime</div>
                </div>
              </div>
              <UptimeBars bars={component.bars} className="mt-3" />
              <div className="mt-1 flex justify-between text-[11px] text-muted-foreground">
                <span>90 days ago</span>
                <span>Today</span>
              </div>
            </div>
          ))}
        </section>
        <section className="mt-10">
          <h2 className="text-lg font-semibold">Incidents</h2>
          {active.length === 0 && history.length === 0 ? (
            <p className="mt-2 text-sm text-muted-foreground">No incidents reported.</p>
          ) : null}
          <div className="mt-4 space-y-8">
            {[...active, ...history].map((incident) => (
              <article key={incident.id}>
                <div className="flex flex-wrap items-center gap-2">
                  <h3 className="font-medium">{incident.title}</h3>
                  <Badge className={stateTone(incident.status)}>{stateLabel[incident.status]}</Badge>
                  <span className="text-xs text-muted-foreground">{formatTime(incident.started_at)}</span>
                </div>
                <ol className="mt-3 space-y-3 border-l pl-4">
                  {[...incident.updates].reverse().map((update) => (
                    <li key={update.id}>
                      <div className="text-xs text-muted-foreground">{stateLabel[update.status]} · {formatTime(update.created_at)}</div>
                      <p className="text-sm">{update.message}</p>
                    </li>
                  ))}
                </ol>
              </article>
            ))}
          </div>
        </section>
        <footer className="mt-16 text-center text-xs text-muted-foreground">Powered by CoolStatus</footer>
      </div>
    </div>
  );
}
