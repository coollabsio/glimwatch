import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { PageHeader } from "@/components/ui";
import { Badge } from "@/components/issue-badge";
import { Button } from "@/components/ui";
import { Input } from "@/components/ui";
import { api, type Project } from "@/lib/api";

export function ErrorProjectsPage() {
  const navigate = useNavigate();
  const client = useQueryClient();
  const projects = useQuery({ queryKey: ["projects"], queryFn: () => api<Project[]>("/api/error-projects") });
  const [name, setName] = useState("");
  const [platform, setPlatform] = useState("php");
  const [error, setError] = useState("");
  const create = useMutation({
    mutationFn: () =>
      api<Project>("/api/error-projects", { method: "POST", body: JSON.stringify({ name, platform }) }),
    onSuccess: async (project) => {
      setName("");
      await client.invalidateQueries({ queryKey: ["projects"] });
      navigate(`/error-projects/${project.id}`);
    },
    onError: (reason: Error) => setError(reason.message),
  });

  return (
    <div>
      <PageHeader title="Projects" />
      <div className="px-4 py-4">
      <p className="mb-4 text-[13px] text-[var(--color-fg-dim)]">
        Each project has a Sentry DSN. Paste it into an official SDK and errors land in Issues.
      </p>
      <form
        className="card mb-4 flex flex-col gap-3 p-4 md:flex-row md:items-end"
        onSubmit={(event) => {
          event.preventDefault();
          setError("");
          create.mutate();
        }}
      >
        <label className="block flex-1 text-[13px]">
          <span className="mb-1 block text-[var(--color-fg-dim)]">Name</span>
          <Input value={name} required placeholder="PHP checkout" onChange={(event) => setName(event.target.value)} />
        </label>
        <label className="block text-[13px] md:w-44">
          <span className="mb-1 block text-[var(--color-fg-dim)]">Platform</span>
          <select className="field w-full" value={platform} onChange={(event) => setPlatform(event.target.value)}>
            <option value="php">PHP</option>
            <option value="javascript">JavaScript</option>
            <option value="rust">Rust</option>
            <option value="python">Python</option>
            <option value="other">Other</option>
          </select>
        </label>
        <Button variant="highlighted" type="submit" disabled={create.isPending}>
          Create project
        </Button>
      </form>
      {error ? <p className="mb-3 text-[13px] text-error">{error}</p> : null}
      {projects.isLoading ? <p className="text-[13px]">Loading projects...</p> : null}
      {projects.data && projects.data.length === 0 ? (
        <div className="card p-8 text-center text-[13px] text-[var(--color-fg-dim)]">No projects yet.</div>
      ) : null}
      <div className="grid gap-3 md:grid-cols-2">
        {(projects.data ?? []).map((project) => (
          <Link key={project.id} to={`/error-projects/${project.id}`} className="card block p-4 hover:bg-[var(--row-hover)]">
            <div className="flex items-center justify-between gap-2">
              <h2 className="font-medium">{project.name}</h2>
              <Badge>{project.platform}</Badge>
            </div>
            <p className="mt-2 truncate font-mono text-[12px] text-[var(--color-fg-faint)]">{project.dsn}</p>
          </Link>
        ))}
      </div>
      </div>
    </div>
  );
}

export function ProjectDetailPage() {
  const { id = "" } = useParams();
  const navigate = useNavigate();
  const client = useQueryClient();
  const project = useQuery({
    queryKey: ["project", id],
    queryFn: () => api<Project>(`/api/error-projects/${id}`),
  });
  const [copied, setCopied] = useState(false);
  const remove = useMutation({
    mutationFn: () => api(`/api/error-projects/${id}`, { method: "DELETE" }),
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: ["projects"] });
      navigate("/error-projects");
    },
  });

  if (project.isLoading) return <p className="text-[13px]">Loading project...</p>;
  if (project.isError || !project.data) {
    return <p className="text-[13px] text-error">{(project.error as Error)?.message || "Project not found"}</p>;
  }
  const data = project.data;
  const snippets = snippetsFor(data);

  return (
    <div>
      <PageHeader
        title={
          <span className="flex items-center gap-2 text-[13px] font-normal">
            <Link to="/error-projects" className="text-[var(--color-fg-dim)]">
              Projects
            </Link>
            <span className="text-[var(--color-fg-faint)]">/</span>
            <span className="font-medium">{data.name}</span>
          </span>
        }
        actions={
          <Button variant="error" onClick={() => remove.mutate()} disabled={remove.isPending}>
            Delete project
          </Button>
        }
      />
      <div className="px-4 py-4">
      <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-lg font-medium">{data.name}</h1>
          <p className="text-[13px] text-[var(--color-fg-dim)]">
            Project id {data.id} · {data.platform}
          </p>
        </div>
      </div>
      <section className="card p-4">
        <h2 className="text-[13px] font-medium">Sentry DSN</h2>
        <p className="mt-1 text-[13px] text-[var(--color-fg-dim)]">
          Use this with any official Sentry SDK. The public key identifies the project. It is not a secret, which is
          why browser SDKs can ship it.
        </p>
        <div className="mt-3 flex flex-col gap-2 sm:flex-row">
          <Input readOnly value={data.dsn} className="font-mono" onFocus={(event) => event.currentTarget.select()} />
          <Button
            variant="highlighted"
            onClick={async () => {
              await navigator.clipboard.writeText(data.dsn);
              setCopied(true);
            }}
          >
            {copied ? "Copied" : "Copy DSN"}
          </Button>
        </div>
      </section>
      <div className="mt-4 grid gap-3">
        {snippets.map((snippet) => (
          <section key={snippet.title} className="card p-4">
            <h3 className="text-[13px] font-medium">{snippet.title}</h3>
            <pre className="mt-2 overflow-auto rounded-md bg-black/5 p-3 font-mono text-[12px] leading-5 dark:bg-black/30">
              {snippet.code}
            </pre>
          </section>
        ))}
      </div>
      </div>
    </div>
  );
}

function snippetsFor(project: Project) {
  const dsn = project.dsn;
  return [
    {
      title: "PHP (sentry/sentry)",
      code: `\\Sentry\\init([\n  'dsn' => '${dsn}',\n]);`,
    },
    {
      title: "Node (@sentry/node)",
      code: `Sentry.init({\n  dsn: "${dsn}",\n});`,
    },
    {
      title: "Browser (@sentry/browser)",
      code: `Sentry.init({\n  dsn: "${dsn}",\n});`,
    },
    {
      title: "Rust (sentry crate)",
      code: `let _guard = sentry::init("${dsn}");`,
    },
  ];
}
