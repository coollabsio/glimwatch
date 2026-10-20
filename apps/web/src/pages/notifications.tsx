import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Button, Empty, ErrorText, Field, Input, LayerCard, PageHeader, Select, StatusBadge } from "@/components/ui";
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
      <div className="flex flex-col gap-4">
        <PageHeader title="Notifications" description="Email and webhooks ship today. Slack and Discord use the same incoming-webhook shape." />
        {query.isLoading ? <p className="text-[13px] text-[var(--coollabs-subtle)]">Loading channels…</p> : null}
        {(query.data?.length ?? 0) === 0 && !query.isLoading ? (
          <Empty title="No channels" body="A channel with no monitors attached hears every transition. Link one to narrow it." />
        ) : (
          query.data?.map((channel) => (
            <LayerCard
              key={channel.id}
              title={channel.name}
              description={`${channel.provider} · ${channel.monitor_ids.length === 0 ? "all monitors" : `${channel.monitor_ids.length} monitors`}`}
              actions={<StatusBadge tone={channel.enabled ? "success" : "neutral"}>{channel.enabled ? "Enabled" : "Off"}</StatusBadge>}
            >
              <div className="break-all text-[13px] text-[var(--color-fg-dim)]">{channel.config.to || channel.config.url}</div>
              {channel.deliveries[0] ? (
                <div className="mt-2 text-[11px] text-[var(--coollabs-subtle)]">
                  Last delivery {channel.deliveries[0].status} · {channel.deliveries[0].event} · {formatTime(channel.deliveries[0].created_at)}
                  {channel.deliveries[0].error ? ` · ${channel.deliveries[0].error}` : ""}
                </div>
              ) : null}
              <div className="mt-3 flex gap-2">
                <Button onClick={() => test.mutate(channel.id)}>Send test</Button>
                <Button variant="danger" onClick={() => remove.mutate(channel.id)}>Delete</Button>
              </div>
            </LayerCard>
          ))
        )}
      </div>
      <LayerCard title="Add a channel" className="h-fit">
        <form className="space-y-3" onSubmit={submit}>
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
          <Button className="w-full" type="submit" variant="primary" disabled={save.isPending}>{save.isPending ? "Saving…" : "Add channel"}</Button>
        </form>
      </LayerCard>
    </div>
  );
}
