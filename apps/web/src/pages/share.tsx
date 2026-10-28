import { useQuery } from "@tanstack/react-query";
import { useParams } from "react-router";
import { LayerCard } from "@/components/ui";
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
  const list = rows.data?.rows ?? [];
  return (
    <div className="min-h-svh bg-[var(--ui-canvas)] text-[var(--ui-fg)]">
      <div className="mx-auto flex max-w-3xl flex-col gap-6 px-4 py-12 sm:py-16">
        <header>
          <div className="text-[11px] font-medium text-[var(--ui-fg-faint)]">Glimwatch</div>
          <h1 className="text-[24px] font-semibold leading-8 tracking-tight">{data?.name ?? "Shared analytics"}</h1>
          {data?.subtitle ? <p className="mt-1 text-[13px] text-[var(--ui-fg-faint)]">{data.subtitle}</p> : null}
        </header>
        {overview.error ? <p className="field-error">{(overview.error as Error).message}</p> : null}
        <div className="grid gap-6 sm:grid-cols-2">
          <LayerCard title={safeKind === "links" ? "Clicks" : "Pageviews"} description="Last 30 days">
            <div className="text-[24px] font-semibold tabular-nums">{metric ?? "-"}</div>
          </LayerCard>
          <LayerCard title="Visitors" description="Last 30 days">
            <div className="text-[24px] font-semibold tabular-nums">{data?.visitors ?? "-"}</div>
          </LayerCard>
        </div>
        <LayerCard title={safeKind === "links" ? "Referrers" : "Pages"} flush>
          {list.length === 0 ? (
            <p className="px-4 py-10 text-center text-[13px] text-[var(--ui-fg-faint)]">No data for this period.</p>
          ) : (
            <table className="cool-table">
              <thead>
                <tr>
                  <th>{safeKind === "links" ? "Referrer" : "Page"}</th>
                  <th><span className="block text-right">{safeKind === "links" ? "Clicks" : "Views"}</span></th>
                </tr>
              </thead>
              <tbody>
                {list.map((row) => (
                  <tr key={row.label}>
                    <td className="break-all text-[var(--ui-fg)]">{row.label}</td>
                    <td className="text-right tabular-nums">{row.count}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </LayerCard>
        <footer className="pt-4 text-center text-[11px] text-[var(--ui-fg-faint)]">
          <a href="https://glimwatch.io" className="hover:text-[var(--ui-fg-dim)]">Powered by Glimwatch</a>
        </footer>
      </div>
    </div>
  );
}
