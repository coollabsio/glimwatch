import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Button, ErrorText, Field, Input, LayerCard, PageHeader, Select, StatusBadge } from "@/components/ui";
import { ApiError, api, type Channel, type Monitor } from "@/lib/api";
import { formatTime } from "@/lib/utils";

const providers = ["email", "webhook", "slack", "discord"] as const;
const providerLabel: Record<(typeof providers)[number], string> = { email: "Email", webhook: "Webhook", slack: "Slack", discord: "Discord" };

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

  const channels = query.data ?? [];

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Notifications" description="Email and webhooks ship today. Slack and Discord use the same incoming-webhook shape." />
      <div className="grid items-start gap-6 lg:grid-cols-[1fr_340px]">
        <LayerCard title="Channels" description={query.data ? `${channels.length} total` : undefined} flush>
          {query.isLoading ? (
            <CardText title="Loading channels…" />
          ) : channels.length === 0 ? (
            <CardText title="No channels" body="A channel with no monitors attached hears every transition. Link one to narrow it." />
          ) : (
            <div className="overflow-x-auto">
              <table className="cool-table">
                <thead>
                  <tr>
                    <th>Channel</th>
                    <th className="hidden md:table-cell">Scope</th>
                    <th className="hidden xl:table-cell">Last delivery</th>
                    <th>Status</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {channels.map((channel) => {
                    const last = channel.deliveries[0];
                    return (
                      <tr key={channel.id}>
                        <td className="max-w-[20rem]">
                          <div className="font-medium text-[var(--ui-fg)]">{channel.name}</div>
                          <div className="truncate text-[11px] text-[var(--ui-fg-faint)]">
                            {providerLabel[channel.provider]} · {channel.config.to || channel.config.url}
                          </div>
                        </td>
                        <td className="hidden whitespace-nowrap md:table-cell">
                          {channel.monitor_ids.length === 0 ? "All monitors" : `${channel.monitor_ids.length} monitors`}
                        </td>
                        <td className="hidden max-w-[16rem] xl:table-cell">
                          {last ? (
                            <>
                              <div className="truncate">{last.status} · {last.event}</div>
                              <div className="truncate text-[11px] text-[var(--ui-fg-faint)]">
                                {formatTime(last.created_at)}
                                {last.error ? ` · ${last.error}` : ""}
                              </div>
                            </>
                          ) : (
                            "-"
                          )}
                        </td>
                        <td>
                          <StatusBadge tone={channel.enabled ? "success" : "neutral"}>{channel.enabled ? "Enabled" : "Off"}</StatusBadge>
                        </td>
                        <td>
                          <div className="flex justify-end gap-2">
                            <Button size="sm" onClick={() => test.mutate(channel.id)}>Send test</Button>
                            <Button size="sm" variant="danger" onClick={() => remove.mutate(channel.id)}>Delete</Button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </LayerCard>
        <LayerCard title="Add a channel" description="Alerts fire when a monitor changes state.">
          <form className="space-y-3" onSubmit={submit}>
            <Field label="Name" error={fields.name}><Input value={name} onChange={(event) => setName(event.target.value)} required /></Field>
            <Field label="Provider" error={fields.provider}>
              <Select value={provider} options={providers.map((item) => ({ value: item, label: providerLabel[item] }))} onChange={setProvider} aria-label="Provider" />
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
              <Select
                value={monitorId}
                options={[{ value: "", label: "All monitors" }, ...(monitors.data ?? []).map((monitor) => ({ value: monitor.id, label: monitor.name }))]}
                onChange={setMonitorId}
                aria-label="Monitor"
              />
            </Field>
            <ErrorText error={save.error || test.error || remove.error} />
            <div className="flex justify-end pt-1">
              <Button type="submit" variant="primary" disabled={save.isPending}>{save.isPending ? "Saving…" : "Add channel"}</Button>
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
