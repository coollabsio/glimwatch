import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Link, useParams } from "react-router";
import { Button, Dialog, Empty, ErrorText, Field, Input, LayerCard, PageHeader, Select } from "@/components/ui";
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
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Links"
        description="Short links on your own hostnames, plus /go/{slug} on this Haruvo server. Clicks are counted the same way as page views: no cookies."
        actions={
          <>
            <Button onClick={() => setOpenDomain(true)}>New domain</Button>
            <Button variant="primary" onClick={() => setOpenLink(true)} disabled={(domains.data ?? []).length === 0}>New link</Button>
          </>
        }
      />
      <LayerCard title="Domains" description="A hostname here is only used for redirects when it is not this server's own host." flush>
        {(domains.data ?? []).length === 0 ? <p className="px-4 py-6 text-[13px] text-[var(--coollabs-subtle)]">Add a domain before you create a link.</p> : (
          <ul className="divide-y divide-[var(--coollabs-hairline)] text-[13px]">
            {(domains.data ?? []).map((domain) => (
              <li key={domain.id} className="flex justify-between px-4 py-2">
                <span>{domain.hostname}</span>
                <span className="text-[var(--coollabs-subtle)]">{domain.enabled ? "Enabled" : "Off"}</span>
              </li>
            ))}
          </ul>
        )}
      </LayerCard>
      {(links.data ?? []).length === 0 ? (
        <Empty title="No short links" body="Create a link. Visitors who open /go/your-slug are redirected, and the click is stored." />
      ) : (
        <div className="grid gap-4">
          {(links.data ?? []).map((link) => (
            <Link key={link.id} to={`/links/${link.id}`}>
              <LayerCard title={`/${link.slug}`} description={link.destination}>
                <p className="text-[13px] text-[var(--coollabs-subtle)]">{link.total_clicks} clicks · {link.hostname} · /go/{link.slug}</p>
              </LayerCard>
            </Link>
          ))}
        </div>
      )}
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
            <Select value={domainId || domains.data?.[0]?.id || ""} onChange={(event) => setDomainId(event.target.value)}>
              {(domains.data ?? []).map((domain) => <option key={domain.id} value={domain.id}>{domain.hostname}</option>)}
            </Select>
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
  const values = (overview.data?.series ?? []).map((point) => point.clicks ?? 0);
  const max = Math.max(...values, 1);
  return (
    <div className="flex flex-col gap-6">
      <PageHeader title={item ? `/${item.slug}` : "Link"} description={item ? `${item.destination} · ${window.location.origin}/go/${item.slug}` : "Loading…"} />
      <ErrorText error={link.error ?? overview.error} />
      <div className="grid gap-4 sm:grid-cols-2">
        <LayerCard title="Clicks"><p className="text-2xl font-semibold">{overview.data?.clicks ?? item?.total_clicks ?? "—"}</p></LayerCard>
        <LayerCard title="Visitors"><p className="text-2xl font-semibold">{overview.data?.visitors ?? "—"}</p></LayerCard>
      </div>
      <LayerCard title="Last 30 days">
        <div className="flex h-28 items-end gap-1">
          {(overview.data?.series ?? []).map((point, index) => (
            <div key={point.day} className="min-w-0 flex-1 rounded-sm bg-[var(--color-brand)]" style={{ height: values[index] === 0 ? "0%" : `${Math.max(8, (values[index] / max) * 100)}%` }} />
          ))}
        </div>
      </LayerCard>
      <LayerCard title="Referrers" flush>
        <ul className="divide-y divide-[var(--coollabs-hairline)] text-[13px]">
          {(referrers.data?.rows ?? []).map((row) => (
            <li key={row.label} className="flex justify-between px-4 py-2"><span>{row.label}</span><span>{row.count}</span></li>
          ))}
        </ul>
      </LayerCard>
    </div>
  );
}
