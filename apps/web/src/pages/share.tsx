import { useQuery } from "@tanstack/react-query";
import { useParams } from "react-router";
import { api, type InsightRow, type PublicInsight } from "@/lib/api";

export function SharePage() {
  const { kind, slug } = useParams();
  const safeKind = kind === "links" ? "links" : "sites";
  const to = Math.floor(Date.now() / 1000);
  const query = `from=${to - 30 * 86_400}&to=${to}`;
  const overview = useQuery({
    queryKey: ["public", safeKind, slug, query],
    queryFn: () => api<PublicInsight>(`/api/public/${safeKind}/${slug}/overview?${query}`),
    enabled: Boolean(slug),
  });
  const dimension = safeKind === "links" ? "referrer" : "path";
  const rows = useQuery({
    queryKey: ["public-rows", safeKind, slug, dimension, query],
    queryFn: () => api<{ rows: InsightRow[] }>(`/api/public/${safeKind}/${slug}/${dimension}?${query}`),
    enabled: Boolean(slug) && overview.isSuccess,
  });
  const data = overview.data;
  const metric = safeKind === "links" ? data?.clicks : data?.pageviews;
  return (
    <div className="min-h-svh bg-[var(--coollabs-canvas)] text-[var(--color-fg)]">
      <div className="mx-auto flex max-w-3xl flex-col gap-6 px-4 py-12">
        <header>
          <div className="text-[11px] font-medium text-[var(--coollabs-subtle)]">Haruvo</div>
          <h1 className="text-xl font-semibold">{data?.name ?? "Shared analytics"}</h1>
          <p className="text-[13px] text-[var(--coollabs-subtle)]">{data?.subtitle}</p>
        </header>
        {overview.error ? <p className="field-error">{(overview.error as Error).message}</p> : null}
        <div className="grid gap-4 sm:grid-cols-2">
          <section className="layer-card"><header><h2>{safeKind === "links" ? "Clicks" : "Pageviews"}</h2></header><div className="layer-card-body text-2xl font-semibold">{metric ?? "—"}</div></section>
          <section className="layer-card"><header><h2>Visitors</h2></header><div className="layer-card-body text-2xl font-semibold">{data?.visitors ?? "—"}</div></section>
        </div>
        <section className="layer-card">
          <header><h2>{safeKind === "links" ? "Referrers" : "Pages"}</h2></header>
          <ul className="layer-card-body is-flush divide-y divide-[var(--coollabs-hairline)] text-[13px]">
            {(rows.data?.rows ?? []).map((row) => (
              <li key={row.label} className="flex justify-between px-4 py-2"><span>{row.label}</span><span>{row.count}</span></li>
            ))}
          </ul>
        </section>
      </div>
    </div>
  );
}
