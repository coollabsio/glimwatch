import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { LatencyChart, UptimeBars } from "@/components/charts";
import { Button, Dialog, Empty, ErrorText, Field, Input, LayerCard, PageHeader, Select, StatusBadge, Textarea } from "@/components/ui";
import { ApiError, api, type Assertion, listAgents, type Monitor } from "@/lib/api";
import { formatTime, percent, stateLabel, timeAgo } from "@/lib/utils";

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
  const label = useMonitorLabel();
  const [open, setOpen] = useState(false);
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Monitors"
        description="HTTP and TCP checks from this server. Agents add checks from their own config files."
        actions={<Button variant="primary" onClick={() => setOpen(true)}>New monitor</Button>}
      />
      {query.isLoading ? <p className="text-[13px] text-[var(--coollabs-subtle)]">Loading monitors…</p> : null}
      {(query.data?.length ?? 0) === 0 && !query.isLoading ? (
        <Empty title="No monitors" body="Point one at an HTTP URL or a host:port. Checks start on the next scheduler tick." action={<Button variant="primary" onClick={() => setOpen(true)}>New monitor</Button>} />
      ) : (
        <LayerCard title="All monitors" flush>
          <table className="cool-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>State</th>
                <th className="hidden md:table-cell">Today</th>
                <th className="hidden sm:table-cell">30 days</th>
                <th>Last</th>
              </tr>
            </thead>
            <tbody>
              {query.data?.map((monitor) => (
                <tr key={monitor.id}>
                  <td>
                    <Link className="font-medium text-[var(--color-fg)]" to={`/monitors/${monitor.id}`}>{monitor.name}</Link>
                    <div className="text-[11px] text-[var(--coollabs-subtle)]">{label(monitor)}</div>
                  </td>
                  <td><StatusBadge tone={monitor.state === "up" ? "success" : monitor.state === "down" ? "error" : monitor.state === "degraded" ? "warning" : "neutral"}>{stateLabel[monitor.state]}</StatusBadge></td>
                  <td className="hidden md:table-cell">{percent(monitor.uptime_24h)}</td>
                  <td className="hidden sm:table-cell">{percent(monitor.uptime_30d)}</td>
                  <td>{timeAgo(monitor.last_checked_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </LayerCard>
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
  const label = useMonitorLabel();
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
  if (query.isLoading) return <p className="text-[13px] text-[var(--coollabs-subtle)]">Loading monitor…</p>;
  if (!monitor) return <p className="field-error">{(query.error as Error | null)?.message ?? "Monitor not found."}</p>;
  const stateToneName = monitor.state === "up" ? "success" : monitor.state === "down" ? "error" : monitor.state === "degraded" ? "warning" : "neutral";
  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title={monitor.name}
        description={label(monitor)}
        actions={
          <>
            <StatusBadge tone={monitor.active ? stateToneName : "neutral"}>{monitor.active ? stateLabel[monitor.state] : "Paused"}</StatusBadge>
            {monitor.agent_id ? null : <Button variant="primary" onClick={() => run.mutate()} disabled={run.isPending}>{run.isPending ? "Checking…" : "Run check"}</Button>}
            <Button onClick={() => pause.mutate(!monitor.active)}>{monitor.active ? "Pause" : "Resume"}</Button>
            {monitor.agent_id ? null : <Button onClick={() => setEditing(true)}>Edit</Button>}
            <Button variant="danger" onClick={() => remove.mutate()}>Delete</Button>
          </>
        }
      />
      <ErrorText error={run.error || pause.error || remove.error} />
      {monitor.agent_id ? (
        <p className="text-[13px] text-[var(--coollabs-subtle)]">This check is defined in the agent's config file. Change it there and restart the agent.</p>
      ) : null}
      <div className="grid gap-4 sm:grid-cols-4">
        <Stat label="Today" value={percent(monitor.uptime_24h)} />
        <Stat label="30 days" value={percent(monitor.uptime_30d)} />
        <Stat label="90 days" value={percent(monitor.uptime_90d)} />
        <Stat label="Avg latency today" value={monitor.avg_latency_ms == null ? "-" : `${monitor.avg_latency_ms} ms`} />
      </div>
      <LayerCard title="Uptime" description="90 days">
        <div className="mb-2 flex justify-end text-[11px] text-[var(--coollabs-subtle)]">
          <span>{percent(monitor.uptime_90d)} uptime</span>
        </div>
        <UptimeBars bars={monitor.bars} />
      </LayerCard>
      <LayerCard title="Latency">
        <LatencyChart checks={monitor.recent ?? []} />
      </LayerCard>
      <LayerCard title="Recent checks" flush>
        <div className="overflow-x-auto">
          <table className="cool-table">
            <thead>
              <tr>
                <th>Result</th>
                <th>Status</th>
                <th>Latency</th>
                <th>Error</th>
                <th>From</th>
                <th>When</th>
              </tr>
            </thead>
            <tbody>
              {(monitor.recent ?? []).map((check) => (
                <tr key={check.id}>
                  <td><StatusBadge tone={check.status === "success" ? "success" : check.status === "failure" ? "error" : check.status === "degraded" ? "warning" : "info"}>{stateLabel[check.status] ?? check.status}</StatusBadge></td>
                  <td>{check.status_code ?? "-"}</td>
                  <td>{check.latency_ms} ms</td>
                  <td>{check.error ?? ""}</td>
                  <td>{check.region === "local" ? "This server" : check.region}</td>
                  <td>{formatTime(check.checked_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </LayerCard>
      {monitor.agent_id ? null : <MonitorDialog open={editing} onClose={() => setEditing(false)} monitor={monitor} />}
    </div>
  );
}

function useMonitorLabel() {
  const agents = useQuery({ queryKey: ["agents"], queryFn: listAgents });
  return (monitor: Monitor) => {
    const base = `${monitor.check_type.toUpperCase()} · ${monitor.url}`;
    if (!monitor.agent_id) return base;
    const agent = agents.data?.find((agent) => agent.id === monitor.agent_id)?.name ?? "unknown";
    return `${base} · agent ${agent}`;
  };
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <LayerCard>
      <div className="text-[11px] uppercase tracking-wide text-[var(--coollabs-subtle)]">{label}</div>
      <div className="mt-1 text-xl font-semibold text-[var(--color-fg)]">{value}</div>
    </LayerCard>
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
      description="Failures are counted until the threshold, then Status opens an incident."
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
          <Button type="button" onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={save.isPending}>{save.isPending ? "Saving…" : "Save monitor"}</Button>
        </div>
      </form>
    </Dialog>
  );
}
