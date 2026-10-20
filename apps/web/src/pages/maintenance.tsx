import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Button, Empty, ErrorText, Field, Input, LayerCard, PageHeader, Select, StatusBadge, Textarea } from "@/components/ui";
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

  return (
    <div className="grid gap-6 lg:grid-cols-[1fr_320px]">
      <div className="flex flex-col gap-4">
        <PageHeader title="Maintenance" description="Checks inside a window are recorded, not counted as downtime, and do not page anyone." />
        {query.isLoading ? <p className="text-[13px] text-[var(--coollabs-subtle)]">Loading windows…</p> : null}
        {(query.data?.length ?? 0) === 0 && !query.isLoading ? (
          <Empty title="No windows" body="Schedule one before a failover or a deploy so the status page stays calm." />
        ) : (
          query.data?.map((item) => (
            <LayerCard
              key={item.id}
              title={item.title}
              description={`${formatTime(item.starts_at)} - ${formatTime(item.ends_at)}`}
              actions={<StatusBadge tone={item.active ? "info" : "neutral"}>{item.active ? "In progress" : "Scheduled"}</StatusBadge>}
            >
              <p className="text-[13px] text-[var(--color-fg-dim)]">{item.message}</p>
              <Button className="mt-3" onClick={() => remove.mutate(item.id)}>Remove</Button>
            </LayerCard>
          ))
        )}
      </div>
      <LayerCard title="Schedule a window" className="h-fit">
        <form className="space-y-3" onSubmit={submit}>
          <Field label="Title" error={fields.title}><Input value={title} onChange={(event) => setTitle(event.target.value)} required /></Field>
          <Field label="Message" error={fields.message}><Textarea value={message} onChange={(event) => setMessage(event.target.value)} required /></Field>
          <Field label="Status page" error={fields.page_id}>
            <Select value={pageId} onChange={(event) => setPageId(event.target.value)} required>
              <option value="">Choose</option>
              {pages.data?.map((page) => <option key={page.id} value={page.id}>{page.title}</option>)}
            </Select>
          </Field>
          <Field label="Monitor" hint="blank = whole page">
            <Select value={monitorId} onChange={(event) => setMonitorId(event.target.value)}>
              <option value="">Every monitor on the page</option>
              {monitors.data?.map((monitor) => <option key={monitor.id} value={monitor.id}>{monitor.name}</option>)}
            </Select>
          </Field>
          <Field label="Starts" error={fields.starts_at}><Input type="datetime-local" value={starts} onChange={(event) => setStarts(event.target.value)} required /></Field>
          <Field label="Ends" error={fields.ends_at}><Input type="datetime-local" value={ends} onChange={(event) => setEnds(event.target.value)} required /></Field>
          <ErrorText error={save.error} />
          <Button type="submit" variant="primary" className="w-full" disabled={save.isPending}>{save.isPending ? "Saving…" : "Schedule"}</Button>
        </form>
      </LayerCard>
    </div>
  );
}
