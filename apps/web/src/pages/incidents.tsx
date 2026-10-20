import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { Button, Dialog, Empty, ErrorText, Field, Input, LayerCard, PageHeader, Select, StatusBadge, Textarea } from "@/components/ui";
import { ApiError, api, type Incident, type Monitor, type StatusPage } from "@/lib/api";
import { formatTime, stateLabel } from "@/lib/utils";

export function IncidentsPage() {
  const query = useQuery({ queryKey: ["incidents"], queryFn: () => api<Incident[]>("/api/incidents") });
  const [open, setOpen] = useState(false);
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Incidents"
        description="A timeline customers read on the status page. Automatic incidents open after repeated failures."
        actions={<Button variant="primary" onClick={() => setOpen(true)}>Report incident</Button>}
      />
      {query.isLoading ? <p className="text-[13px] text-[var(--coollabs-subtle)]">Loading incidents…</p> : null}
      {(query.data?.length ?? 0) === 0 && !query.isLoading ? (
        <Empty title="No incidents" body="When a monitor crosses its failure threshold, Status files one. You can also write one by hand." />
      ) : (
        <div className="flex flex-col gap-4">
          {query.data?.map((incident) => (
            <Link key={incident.id} to={`/incidents/${incident.id}`}>
              <LayerCard
                title={incident.title}
                actions={
                  <>
                    <StatusBadge tone={incident.impact === "critical" || incident.impact === "major" ? "error" : incident.impact === "minor" ? "warning" : "neutral"}>{stateLabel[incident.impact]}</StatusBadge>
                    <StatusBadge tone={incident.status === "resolved" ? "success" : incident.status === "investigating" ? "warning" : "info"}>{stateLabel[incident.status]}</StatusBadge>
                  </>
                }
              >
                <p className="line-clamp-2 text-[13px] text-[var(--color-fg-dim)]">{incident.updates.at(-1)?.message}</p>
                <div className="mt-2 text-[11px] text-[var(--coollabs-subtle)]">{formatTime(incident.started_at)}{incident.auto ? " · automatic" : ""}</div>
              </LayerCard>
            </Link>
          ))}
        </div>
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
  if (query.isLoading) return <p className="text-sm text-muted-foreground">Loading incident…</p>;
  if (!incident) return <p className="text-sm text-destructive">{(query.error as Error | null)?.message ?? "Not found."}</p>;
  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6">
      <PageHeader
        title={incident.title}
        description={`Opened ${formatTime(incident.started_at)}${incident.auto ? " by the scheduler" : ""}`}
        actions={<Button variant="danger" onClick={() => remove.mutate()}>Delete</Button>}
      />
      <LayerCard title="Timeline">
        <ol className="space-y-4 border-l border-[var(--coollabs-fill)] pl-5">
          {incident.updates.map((item) => (
            <li key={item.id} className="relative">
              <span className="absolute -left-[1.4rem] top-1.5 size-2.5 rounded-full bg-[var(--color-brand)]" />
              <div className="text-[11px] text-[var(--coollabs-subtle)]">{formatTime(item.created_at)} · {stateLabel[item.status]}</div>
              <p className="mt-1 text-[13px] text-[var(--color-fg-dim)]">{item.message}</p>
            </li>
          ))}
        </ol>
      </LayerCard>
      <LayerCard title="Post an update">
        <div className="space-y-3">
        <Field label="Status">
          <Select value={status} onChange={(event) => setStatus(event.target.value)}>
            {["investigating", "identified", "monitoring", "resolved"].map((item) => (
              <option key={item} value={item}>{stateLabel[item]}</option>
            ))}
          </Select>
        </Field>
        <Field label="Message" error={fields.message}>
          <Textarea value={message} onChange={(event) => setMessage(event.target.value)} />
        </Field>
        <ErrorText error={update.error} />
        <Button variant="primary" onClick={() => update.mutate()} disabled={update.isPending || message.trim().length === 0}>
          {update.isPending ? "Posting…" : "Post update"}
        </Button>
        </div>
      </LayerCard>
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
            <Select value={status} onChange={(event) => setStatus(event.target.value)}>
              {["investigating", "identified", "monitoring", "resolved"].map((item) => <option key={item} value={item}>{stateLabel[item]}</option>)}
            </Select>
          </Field>
          <Field label="Impact">
            <Select value={impact} onChange={(event) => setImpact(event.target.value)}>
              {["none", "minor", "major", "critical"].map((item) => <option key={item} value={item}>{stateLabel[item]}</option>)}
            </Select>
          </Field>
        </div>
        <Field label="Status page">
          <Select value={pageId} onChange={(event) => setPageId(event.target.value)}>
            <option value="">None</option>
            {pages.data?.map((page) => <option key={page.id} value={page.id}>{page.title}</option>)}
          </Select>
        </Field>
        <Field label="Monitor">
          <Select value={monitorId} onChange={(event) => setMonitorId(event.target.value)}>
            <option value="">None</option>
            {monitors.data?.map((monitor) => <option key={monitor.id} value={monitor.id}>{monitor.name}</option>)}
          </Select>
        </Field>
        <Field label="Message" error={fields.message}><Textarea value={message} onChange={(event) => setMessage(event.target.value)} required /></Field>
        <ErrorText error={save.error} />
        <div className="flex justify-end"><Button type="submit" variant="primary" disabled={save.isPending}>{save.isPending ? "Saving…" : "Create incident"}</Button></div>
      </form>
    </Dialog>
  );
}
