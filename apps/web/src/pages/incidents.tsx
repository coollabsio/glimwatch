import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { Search4 as Search } from "reicon-react";
import { Button, Dialog, Empty, ErrorText, Field, Input, LayerCard, PageHeader, Select, StatusBadge, Textarea } from "@/components/ui";
import { ApiError, api, type Incident, type Monitor, type StatusPage } from "@/lib/api";
import { formatTime, stateLabel } from "@/lib/utils";

const statuses = ["investigating", "identified", "monitoring", "resolved"].map((item) => ({ value: item, label: stateLabel[item] }));
const impacts = ["none", "minor", "major", "critical"].map((item) => ({ value: item, label: stateLabel[item] }));

function impactTone(impact: string) {
  return impact === "critical" || impact === "major" ? "error" : impact === "minor" ? "warning" : "neutral";
}

function statusTone(status: string) {
  return status === "resolved" ? "success" : status === "investigating" ? "warning" : "info";
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

export function IncidentsPage() {
  const query = useQuery({ queryKey: ["incidents"], queryFn: () => api<Incident[]>("/api/incidents") });
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const needle = search.trim().toLowerCase();
  const rows = (query.data ?? []).filter((incident) => !needle || incident.title.toLowerCase().includes(needle));
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Incidents"
        description="A timeline customers read on the status page. Automatic incidents open after repeated failures."
        actions={<Button variant="primary" onClick={() => setOpen(true)}>Report incident</Button>}
      />
      {(query.data?.length ?? 0) === 0 && !query.isLoading ? (
        <Empty title="No incidents" body="When a monitor crosses its failure threshold, Owlee files one. You can also write one by hand." />
      ) : (
        <LayerCard title="All incidents" description={query.data ? `${query.data.length} total` : undefined} flush>
          <div className="flex flex-wrap items-center gap-2 border-b border-[var(--ui-divider)] p-2">
            <div className="relative w-full sm:w-72">
              <Search className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-[var(--ui-fg-faint)]" aria-hidden="true" />
              <Input style={{ paddingLeft: 36 }} placeholder="Search by title" value={search} onChange={(event) => setSearch(event.target.value)} aria-label="Search incidents" />
            </div>
          </div>
          {query.isLoading ? (
            <CardText title="Loading incidents…" />
          ) : rows.length === 0 ? (
            <CardText title="No matching incidents" body="Try a different title." />
          ) : (
            <div className="overflow-x-auto">
              <table className="cool-table">
                <thead>
                  <tr>
                    <th>Incident</th>
                    <th>Impact</th>
                    <th>Status</th>
                    <th className="hidden md:table-cell">Source</th>
                    <th>Started</th>
                  </tr>
                </thead>
                <tbody>
                  {rows.map((incident) => {
                    const to = `/incidents/${incident.id}`;
                    return (
                      <tr key={incident.id} className="clickable-row">
                        <td className="cell-link max-w-[28rem]">
                          <Link className="row-link" to={to}>{incident.title}</Link>
                          {rowCover(to)}
                          <div className="truncate text-[11px] text-[var(--ui-fg-faint)]">{incident.updates.at(-1)?.message || "-"}</div>
                        </td>
                        <td className="cell-link">
                          {rowCover(to)}
                          <StatusBadge tone={impactTone(incident.impact)}>{stateLabel[incident.impact]}</StatusBadge>
                        </td>
                        <td className="cell-link">
                          {rowCover(to)}
                          <StatusBadge tone={statusTone(incident.status)}>{stateLabel[incident.status]}</StatusBadge>
                        </td>
                        <td className="cell-link hidden md:table-cell">
                          {rowCover(to)}
                          {incident.auto ? "Automatic" : "Manual"}
                        </td>
                        <td className="cell-link whitespace-nowrap">
                          {rowCover(to)}
                          {formatTime(incident.started_at)}
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
      <IncidentDialog open={open} onClose={() => setOpen(false)} />
    </div>
  );
}

export function IncidentDetailPage() {
  const { id = "" } = useParams();
  const navigate = useNavigate();
  const client = useQueryClient();
  const query = useQuery({ queryKey: ["incident", id], queryFn: () => api<Incident>(`/api/incidents/${id}`) });
  const [status, setStatus] = useState("monitoring");
  const [message, setMessage] = useState("");
  const [fields, setFields] = useState<Record<string, string>>({});
  const update = useMutation({
    mutationFn: () =>
      api<Incident>(`/api/incidents/${id}/updates`, {
        method: "POST",
        body: JSON.stringify({ status, message }),
      }),
    onSuccess: async (incident) => {
      client.setQueryData(["incident", id], incident);
      setMessage("");
      await client.invalidateQueries({ queryKey: ["incidents"] });
      await client.invalidateQueries({ queryKey: ["overview"] });
    },
    onError: (error) => setFields(error instanceof ApiError ? error.fields ?? {} : {}),
  });
  const remove = useMutation({
    mutationFn: () => api(`/api/incidents/${id}`, { method: "DELETE" }),
    onSuccess: () => navigate("/incidents"),
  });
  const incident = query.data;
  if (query.isLoading) return <p className="text-[13px] text-[var(--ui-fg-faint)]">Loading incident…</p>;
  if (!incident) return <p className="field-error">{(query.error as Error | null)?.message ?? "Not found."}</p>;
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title={incident.title}
        keepTitle
        description={`Opened ${formatTime(incident.started_at)}${incident.auto ? " by the scheduler" : ""}`}
        actions={<Button variant="danger" onClick={() => remove.mutate()}>Delete</Button>}
      />
      <ErrorText error={remove.error} />
      <div className="grid items-start gap-6 lg:grid-cols-[1fr_360px]">
        <LayerCard
          title="Timeline"
          actions={
            <>
              <StatusBadge tone={impactTone(incident.impact)}>{stateLabel[incident.impact]}</StatusBadge>
              <StatusBadge tone={statusTone(incident.status)}>{stateLabel[incident.status]}</StatusBadge>
            </>
          }
        >
          <ol className="space-y-4 border-l border-[var(--ui-divider)] pl-5">
            {incident.updates.map((item) => (
              <li key={item.id} className="relative">
                <span className="absolute -left-[1.4rem] top-1.5 size-2.5 rounded-full bg-[var(--ui-accent)]" />
                <div className="text-[11px] text-[var(--ui-fg-faint)]">{formatTime(item.created_at)} · {stateLabel[item.status]}</div>
                <p className="mt-1 text-[13px] text-[var(--ui-fg-dim)]">{item.message}</p>
              </li>
            ))}
          </ol>
        </LayerCard>
        <LayerCard title="Post an update" description="Updates are added to the public timeline.">
          <div className="space-y-3">
            <Field label="Status">
              <Select value={status} options={statuses} onChange={setStatus} aria-label="Status" />
            </Field>
            <Field label="Message" error={fields.message}>
              <Textarea value={message} onChange={(event) => setMessage(event.target.value)} />
            </Field>
            <ErrorText error={update.error} />
            <div className="flex justify-end">
              <Button variant="primary" onClick={() => update.mutate()} disabled={update.isPending || message.trim().length === 0}>
                {update.isPending ? "Posting…" : "Post update"}
              </Button>
            </div>
          </div>
        </LayerCard>
      </div>
    </div>
  );
}

function IncidentDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const navigate = useNavigate();
  const client = useQueryClient();
  const pages = useQuery({ queryKey: ["pages"], queryFn: () => api<StatusPage[]>("/api/pages"), enabled: open });
  const monitors = useQuery({ queryKey: ["monitors"], queryFn: () => api<Monitor[]>("/api/monitors"), enabled: open });
  const [title, setTitle] = useState("");
  const [message, setMessage] = useState("");
  const [status, setStatus] = useState("investigating");
  const [impact, setImpact] = useState("major");
  const [pageId, setPageId] = useState("");
  const [monitorId, setMonitorId] = useState("");
  const [fields, setFields] = useState<Record<string, string>>({});
  const save = useMutation({
    mutationFn: () =>
      api<Incident>("/api/incidents", {
        method: "POST",
        body: JSON.stringify({
          title,
          message,
          status,
          impact,
          page_id: pageId || null,
          monitor_id: monitorId || null,
        }),
      }),
    onSuccess: async (incident) => {
      await client.invalidateQueries({ queryKey: ["incidents"] });
      onClose();
      setTitle("");
      setMessage("");
      navigate(`/incidents/${incident.id}`);
    },
    onError: (error) => setFields(error instanceof ApiError ? error.fields ?? {} : {}),
  });
  function submit(event: FormEvent) {
    event.preventDefault();
    save.mutate();
  }
  return (
    <Dialog open={open} onClose={onClose} title="Report an incident" description="The first message becomes the start of the public timeline.">
      <form className="space-y-3" onSubmit={submit}>
        <Field label="Title" error={fields.title}><Input value={title} onChange={(event) => setTitle(event.target.value)} required /></Field>
        <div className="grid gap-3 sm:grid-cols-2">
          <Field label="Status">
            <Select value={status} options={statuses} onChange={setStatus} aria-label="Status" />
          </Field>
          <Field label="Impact">
            <Select value={impact} options={impacts} onChange={setImpact} aria-label="Impact" />
          </Field>
        </div>
        <Field label="Status page">
          <Select
            value={pageId}
            options={[{ value: "", label: "None" }, ...(pages.data ?? []).map((page) => ({ value: page.id, label: page.title }))]}
            onChange={setPageId}
            aria-label="Status page"
          />
        </Field>
        <Field label="Monitor">
          <Select
            value={monitorId}
            options={[{ value: "", label: "None" }, ...(monitors.data ?? []).map((monitor) => ({ value: monitor.id, label: monitor.name }))]}
            onChange={setMonitorId}
            aria-label="Monitor"
          />
        </Field>
        <Field label="Message" error={fields.message}><Textarea value={message} onChange={(event) => setMessage(event.target.value)} required /></Field>
        <ErrorText error={save.error} />
        <div className="flex justify-end gap-2 pt-2"><Button type="button" onClick={onClose}>Cancel</Button><Button type="submit" variant="primary" disabled={save.isPending}>{save.isPending ? "Saving…" : "Create incident"}</Button></div>
      </form>
    </Dialog>
  );
}
