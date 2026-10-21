import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Button, ErrorText, Field, Input, LayerCard, PageHeader, Select, StatusBadge, Textarea } from "@/components/ui";
import { ApiError, api, type Maintenance, type Monitor, type StatusPage } from "@/lib/api";
import { formatTime, fromLocalInput, toLocalInput } from "@/lib/utils";

export function MaintenancePage() {
  const client = useQueryClient();
  const query = useQuery({ queryKey: ["maintenances"], queryFn: () => api<Maintenance[]>("/api/maintenances") });
  const pages = useQuery({ queryKey: ["pages"], queryFn: () => api<StatusPage[]>("/api/pages") });
  const monitors = useQuery({ queryKey: ["monitors"], queryFn: () => api<Monitor[]>("/api/monitors") });
  const [title, setTitle] = useState("");
  const [message, setMessage] = useState("");
  const [pageId, setPageId] = useState("");
  const [monitorId, setMonitorId] = useState("");
  const [starts, setStarts] = useState(toLocalInput());
  const [ends, setEnds] = useState(toLocalInput(new Date(Date.now() + 2 * 60 * 60 * 1000).toISOString()));
  const [fields, setFields] = useState<Record<string, string>>({});
  const save = useMutation({
    mutationFn: () =>
      api<Maintenance>("/api/maintenances", {
        method: "POST",
        body: JSON.stringify({
          title,
          message,
          page_id: pageId,
          monitor_ids: monitorId ? [monitorId] : [],
          starts_at: fromLocalInput(starts),
          ends_at: fromLocalInput(ends),
        }),
      }),
    onSuccess: async () => {
      setTitle("");
      setMessage("");
      await client.invalidateQueries({ queryKey: ["maintenances"] });
    },
    onError: (error) => setFields(error instanceof ApiError ? error.fields ?? {} : {}),
  });
  const remove = useMutation({
    mutationFn: (id: string) => api(`/api/maintenances/${id}`, { method: "DELETE" }),
    onSuccess: () => client.invalidateQueries({ queryKey: ["maintenances"] }),
  });

  function submit(event: FormEvent) {
    event.preventDefault();
    setFields({});
    save.mutate();
  }

  const windows = query.data ?? [];

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Maintenance" description="Checks inside a window are recorded, not counted as downtime, and do not page anyone." />
      <div className="grid items-start gap-6 lg:grid-cols-[1fr_340px]">
        <LayerCard title="Windows" description={query.data ? `${windows.length} scheduled` : undefined} flush>
          {query.isLoading ? (
            <CardText title="Loading windows…" />
          ) : windows.length === 0 ? (
            <CardText title="No windows" body="Schedule one before a failover or a deploy so the status page stays calm." />
          ) : (
            <div className="overflow-x-auto">
              <table className="cool-table">
                <thead>
                  <tr>
                    <th>Window</th>
                    <th className="hidden md:table-cell">When</th>
                    <th>Status</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {windows.map((item) => (
                    <tr key={item.id}>
                      <td className="max-w-[24rem]">
                        <div className="font-medium text-[var(--ui-fg)]">{item.title}</div>
                        <div className="truncate text-[11px] text-[var(--ui-fg-faint)]">{item.message || "-"}</div>
                        <div className="text-[11px] text-[var(--ui-fg-faint)] md:hidden">{formatTime(item.starts_at)} - {formatTime(item.ends_at)}</div>
                      </td>
                      <td className="hidden whitespace-nowrap md:table-cell">{formatTime(item.starts_at)} - {formatTime(item.ends_at)}</td>
                      <td>
                        <StatusBadge tone={item.active ? "info" : "neutral"}>{item.active ? "In progress" : "Scheduled"}</StatusBadge>
                      </td>
                      <td className="text-right">
                        <Button size="sm" onClick={() => remove.mutate(item.id)}>Remove</Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </LayerCard>
        <LayerCard title="Schedule a window" description="Shown on the status page while it runs.">
          <form className="space-y-3" onSubmit={submit}>
            <Field label="Title" error={fields.title}><Input value={title} onChange={(event) => setTitle(event.target.value)} required /></Field>
            <Field label="Message" error={fields.message}><Textarea value={message} onChange={(event) => setMessage(event.target.value)} required /></Field>
            <Field label="Status page" error={fields.page_id}>
              <Select
                value={pageId}
                options={(pages.data ?? []).map((page) => ({ value: page.id, label: page.title }))}
                onChange={setPageId}
                aria-label="Status page"
              />
            </Field>
            <Field label="Monitor" hint="blank = whole page">
              <Select
                value={monitorId}
                options={[{ value: "", label: "Every monitor on the page" }, ...(monitors.data ?? []).map((monitor) => ({ value: monitor.id, label: monitor.name }))]}
                onChange={setMonitorId}
                aria-label="Monitor"
              />
            </Field>
            <Field label="Starts" error={fields.starts_at}><Input type="datetime-local" value={starts} onChange={(event) => setStarts(event.target.value)} required /></Field>
            <Field label="Ends" error={fields.ends_at}><Input type="datetime-local" value={ends} onChange={(event) => setEnds(event.target.value)} required /></Field>
            <ErrorText error={save.error || remove.error} />
            <div className="flex justify-end pt-1">
              <Button type="submit" variant="primary" disabled={save.isPending || !pageId}>{save.isPending ? "Saving…" : "Schedule"}</Button>
            </div>
          </form>
        </LayerCard>
      </div>
    </div>
  );
}

function CardText({ title, body }: { title: string; body?: string }) {
  return (
    <div className="flex min-h-32 flex-col items-center justify-center px-4 py-8 text-center">
      <p className="text-[14px] font-medium text-[var(--ui-fg)]">{title}</p>
      {body ? <p className="mx-auto mt-1 max-w-[28rem] text-[13px] text-[var(--ui-fg-faint)]">{body}</p> : null}
    </div>
  );
}
