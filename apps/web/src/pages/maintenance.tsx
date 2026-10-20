import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Badge, Button, Card, Empty, ErrorText, Field, Input, Select, Textarea } from "@/components/ui";
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
      <div className="space-y-4">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Maintenance</h1>
          <p className="text-sm text-muted-foreground">Checks inside a window are recorded, not counted as downtime, and do not page anyone.</p>
        </div>
        {query.isLoading ? <p className="text-sm text-muted-foreground">Loading windows…</p> : null}
        {(query.data?.length ?? 0) === 0 && !query.isLoading ? (
          <Empty title="No windows" body="Schedule one before a failover or a deploy so the status page stays calm." />
        ) : (
          query.data?.map((item) => (
            <Card key={item.id} className="p-4">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <div className="font-medium">{item.title}</div>
                  <div className="text-xs text-muted-foreground">{formatTime(item.starts_at)} – {formatTime(item.ends_at)}</div>
                </div>
                <Badge className={item.active ? "bg-sky-500/15 text-sky-800 dark:text-sky-200" : "bg-muted text-muted-foreground"}>
                  {item.active ? "In progress" : "Scheduled"}
                </Badge>
              </div>
              <p className="mt-2 text-sm text-muted-foreground">{item.message}</p>
              <Button className="mt-3" variant="outline" size="sm" onClick={() => remove.mutate(item.id)}>Remove</Button>
            </Card>
          ))
        )}
      </div>
      <Card className="h-fit p-4">
        <h2 className="font-medium">Schedule a window</h2>
        <form className="mt-3 space-y-3" onSubmit={submit}>
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
          <Button type="submit" className="w-full" disabled={save.isPending}>{save.isPending ? "Saving…" : "Schedule"}</Button>
        </form>
      </Card>
    </div>
  );
}
