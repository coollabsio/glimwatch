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
  const [search, setSearch] = useState("");
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

  const agents = (query.data ?? []).filter((agent) => agent.name.toLowerCase().includes(search.trim().toLowerCase()));

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Agents" description="Agents run checks inside private networks. Each agent reads its checks from a config file on its host and reports results over outbound HTTPS, so nothing needs to be exposed." />
      <div className="grid items-start gap-6 lg:grid-cols-[minmax(0,1fr)_340px]">
        {query.isLoading ? (
          <p className="text-[13px] text-[var(--ui-fg-faint)]">Loading agents…</p>
        ) : (query.data?.length ?? 0) === 0 ? (
          <Empty title="No agents" body="Add one, list the checks in its config file, and run it next to the services you want to check. Its checks show up under Monitors." />
        ) : (
          <LayerCard title="All agents" flush>
            <div className="flex items-center gap-2 border-b border-[var(--ui-divider)] p-2">
              <Input className="max-w-64" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search agents" aria-label="Search agents" />
            </div>
            <div className="overflow-x-auto">
              <table className="cool-table">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Status</th>
                    <th className="hidden sm:table-cell">Monitors</th>
                    <th className="hidden md:table-cell">Last seen</th>
                    <th className="w-0" />
                  </tr>
                </thead>
                <tbody>
                  {agents.map((agent) => (
                    <tr key={agent.id}>
                      <td className="font-medium text-[var(--ui-fg)]">{agent.name}</td>
                      <td><StatusBadge tone={agent.online ? "success" : "neutral"}>{agent.online ? "Online" : "Offline"}</StatusBadge></td>
                      <td className="hidden sm:table-cell">{agent.monitor_count}</td>
                      <td className="hidden md:table-cell">{timeAgo(agent.last_seen_at)}</td>
                      <td className="text-right">
                        <Button variant="danger" onClick={() => remove.mutate(agent.id)} disabled={remove.isPending && remove.variables === agent.id}>Delete</Button>
                        {remove.variables === agent.id ? <ErrorText error={remove.error} /> : null}
                      </td>
                    </tr>
                  ))}
                  {agents.length === 0 ? (
                    <tr>
                      <td colSpan={5} className="text-center text-[var(--ui-fg-faint)]">No agents match your search.</td>
                    </tr>
                  ) : null}
                </tbody>
              </table>
            </div>
          </LayerCard>
        )}
        {created ? (
          <AgentToken agent={created} onDone={() => setCreated(null)} />
        ) : (
          <LayerCard title="Add an agent" description="The name is shown as the check region.">
            <form className="flex flex-col gap-4" onSubmit={submit}>
              <Field label="Name" error={fields.name}>
                <Input value={name} onChange={(event) => setName(event.target.value)} placeholder="office-vpc" required />
              </Field>
              <ErrorText error={fields.name ? null : save.error} />
              <div className="flex justify-end">
                <Button type="submit" variant="highlighted" disabled={save.isPending}>{save.isPending ? "Saving…" : "Add agent"}</Button>
              </div>
            </form>
          </LayerCard>
        )}
      </div>
    </div>
  );
}

const configPath = "/etc/owlee/agent.toml";

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
  const docker = `docker run -d --name owlee-agent --restart unless-stopped --no-healthcheck -e OWLEE_AGENT_SERVER=${origin} -e OWLEE_AGENT_TOKEN=${agent.token} -e OWLEE_AGENT_CONFIG=${configPath} -e PG_PASSWORD=... -v ${configPath}:${configPath}:ro ghcr.io/coollabsio/owlee:latest agent`;
  const binary = `PG_PASSWORD=... owlee agent --server ${origin} --token ${agent.token} --config ${configPath}`;
  return (
    <LayerCard title={`${agent.name} is ready`} description="Copy the token now. Owlee stores only its hash and cannot show it again." >
      <div className="flex flex-col gap-4">
        <Snippet label="Token" value={agent.token} />
        <Snippet label={configPath} value={exampleConfig} multiline />
        <p className="text-[12px] text-[var(--ui-fg-faint)]">Passwords stay on the agent host. The config names an environment variable, not the password.</p>
        <Snippet label="Docker" value={docker} />
        <Snippet label="Binary" value={binary} />
        <div className="flex justify-end">
          <Button variant="highlighted" onClick={onDone}>Done</Button>
        </div>
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
        <button type="button" className="ml-auto text-[11px] font-normal hover:text-[var(--ui-fg)]" onClick={() => void copy()}>
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <code className={`block select-all ${multiline ? "whitespace-pre overflow-x-auto" : "break-all"} rounded-lg border border-[var(--ui-control-border)] bg-[var(--ui-control-bg)] px-2.5 py-2 font-mono text-[12px] text-[var(--ui-fg)]`}>
        {value}
      </code>
    </div>
  );
}
