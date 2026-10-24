import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { Badge } from "@/components/issue-badge";
import { Button, Field, Input, LayerCard, PageHeader, Select } from "@/components/ui";
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
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Projects"
        description="Each project has a Sentry DSN. Paste it into an official SDK and errors land in Issues."
      />
      <LayerCard title="New project" description="Pick the platform you will install the SDK on.">
        <form
          className="flex flex-col gap-3 md:flex-row md:items-end"
          onSubmit={(event) => {
            event.preventDefault();
            setError("");
            create.mutate();
          }}
        >
          <div className="flex-1">
            <Field label="Name">
              <Input value={name} required placeholder="PHP checkout" onChange={(event) => setName(event.target.value)} />
            </Field>
          </div>
          <div className="md:w-44">
            <Field label="Platform">
              <Select aria-label="Platform" value={platform} onChange={setPlatform} options={platforms} />
            </Field>
          </div>
          <Button variant="highlighted" type="submit" disabled={create.isPending}>
            Create project
          </Button>
        </form>
        {error ? <p className="field-error">{error}</p> : null}
      </LayerCard>
      <LayerCard title="All projects" flush>
        {projects.isLoading ? <p className="p-4 text-[13px] text-[var(--ui-fg-faint)]">Loading projects...</p> : null}
        {projects.data && projects.data.length === 0 ? (
          <div className="flex min-h-40 flex-col items-center justify-center px-4 py-10 text-center">
            <h2 className="text-[14px] font-medium text-[var(--ui-fg)]">No projects yet</h2>
            <p className="mx-auto mt-1 max-w-[28rem] text-[13px] text-[var(--ui-fg-faint)]">
              Create a project above to get a DSN for your SDK.
            </p>
          </div>
        ) : null}
        {projects.data && projects.data.length > 0 ? (
          <div className="overflow-x-auto">
            <table className="cool-table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Platform</th>
                  <th className="hidden md:table-cell">DSN</th>
                </tr>
              </thead>
              <tbody>
                {projects.data.map((project) => (
                  <tr key={project.id} className="clickable-row" onClick={() => navigate(`/error-projects/${project.id}`)}>
                    <td>
                      <Link className="row-link" to={`/error-projects/${project.id}`} onClick={(event) => event.stopPropagation()}>
                        {project.name}
                      </Link>
                    </td>
                    <td>
                      <Badge>{platformLabel(project.platform)}</Badge>
                    </td>
                    <td className="hidden max-w-0 md:table-cell md:w-1/2">
                      <span className="block truncate font-mono text-[12px] text-[var(--ui-fg-faint)]">{project.dsn}</span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : null}
      </LayerCard>
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

  if (project.isLoading) return <p className="text-[13px] text-[var(--ui-fg-faint)]">Loading project...</p>;
  if (project.isError || !project.data) {
    return <p className="text-[13px] text-[var(--ui-error)]">{(project.error as Error)?.message || "Project not found"}</p>;
  }
  const data = project.data;
  const snippets = snippetsFor(data);

  return (
    <div className="flex flex-col gap-6">
      <div>
        <nav className="mb-2 flex items-center gap-2 text-[13px] text-[var(--ui-fg-faint)]">
          <Link to="/error-projects">
            Projects
          </Link>
          <span>/</span>
          <span className="text-[var(--ui-fg-dim)]">{data.name}</span>
        </nav>
        <PageHeader
          title={data.name}
          keepTitle
          description={`Project id ${data.id} · ${platformLabel(data.platform)}`}
          actions={
            <Button variant="danger" onClick={() => remove.mutate()} disabled={remove.isPending}>
              Delete project
            </Button>
          }
        />
      </div>
      <LayerCard
        title="Sentry DSN"
        description="Use this with any official Sentry SDK. The public key identifies the project. It is not a secret, which is why browser SDKs can ship it."
        actions={
          <Button
            variant="highlighted"
            onClick={async () => {
              await navigator.clipboard.writeText(data.dsn);
              setCopied(true);
            }}
          >
            {copied ? "Copied" : "Copy DSN"}
          </Button>
        }
      >
        <Input readOnly value={data.dsn} className="font-mono text-[13px]" onFocus={(event) => event.currentTarget.select()} />
      </LayerCard>
      <LayerCard title="SDK setup" description="Initialize the SDK with this DSN as early as possible in your app.">
        <div className="grid gap-4 lg:grid-cols-2">
          {snippets.map((snippet) => (
            <div key={snippet.title}>
              <h3 className="field-label">{snippet.title}</h3>
              <pre className="overflow-auto rounded-lg border border-[var(--ui-control-border)] bg-[var(--ui-control-bg)] p-3 font-mono text-[12px] leading-5 text-[var(--ui-fg)]">
                {snippet.code}
              </pre>
            </div>
          ))}
        </div>
      </LayerCard>
    </div>
  );
}

const platforms = [
  { value: "php", label: "PHP" },
  { value: "javascript", label: "JavaScript" },
  { value: "rust", label: "Rust" },
  { value: "python", label: "Python" },
  { value: "other", label: "Other" },
];

function platformLabel(platform: string) {
  return platforms.find((item) => item.value === platform)?.label ?? platform;
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
