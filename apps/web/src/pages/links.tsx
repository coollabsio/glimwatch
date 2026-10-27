import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Link, useParams } from "react-router";
import { BarList, DailyBars, Stat } from "@/components/charts";
import { Button, Dialog, ErrorText, Field, Input, LayerCard, PageHeader, Select, StatusBadge } from "@/components/ui";
import { api, type InsightDomain, type InsightLink, type InsightRow, type LinkOverview } from "@/lib/api";

export function LinksPage() {
  const client = useQueryClient();
  const domains = useQuery({ queryKey: ["domains"], queryFn: () => api<InsightDomain[]>("/api/domains") });
  const links = useQuery({ queryKey: ["links"], queryFn: () => api<InsightLink[]>("/api/links") });
  const [openDomain, setOpenDomain] = useState(false);
  const [openLink, setOpenLink] = useState(false);
  const [hostname, setHostname] = useState("");
  const [slug, setSlug] = useState("");
  const [destination, setDestination] = useState("https://");
  const [domainId, setDomainId] = useState("");
  const createDomain = useMutation({
    mutationFn: () => api<InsightDomain>("/api/domains", { method: "POST", body: JSON.stringify({ hostname, enabled: true }) }),
    onSuccess: async () => {
      setOpenDomain(false);
      setHostname("");
      await client.invalidateQueries({ queryKey: ["domains"] });
    },
  });
  const createLink = useMutation({
    mutationFn: () =>
      api<InsightLink>("/api/links", {
        method: "POST",
        body: JSON.stringify({ domain_id: domainId || domains.data?.[0]?.id, slug, destination, redirect_type: "temporary", enabled: true }),
      }),
    onSuccess: async () => {
      setOpenLink(false);
      setSlug("");
      await client.invalidateQueries({ queryKey: ["links"] });
    },
  });
  const domainRows = domains.data ?? [];
  const linkRows = links.data ?? [];
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Links"
        description="Short links on your own hostnames, plus /go/{slug} on this Haruvo server. Clicks are counted the same way as page views: no cookies."
      />
      <LayerCard
        title="Short links"
        description="Open a link to see its clicks and referrers."
        actions={<Button variant="primary" onClick={() => setOpenLink(true)} disabled={domainRows.length === 0}>New link</Button>}
        flush
      >
        {linkRows.length === 0 ? (
          <CardEmpty title="No short links" body="Create a link. Visitors who open /go/your-slug are redirected, and the click is stored." />
        ) : (
          <table className="cool-table">
            <thead>
              <tr>
                <th>Slug</th>
                <th className="hidden md:table-cell">Destination</th>
                <th className="hidden sm:table-cell">Domain</th>
                <th className="text-right!">Clicks</th>
              </tr>
            </thead>
            <tbody>
              {linkRows.map((link) => {
                const to = `/links/${link.id}`;
                return (
                  <tr key={link.id} className="clickable-row">
                    <td className="cell-link">
                      <Link className="row-link" to={to}>/{link.slug}</Link>
                      {rowCover(to)}
                      <div className="font-mono text-[11px] text-[var(--ui-fg-faint)]">/go/{link.slug}</div>
                    </td>
                    <td className="cell-link hidden md:table-cell">{rowCover(to)}<div className="max-w-[24rem] truncate">{link.destination}</div></td>
                    <td className="cell-link hidden sm:table-cell">{rowCover(to)}{link.hostname}</td>
                    <td className="cell-link text-right font-mono text-[12px]">{rowCover(to)}{link.total_clicks}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </LayerCard>
      <LayerCard
        title="Domains"
        description="A hostname here is only used for redirects when it is not this server's own host."
        actions={<Button onClick={() => setOpenDomain(true)}>New domain</Button>}
        flush
      >
        {domainRows.length === 0 ? <CardEmpty title="No domains" body="Add a domain before you create a link." /> : (
          <table className="cool-table">
            <thead>
              <tr>
                <th>Hostname</th>
                <th className="text-right!">Status</th>
              </tr>
            </thead>
            <tbody>
              {domainRows.map((domain) => (
                <tr key={domain.id}>
                  <td className="font-medium text-[var(--ui-fg)]">{domain.hostname}</td>
                  <td className="text-right"><StatusBadge tone={domain.enabled ? "success" : "neutral"}>{domain.enabled ? "Enabled" : "Off"}</StatusBadge></td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </LayerCard>
      <Dialog open={openDomain} title="New domain" onClose={() => setOpenDomain(false)}>
        <form className="space-y-4" onSubmit={(event: FormEvent) => { event.preventDefault(); createDomain.mutate(); }}>
          <Field label="Hostname"><Input required value={hostname} onChange={(event) => setHostname(event.target.value)} placeholder="go.example.com" /></Field>
          <ErrorText error={createDomain.error} />
          <Button variant="primary" type="submit">Save domain</Button>
        </form>
      </Dialog>
      <Dialog open={openLink} title="New link" onClose={() => setOpenLink(false)}>
        <form className="space-y-4" onSubmit={(event: FormEvent) => { event.preventDefault(); createLink.mutate(); }}>
          <Field label="Domain">
            <Select
              aria-label="Domain"
              className="w-full"
              value={domainId || domains.data?.[0]?.id || ""}
              options={(domains.data ?? []).map((domain) => ({ value: domain.id, label: domain.hostname }))}
              onChange={(value) => setDomainId(value)}
            />
          </Field>
          <Field label="Slug"><Input required value={slug} onChange={(event) => setSlug(event.target.value)} placeholder="coolify" /></Field>
          <Field label="Destination"><Input required value={destination} onChange={(event) => setDestination(event.target.value)} /></Field>
          <ErrorText error={createLink.error} />
          <Button variant="primary" type="submit">Create link</Button>
        </form>
      </Dialog>
    </div>
  );
}

export function LinkDetailPage() {
  const { linkId } = useParams();
  const to = Math.floor(Date.now() / 1000);
  const query = `from=${to - 30 * 86_400}&to=${to}`;
  const link = useQuery({
    queryKey: ["link", linkId],
    queryFn: () => api<InsightLink>(`/api/links/${linkId}`),
    enabled: Boolean(linkId),
  });
  const overview = useQuery({
    queryKey: ["link-overview", linkId, query],
    queryFn: () => api<LinkOverview>(`/api/links/${linkId}/overview?${query}`),
    enabled: Boolean(linkId),
  });
  const referrers = useQuery({
    queryKey: ["link-referrers", linkId, query],
    queryFn: () => api<{ rows: InsightRow[] }>(`/api/links/${linkId}/breakdown/referrer?${query}`),
    enabled: Boolean(linkId),
  });
  const item = link.data;
  return (
    <div className="flex flex-col gap-6">
      <PageHeader keepTitle title={item ? `/${item.slug}` : "Link"} description={item ? `${item.destination} · ${window.location.origin}/go/${item.slug}` : "Loading..."} />
      <ErrorText error={link.error ?? overview.error} />
      <LayerCard title="Summary" description="Last 30 days.">
        <div className="grid gap-6 sm:grid-cols-2">
          <Stat label="Clicks" value={overview.data?.clicks ?? item?.total_clicks} />
          <Stat label="Visitors" value={overview.data?.visitors} />
        </div>
      </LayerCard>
      <LayerCard title="Clicks" description="Clicks per day over the last 30 days.">
        <DailyBars series={overview.data?.series ?? []} field="clicks" />
      </LayerCard>
      <LayerCard title="Referrers" flush>
        <BarList rows={referrers.data?.rows ?? []} empty="No referrers in the last 30 days." />
      </LayerCard>
    </div>
  );
}

function rowCover(to: string) {
  return <Link className="cell-link-anchor" to={to} tabIndex={-1} aria-hidden="true" />;
}

function CardEmpty({ title, body }: { title: string; body: string }) {
  return (
    <div className="px-4 py-10 text-center">
      <p className="text-[14px] font-medium text-[var(--ui-fg)]">{title}</p>
      <p className="mx-auto mt-1 max-w-[28rem] text-[13px] text-[var(--ui-fg-faint)]">{body}</p>
    </div>
  );
}
