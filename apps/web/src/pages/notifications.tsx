import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Badge, Button, Card, Empty, ErrorText, Field, Input, Select } from "@/components/ui";
import { ApiError, api, type Channel, type Monitor } from "@/lib/api";
import { formatTime } from "@/lib/utils";

const providers = ["email", "webhook", "slack", "discord"] as const;

export function NotificationsPage() {
  const client = useQueryClient();
  const query = useQuery({ queryKey: ["notifications"], queryFn: () => api<Channel[]>("/api/notifications") });
  const monitors = useQuery({ queryKey: ["monitors"], queryFn: () => api<Monitor[]>("/api/monitors") });
  const [name, setName] = useState("");
  const [provider, setProvider] = useState<(typeof providers)[number]>("email");
  const [target, setTarget] = useState("");
  const [secret, setSecret] = useState("");
  const [monitorId, setMonitorId] = useState("");
  const [fields, setFields] = useState<Record<string, string>>({});
  const save = useMutation({
    mutationFn: () =>
      api<Channel>("/api/notifications", {
        method: "POST",
        body: JSON.stringify({
          name,
          provider,
          config: provider === "email" ? { to: target } : { url: target, secret },
          monitor_ids: monitorId ? [monitorId] : [],
          enabled: true,
        }),
      }),
    onSuccess: async () => {
      setName("");
      setTarget("");
      setSecret("");
      await client.invalidateQueries({ queryKey: ["notifications"] });
    },
    onError: (error) => setFields(error instanceof ApiError ? error.fields ?? {} : {}),
  });
  const test = useMutation({
    mutationFn: (id: string) => api<Channel>(`/api/notifications/${id}/test`, { method: "POST" }),
    onSuccess: () => client.invalidateQueries({ queryKey: ["notifications"] }),
  });
  const remove = useMutation({
    mutationFn: (id: string) => api(`/api/notifications/${id}`, { method: "DELETE" }),
    onSuccess: () => client.invalidateQueries({ queryKey: ["notifications"] }),
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
          <h1 className="text-2xl font-semibold tracking-tight">Notifications</h1>
          <p className="text-sm text-muted-foreground">Email and webhooks ship today. Slack and Discord use the same incoming-webhook shape, so another provider is a new formatter.</p>
        </div>
        {query.isLoading ? <p className="text-sm text-muted-foreground">Loading channels…</p> : null}
        {(query.data?.length ?? 0) === 0 && !query.isLoading ? (
          <Empty title="No channels" body="A channel with no monitors attached hears every transition. Link one to narrow it." />
        ) : (
          query.data?.map((channel) => (
            <Card key={channel.id} className="space-y-3 p-4">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <div className="font-medium">{channel.name}</div>
                  <div className="text-xs text-muted-foreground">{channel.provider} · {channel.monitor_ids.length === 0 ? "all monitors" : `${channel.monitor_ids.length} monitors`}</div>
                </div>
                <Badge className={channel.enabled ? "bg-emerald-500/15 text-emerald-700 dark:text-emerald-300" : "bg-muted text-muted-foreground"}>
                  {channel.enabled ? "Enabled" : "Off"}
                </Badge>
              </div>
              <div className="text-sm text-muted-foreground break-all">{channel.config.to || channel.config.url}</div>
              {channel.deliveries[0] ? (
                <div className="text-xs text-muted-foreground">
                  Last delivery {channel.deliveries[0].status} · {channel.deliveries[0].event} · {formatTime(channel.deliveries[0].created_at)}
                  {channel.deliveries[0].error ? ` · ${channel.deliveries[0].error}` : ""}
                </div>
              ) : null}
              <div className="flex gap-2">
                <Button size="sm" variant="outline" onClick={() => test.mutate(channel.id)}>Send test</Button>
                <Button size="sm" variant="destructive" onClick={() => remove.mutate(channel.id)}>Delete</Button>
              </div>
            </Card>
          ))
        )}
      </div>
      <Card className="h-fit p-4">
        <h2 className="font-medium">Add a channel</h2>
        <form className="mt-3 space-y-3" onSubmit={submit}>
          <Field label="Name" error={fields.name}><Input value={name} onChange={(event) => setName(event.target.value)} required /></Field>
          <Field label="Provider" error={fields.provider}>
            <Select value={provider} onChange={(event) => setProvider(event.target.value as (typeof providers)[number])}>
              {providers.map((item) => <option key={item} value={item}>{item}</option>)}
            </Select>
          </Field>
          <Field label={provider === "email" ? "Recipient" : "Webhook URL"} error={fields.config}>
            <Input value={target} onChange={(event) => setTarget(event.target.value)} placeholder={provider === "email" ? "ops@coollabs.io" : "https://"} required />
          </Field>
          {provider === "webhook" ? (
            <Field label="Signing secret" hint="optional">
              <Input value={secret} onChange={(event) => setSecret(event.target.value)} />
            </Field>
          ) : null}
          <Field label="Monitor" hint="blank = all">
            <Select value={monitorId} onChange={(event) => setMonitorId(event.target.value)}>
              <option value="">All monitors</option>
              {monitors.data?.map((monitor) => <option key={monitor.id} value={monitor.id}>{monitor.name}</option>)}
            </Select>
          </Field>
          <ErrorText error={save.error || test.error} />
          <Button className="w-full" type="submit" disabled={save.isPending}>{save.isPending ? "Saving…" : "Add channel"}</Button>
        </form>
      </Card>
    </div>
  );
}
