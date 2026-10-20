import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { LatencyChart, UptimeBars } from "@/components/charts";
import { Badge, Button, Card, Dialog, Empty, ErrorText, Field, Input, Select, Textarea } from "@/components/ui";
import { ApiError, api, type Assertion, type Monitor } from "@/lib/api";
import { formatTime, percent, stateLabel, stateTone, timeAgo } from "@/lib/utils";

type Draft = {
  name: string;
  description: string;
  job_type: "http" | "tcp";
  url: string;
  method: string;
  interval_seconds: string;
  timeout_seconds: string;
  expected_status: string;
  degraded_after_ms: string;
  failure_threshold: string;
  body_contains: string;
  active: boolean;
};

const emptyDraft: Draft = {
  name: "",
  description: "",
  job_type: "http",
  url: "https://",
  method: "GET",
  interval_seconds: "60",
  timeout_seconds: "30",
  expected_status: "",
  degraded_after_ms: "",
  failure_threshold: "3",
  body_contains: "",
  active: true,
};

function draftFrom(monitor: Monitor): Draft {
  const body = monitor.assertions.find((assertion) => assertion.type === "body" && assertion.compare === "contains");
  return {
    name: monitor.name,
    description: monitor.description,
    job_type: monitor.job_type,
    url: monitor.url,
    method: monitor.method,
    interval_seconds: String(monitor.interval_seconds),
    timeout_seconds: String(Math.round(monitor.timeout_ms / 1000)),
    expected_status: monitor.expected_status == null ? "" : String(monitor.expected_status),
    degraded_after_ms: monitor.degraded_after_ms == null ? "" : String(monitor.degraded_after_ms),
    failure_threshold: String(monitor.failure_threshold),
    body_contains: typeof body?.target === "string" ? body.target : "",
    active: monitor.active,
  };
}

function payload(draft: Draft) {
  const assertions: Assertion[] = [];
  if (draft.job_type === "http" && draft.body_contains.trim()) {
    assertions.push({ type: "body", compare: "contains", target: draft.body_contains.trim() });
  }
  return {
    name: draft.name,
    description: draft.description,
    job_type: draft.job_type,
    url: draft.url,
    method: draft.method,
    interval_seconds: Number(draft.interval_seconds),
    timeout_ms: Number(draft.timeout_seconds) * 1000,
    expected_status: draft.expected_status ? Number(draft.expected_status) : null,
    degraded_after_ms: draft.degraded_after_ms ? Number(draft.degraded_after_ms) : null,
    failure_threshold: Number(draft.failure_threshold),
    assertions,
    follow_redirects: true,
    active: draft.active,
    headers: [],
    body: "",
  };
}

export function MonitorsPage() {
  const query = useQuery({ queryKey: ["monitors"], queryFn: () => api<Monitor[]>("/api/monitors"), refetchInterval: 10_000 });
  const [open, setOpen] = useState(false);
  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Monitors</h1>
          <p className="text-sm text-muted-foreground">HTTP and TCP checks from this server.</p>
        </div>
        <Button onClick={() => setOpen(true)}>New monitor</Button>
      </div>
      {query.isLoading ? <p className="text-sm text-muted-foreground">Loading monitors…</p> : null}
      {(query.data?.length ?? 0) === 0 && !query.isLoading ? (
        <Empty title="No monitors" body="Point one at an HTTP URL or a host:port. Checks start on the next scheduler tick." action={<Button onClick={() => setOpen(true)}>New monitor</Button>} />
      ) : (
        <div className="overflow-hidden rounded-xl border">
          <table className="w-full text-sm">
            <thead className="bg-muted/50 text-left text-xs text-muted-foreground">
              <tr>
                <th className="px-4 py-2 font-medium">Name</th>
                <th className="px-4 py-2 font-medium">State</th>
                <th className="hidden px-4 py-2 font-medium md:table-cell">Today</th>
                <th className="hidden px-4 py-2 font-medium sm:table-cell">30 days</th>
                <th className="px-4 py-2 font-medium">Last</th>
              </tr>
            </thead>
            <tbody>
              {query.data?.map((monitor) => (
                <tr key={monitor.id} className="border-t">
                  <td className="px-4 py-3">
                    <Link className="font-medium hover:text-primary" to={`/monitors/${monitor.id}`}>{monitor.name}</Link>
                    <div className="text-xs text-muted-foreground">{monitor.job_type.toUpperCase()} · {monitor.url}</div>
                  </td>
                  <td className="px-4 py-3"><Badge className={stateTone(monitor.state)}>{stateLabel[monitor.state]}</Badge></td>
                  <td className="hidden px-4 py-3 md:table-cell">{percent(monitor.uptime_24h)}</td>
                  <td className="hidden px-4 py-3 sm:table-cell">{percent(monitor.uptime_30d)}</td>
                  <td className="px-4 py-3 text-muted-foreground">{timeAgo(monitor.last_checked_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <MonitorDialog open={open} onClose={() => setOpen(false)} />
    </div>
  );
}

export function MonitorDetailPage() {
  const { id = "" } = useParams();
  const navigate = useNavigate();
  const client = useQueryClient();
  const [editing, setEditing] = useState(false);
  const query = useQuery({
    queryKey: ["monitor", id],
    queryFn: () => api<Monitor>(`/api/monitors/${id}`),
    refetchInterval: 10_000,
  });
  const run = useMutation({
    mutationFn: () => api<Monitor>(`/api/monitors/${id}/checks`, { method: "POST" }),
    onSuccess: (monitor) => client.setQueryData(["monitor", id], monitor),
  });
  const pause = useMutation({
    mutationFn: (active: boolean) =>
      api<Monitor>(`/api/monitors/${id}/active`, { method: "POST", body: JSON.stringify({ active }) }),
    onSuccess: (monitor) => client.setQueryData(["monitor", id], monitor),
  });
  const remove = useMutation({
    mutationFn: () => api(`/api/monitors/${id}`, { method: "DELETE" }),
    onSuccess: () => navigate("/monitors"),
  });
  const monitor = query.data;
  if (query.isLoading) return <p className="text-sm text-muted-foreground">Loading monitor…</p>;
  if (!monitor) return <p className="text-sm text-destructive">{(query.error as Error | null)?.message ?? "Monitor not found."}</p>;
  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <div className="flex items-center gap-2">
            <h1 className="text-2xl font-semibold tracking-tight">{monitor.name}</h1>
            <Badge className={stateTone(monitor.state)}>{monitor.active ? stateLabel[monitor.state] : "Paused"}</Badge>
          </div>
          <p className="text-sm text-muted-foreground">{monitor.job_type.toUpperCase()} {monitor.url}</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" onClick={() => run.mutate()} disabled={run.isPending}>{run.isPending ? "Checking…" : "Run check"}</Button>
          <Button variant="outline" onClick={() => pause.mutate(!monitor.active)}>{monitor.active ? "Pause" : "Resume"}</Button>
          <Button variant="outline" onClick={() => setEditing(true)}>Edit</Button>
          <Button variant="destructive" onClick={() => remove.mutate()}>Delete</Button>
        </div>
      </div>
      <ErrorText error={run.error || pause.error || remove.error} />
      <div className="grid gap-3 sm:grid-cols-4">
        <Stat label="Today" value={percent(monitor.uptime_24h)} />
        <Stat label="30 days" value={percent(monitor.uptime_30d)} />
        <Stat label="90 days" value={percent(monitor.uptime_90d)} />
        <Stat label="Avg latency today" value={monitor.avg_latency_ms == null ? "—" : `${monitor.avg_latency_ms} ms`} />
      </div>
      <Card className="p-4">
        <div className="mb-2 flex justify-between text-xs text-muted-foreground">
          <span>90 days</span>
          <span>{percent(monitor.uptime_90d)} uptime</span>
        </div>
        <UptimeBars bars={monitor.bars} />
      </Card>
      <Card className="p-4">
        <h2 className="mb-2 font-medium">Latency</h2>
        <LatencyChart checks={monitor.recent ?? []} />
      </Card>
      <Card className="overflow-hidden">
        <h2 className="px-4 py-3 font-medium">Recent checks</h2>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <tbody>
              {(monitor.recent ?? []).map((check) => (
                <tr key={check.id} className="border-t">
                  <td className="px-4 py-2"><Badge className={stateTone(check.status)}>{stateLabel[check.status] ?? check.status}</Badge></td>
                  <td className="px-4 py-2">{check.status_code ?? "—"}</td>
                  <td className="px-4 py-2">{check.latency_ms} ms</td>
                  <td className="px-4 py-2 text-muted-foreground">{check.error ?? ""}</td>
                  <td className="px-4 py-2 text-right text-muted-foreground">{formatTime(check.checked_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>
      <MonitorDialog open={editing} onClose={() => setEditing(false)} monitor={monitor} />
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <Card className="p-4">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-1 text-xl font-semibold">{value}</div>
    </Card>
  );
}

function MonitorDialog({ open, onClose, monitor }: { open: boolean; onClose: () => void; monitor?: Monitor }) {
  const client = useQueryClient();
  const [draft, setDraft] = useState<Draft>(monitor ? draftFrom(monitor) : emptyDraft);
  const [fields, setFields] = useState<Record<string, string>>({});
  const save = useMutation({
    mutationFn: () =>
      api<Monitor>(monitor ? `/api/monitors/${monitor.id}` : "/api/monitors", {
        method: monitor ? "PUT" : "POST",
        body: JSON.stringify(payload(draft)),
      }),
    onSuccess: async (saved) => {
      await client.invalidateQueries({ queryKey: ["monitors"] });
      await client.invalidateQueries({ queryKey: ["monitor", saved.id] });
      await client.invalidateQueries({ queryKey: ["overview"] });
      onClose();
    },
    onError: (error) => setFields(error instanceof ApiError ? error.fields ?? {} : {}),
  });

  function submit(event: FormEvent) {
    event.preventDefault();
    setFields({});
    save.mutate();
  }

  return (
    <Dialog
      open={open}
      onClose={() => {
        setDraft(monitor ? draftFrom(monitor) : emptyDraft);
        onClose();
      }}
      title={monitor ? "Edit monitor" : "New monitor"}
      description="Failures are counted until the threshold, then CoolStatus opens an incident."
    >
      <form className="space-y-3" onSubmit={submit}>
        <Field label="Name" error={fields.name}>
          <Input value={draft.name} onChange={(event) => setDraft({ ...draft, name: event.target.value })} required />
        </Field>
        <Field label="Description">
          <Textarea value={draft.description} onChange={(event) => setDraft({ ...draft, description: event.target.value })} />
        </Field>
        <div className="grid gap-3 sm:grid-cols-2">
          <Field label="Type">
            <Select value={draft.job_type} onChange={(event) => setDraft({ ...draft, job_type: event.target.value as Draft["job_type"] })}>
              <option value="http">HTTP</option>
              <option value="tcp">TCP</option>
            </Select>
          </Field>
          <Field label="Interval">
            <Select value={draft.interval_seconds} onChange={(event) => setDraft({ ...draft, interval_seconds: event.target.value })}>
              {[15, 30, 60, 300, 600, 1800, 3600].map((seconds) => (
                <option key={seconds} value={seconds}>{seconds < 60 ? `${seconds}s` : `${seconds / 60} min`}</option>
              ))}
            </Select>
          </Field>
        </div>
        <Field label={draft.job_type === "tcp" ? "Host and port" : "URL"} error={fields.url}>
          <Input value={draft.url} onChange={(event) => setDraft({ ...draft, url: event.target.value })} placeholder={draft.job_type === "tcp" ? "db.internal:5432" : "https://example.com/health"} />
        </Field>
        {draft.job_type === "http" ? (
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Method">
              <Select value={draft.method} onChange={(event) => setDraft({ ...draft, method: event.target.value })}>
                {["GET", "POST", "HEAD", "PUT", "PATCH", "DELETE"].map((method) => <option key={method}>{method}</option>)}
              </Select>
            </Field>
            <Field label="Expected status" hint="blank = any 2xx" error={fields.expected_status}>
              <Input value={draft.expected_status} onChange={(event) => setDraft({ ...draft, expected_status: event.target.value })} inputMode="numeric" />
            </Field>
          </div>
        ) : null}
        <div className="grid gap-3 sm:grid-cols-3">
          <Field label="Timeout" hint="seconds" error={fields.timeout_ms}>
            <Input value={draft.timeout_seconds} onChange={(event) => setDraft({ ...draft, timeout_seconds: event.target.value })} />
          </Field>
          <Field label="Degraded after" hint="ms" error={fields.degraded_after_ms}>
            <Input value={draft.degraded_after_ms} onChange={(event) => setDraft({ ...draft, degraded_after_ms: event.target.value })} />
          </Field>
          <Field label="Failures" hint="before down" error={fields.failure_threshold}>
            <Input value={draft.failure_threshold} onChange={(event) => setDraft({ ...draft, failure_threshold: event.target.value })} />
          </Field>
        </div>
        {draft.job_type === "http" ? (
          <Field label="Body contains" error={fields.assertions}>
            <Input value={draft.body_contains} onChange={(event) => setDraft({ ...draft, body_contains: event.target.value })} placeholder="optional assertion" />
          </Field>
        ) : null}
        <ErrorText error={fields.name ? null : save.error} />
        <div className="flex justify-end gap-2 pt-2">
          <Button type="button" variant="outline" onClick={onClose}>Cancel</Button>
          <Button type="submit" disabled={save.isPending}>{save.isPending ? "Saving…" : "Save monitor"}</Button>
        </div>
      </form>
    </Dialog>
  );
}
