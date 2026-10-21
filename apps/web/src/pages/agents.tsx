import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Button, Empty, ErrorText, Field, Input, LayerCard, PageHeader, StatusBadge } from "@/components/ui";
import { ApiError, createAgent, type CreatedAgent, deleteAgent, listAgents } from "@/lib/api";
import { timeAgo } from "@/lib/utils";

export function AgentsPage() {
  const client = useQueryClient();
  const query = useQuery({ queryKey: ["agents"], queryFn: listAgents, refetchInterval: 10_000 });
  const [name, setName] = useState("");
  const [fields, setFields] = useState<Record<string, string>>({});
  const [created, setCreated] = useState<CreatedAgent | null>(null);
  const save = useMutation({
    mutationFn: () => createAgent(name),
    onSuccess: async (agent) => {
      setName("");
      setCreated(agent);
      await client.invalidateQueries({ queryKey: ["agents"] });
    },
    onError: (error) => setFields(error instanceof ApiError ? error.fields ?? {} : {}),
  });
  const remove = useMutation({
    mutationFn: deleteAgent,
    onSuccess: () => client.invalidateQueries({ queryKey: ["agents"] }),
  });

  function submit(event: FormEvent) {
    event.preventDefault();
    setFields({});
    save.mutate();
  }

  return (
    <div className="grid gap-6 lg:grid-cols-[1fr_320px]">
      <div className="flex flex-col gap-4">
        <PageHeader title="Agents" description="Agents run checks inside private networks. Each agent reads its checks from a config file on its host and reports results over outbound HTTPS, so nothing needs to be exposed." />
        {query.isLoading ? <p className="text-[13px] text-[var(--coollabs-subtle)]">Loading agents…</p> : null}
        {(query.data?.length ?? 0) === 0 && !query.isLoading ? (
          <Empty title="No agents" body="Add one, list the checks in its config file, and run it next to the services you want to check. Its checks show up under Monitors." />
        ) : (
          query.data?.map((agent) => (
            <LayerCard
              key={agent.id}
              title={agent.name}
              description={`${agent.monitor_count} ${agent.monitor_count === 1 ? "monitor" : "monitors"} · last seen ${timeAgo(agent.last_seen_at)}`}
              actions={<StatusBadge tone={agent.online ? "success" : "neutral"}>{agent.online ? "Online" : "Offline"}</StatusBadge>}
            >
              <Button variant="danger" onClick={() => remove.mutate(agent.id)} disabled={remove.isPending && remove.variables === agent.id}>Delete</Button>
              {remove.variables === agent.id ? <ErrorText error={remove.error} /> : null}
            </LayerCard>
          ))
        )}
      </div>
      {created ? (
        <AgentToken agent={created} onDone={() => setCreated(null)} />
      ) : (
        <LayerCard title="Add an agent" className="h-fit">
          <form className="space-y-3" onSubmit={submit}>
            <Field label="Name" hint="shown as the check region" error={fields.name}>
              <Input value={name} onChange={(event) => setName(event.target.value)} placeholder="office-vpc" required />
            </Field>
            <ErrorText error={fields.name ? null : save.error} />
            <Button className="w-full" type="submit" variant="primary" disabled={save.isPending}>{save.isPending ? "Saving…" : "Add agent"}</Button>
          </form>
        </LayerCard>
      )}
    </div>
  );
}

const configPath = "/etc/status/agent.toml";

const exampleConfig = `[[check]]
name = "Main DB"
type = "postgres"          # http, tcp, postgres, mysql, redis
target = "localhost:5432"
user = "status"
password_env = "PG_PASSWORD"
database = "app"
interval = 30`;

function AgentToken({ agent, onDone }: { agent: CreatedAgent; onDone: () => void }) {
  const origin = window.location.origin;
  const docker = `docker run -d --name status-agent --restart unless-stopped --no-healthcheck -e STATUS_AGENT_SERVER=${origin} -e STATUS_AGENT_TOKEN=${agent.token} -e STATUS_AGENT_CONFIG=${configPath} -e PG_PASSWORD=... -v ${configPath}:${configPath}:ro ghcr.io/coollabsio/status:latest agent`;
  const binary = `PG_PASSWORD=... status agent --server ${origin} --token ${agent.token} --config ${configPath}`;
  return (
    <LayerCard title={`${agent.name} is ready`} description="Copy the token now. Status stores only its hash and cannot show it again." className="h-fit">
      <div className="space-y-3">
        <Snippet label="Token" value={agent.token} />
        <Snippet label={configPath} value={exampleConfig} multiline />
        <p className="text-[12px] text-[var(--coollabs-subtle)]">Passwords stay on the agent host. The config names an environment variable, not the password.</p>
        <Snippet label="Docker" value={docker} />
        <Snippet label="Binary" value={binary} />
        <Button className="w-full" variant="primary" onClick={onDone}>Done</Button>
      </div>
    </LayerCard>
  );
}

function Snippet({ label, value, multiline = false }: { label: string; value: string; multiline?: boolean }) {
  const [copied, setCopied] = useState(false);

  async function copy() {
    await navigator.clipboard.writeText(value);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  return (
    <div>
      <div className="field-label">
        <span>{label}</span>
        <button type="button" className="ml-auto text-[11px] font-normal hover:text-[var(--color-fg)]" onClick={() => void copy()}>
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <code className={`block select-all ${multiline ? "whitespace-pre overflow-x-auto" : "break-all"} rounded-md border border-neutral-200 bg-neutral-50 px-2.5 py-2 font-mono text-[12px] text-[var(--color-fg)] dark:border-white/[0.08] dark:bg-white/[0.03]`}>
        {value}
      </code>
    </div>
  );
}
