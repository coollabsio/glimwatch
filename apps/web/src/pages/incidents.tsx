import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { Badge, Button, Card, Dialog, Empty, ErrorText, Field, Input, Select, Textarea } from "@/components/ui";
import { ApiError, api, type Incident, type Monitor, type StatusPage } from "@/lib/api";
import { formatTime, stateLabel, stateTone } from "@/lib/utils";

export function IncidentsPage() {
  const query = useQuery({ queryKey: ["incidents"], queryFn: () => api<Incident[]>("/api/incidents") });
  const [open, setOpen] = useState(false);
  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Incidents</h1>
          <p className="text-sm text-muted-foreground">A timeline customers read on the status page. Automatic incidents open after repeated failures.</p>
        </div>
        <Button onClick={() => setOpen(true)}>Report incident</Button>
      </div>
      {query.isLoading ? <p className="text-sm text-muted-foreground">Loading incidents…</p> : null}
      {(query.data?.length ?? 0) === 0 && !query.isLoading ? (
        <Empty title="No incidents" body="When a monitor crosses its failure threshold, CoolStatus files one. You can also write one by hand." />
      ) : (
        <div className="space-y-3">
          {query.data?.map((incident) => (
            <Link key={incident.id} to={`/incidents/${incident.id}`}>
              <Card className="p-4 transition hover:-translate-y-0.5">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div className="font-medium">{incident.title}</div>
                  <div className="flex gap-2">
                    <Badge className={stateTone(incident.impact)}>{stateLabel[incident.impact]}</Badge>
                    <Badge className={stateTone(incident.status)}>{stateLabel[incident.status]}</Badge>
                  </div>
                </div>
                <p className="mt-2 line-clamp-2 text-sm text-muted-foreground">{incident.updates.at(-1)?.message}</p>
                <div className="mt-2 text-xs text-muted-foreground">{formatTime(incident.started_at)}{incident.auto ? " · automatic" : ""}</div>
              </Card>
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
    <div className="mx-auto max-w-2xl space-y-5">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">{incident.title}</h1>
          <p className="text-sm text-muted-foreground">Opened {formatTime(incident.started_at)}{incident.auto ? " by the scheduler" : ""}</p>
        </div>
        <Button variant="destructive" onClick={() => remove.mutate()}>Delete</Button>
      </div>
      <ol className="space-y-4 border-l pl-5">
        {incident.updates.map((item) => (
          <li key={item.id} className="relative">
            <span className="absolute -left-[1.4rem] top-1.5 size-2.5 rounded-full bg-primary" />
            <div className="text-xs text-muted-foreground">{formatTime(item.created_at)} · {stateLabel[item.status]}</div>
            <p className="mt-1 text-sm">{item.message}</p>
          </li>
        ))}
      </ol>
      <Card className="space-y-3 p-4">
        <h2 className="font-medium">Post an update</h2>
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
        <Button onClick={() => update.mutate()} disabled={update.isPending || message.trim().length === 0}>
          {update.isPending ? "Posting…" : "Post update"}
        </Button>
      </Card>
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
        <div className="flex justify-end"><Button type="submit" disabled={save.isPending}>{save.isPending ? "Saving…" : "Create incident"}</Button></div>
      </form>
    </Dialog>
  );
}
