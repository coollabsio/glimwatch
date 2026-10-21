import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router";
import { PageHeader } from "@/components/ui";
import { PriorityMark, Sparkline } from "@/components/error-charts";
import { Badge } from "@/components/issue-badge";
import { Button } from "@/components/ui";
import { api, type Issue, type IssueFeed, type Project } from "@/lib/api";
import { cn, formatTime, fullTime } from "@/lib/utils";

const ranges = ["24h", "7d", "14d", "30d"] as const;
const sorts = [
  { id: "last_seen", label: "Last seen" },
  { id: "age", label: "Age" },
  { id: "events", label: "Events" },
  { id: "users", label: "Users" },
] as const;

type SavedView = {
  name: string;
  status: string;
  projectId: string;
  environment: string;
  range: string;
  q: string;
  sort: string;
};

export function IssuesPage() {
  const navigate = useNavigate();
  const client = useQueryClient();
  const [status, setStatus] = useState("unresolved");
  const [projectId, setProjectId] = useState("");
  const [environment, setEnvironment] = useState("");
  const [range, setRange] = useState<(typeof ranges)[number]>("14d");
  const [q, setQ] = useState("");
  const [draft, setDraft] = useState("");
  const [sort, setSort] = useState("last_seen");
  const [trend, setTrend] = useState<"24h" | "14d">("14d");
  const [selected, setSelected] = useState<string[]>([]);
  const [saveOpen, setSaveOpen] = useState(false);
  const [saveName, setSaveName] = useState("");
  const [saved, setSaved] = useState<SavedView[]>([]);

  useEffect(() => {
    const raw = localStorage.getItem("haruvo-error-views");
    if (raw) setSaved(JSON.parse(raw) as SavedView[]);
  }, []);

  const projects = useQuery({ queryKey: ["projects"], queryFn: () => api<Project[]>("/api/error-projects") });
  const params = useMemo(() => {
    const search = new URLSearchParams();
    if (status && status !== "all") search.set("status", status);
    if (projectId) search.set("project_id", projectId);
    if (environment) search.set("environment", environment);
    if (range) search.set("since", range);
    if (q.trim()) search.set("q", q.trim());
    if (sort) search.set("sort", sort);
    return search.toString();
  }, [status, projectId, environment, range, q, sort]);
  const feed = useQuery({
    queryKey: ["issues", params],
    queryFn: () => api<IssueFeed>(`/api/error-issues?${params}`),
    refetchInterval: 4000,
  });
  const bulk = useMutation({
    mutationFn: (next: string) =>
      api("/api/error-issues/bulk", { method: "POST", body: JSON.stringify({ ids: selected, status: next }) }),
    onSuccess: async () => {
      setSelected([]);
      await client.invalidateQueries({ queryKey: ["issues"] });
    },
  });

  const issues = feed.data?.issues ?? [];
  const allChecked = issues.length > 0 && selected.length === issues.length;

  function applySaved(view: SavedView) {
    setStatus(view.status);
    setProjectId(view.projectId);
    setEnvironment(view.environment);
    setRange(view.range as (typeof ranges)[number]);
    setQ(view.q);
    setSort(view.sort);
  }

  function saveView() {
    const name = saveName.trim();
    if (!name) return;
    const next = [...saved.filter((view) => view.name !== name), { name, status, projectId, environment, range, q, sort }];
    setSaved(next);
    localStorage.setItem("haruvo-error-views", JSON.stringify(next));
    setSaveName("");
    setSaveOpen(false);
  }

  return (
    <div className="flex min-h-full flex-col">
      <PageHeader title="Issues" />
      <div className="flex flex-wrap items-center gap-2 border-b border-[var(--coollabs-hairline)] px-4 py-2">
        <select className="field w-40" value={projectId} onChange={(event) => setProjectId(event.target.value)}>
          <option value="">All projects</option>
          {(projects.data ?? []).map((project) => (
            <option key={project.id} value={project.id}>
              {project.name}
            </option>
          ))}
        </select>
        <select className="field w-40" value={environment} onChange={(event) => setEnvironment(event.target.value)}>
          <option value="">All envs</option>
          {(feed.data?.environments ?? []).map((item) => (
            <option key={item} value={item}>
              {item}
            </option>
          ))}
        </select>
        <select className="field w-[88px]" value={range} onChange={(event) => setRange(event.target.value as (typeof ranges)[number])}>
          {ranges.map((item) => (
            <option key={item} value={item}>
              {item.toUpperCase()}
            </option>
          ))}
        </select>
        <form
          className="flex min-w-[240px] flex-1 flex-wrap items-center gap-1 rounded-md border border-[var(--coollabs-line)] bg-[var(--coollabs-recessed)] px-2 py-1"
          onSubmit={(event) => {
            event.preventDefault();
            setQ(draft.trim());
          }}
        >
          {status && status !== "all" ? (
            <Token label={`is: ${status}`} onRemove={() => setStatus("all")} />
          ) : null}
          {q ? <Token label={q} onRemove={() => { setQ(""); setDraft(""); }} /> : null}
          <input
            className="min-w-[8rem] flex-1 bg-transparent px-1 text-[13px] outline-none"
            placeholder="Search title, culprit, or id"
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
          />
        </form>
        <select className="field w-36" value={sort} onChange={(event) => setSort(event.target.value)}>
          {sorts.map((item) => (
            <option key={item.id} value={item.id}>
              {item.label}
            </option>
          ))}
        </select>
        <div className="relative">
          <Button type="button" onClick={() => setSaveOpen((open) => !open)}>
            Save as
          </Button>
          {saveOpen ? (
            <div className="absolute right-0 z-20 mt-1 w-56 rounded-md border border-[var(--coollabs-hairline)] bg-[var(--color-panel)] p-2 shadow-[var(--shadow-dropdown,0_8px_24px_rgb(0_0_0/0.25))]">
              <input
                className="field w-full"
                placeholder="View name"
                value={saveName}
                onChange={(event) => setSaveName(event.target.value)}
              />
              <Button className="mt-2" variant="highlighted" type="button" onClick={saveView}>
                Save view
              </Button>
              {saved.length > 0 ? (
                <div className="mt-2 flex flex-col">
                  {saved.map((view) => (
                    <button key={view.name} className="rounded px-2 py-1 text-left text-[13px] hover:bg-[var(--nav-hover)]" type="button" onClick={() => applySaved(view)}>
                      {view.name}
                    </button>
                  ))}
                </div>
              ) : null}
            </div>
          ) : null}
        </div>
      </div>
      {selected.length > 0 ? (
        <div className="flex items-center gap-2 border-b border-[var(--coollabs-hairline)] px-4 py-2 text-[13px]">
          <span>{selected.length} selected</span>
          <Button variant="highlighted" size="sm" onClick={() => bulk.mutate("resolved")}>
            Resolve
          </Button>
          <Button size="sm" onClick={() => bulk.mutate("ignored")}>
            Archive
          </Button>
        </div>
      ) : null}
      <div className="min-w-0 flex-1 overflow-auto">
        {feed.isLoading ? <p className="p-4 text-[13px] text-[var(--color-fg-dim)]">Loading issues...</p> : null}
        {feed.isError ? <p className="p-4 text-[13px] text-error">{(feed.error as Error).message}</p> : null}
        {feed.data && issues.length === 0 ? (
          <div className="p-10 text-center">
            <h2 className="text-[15px] font-medium">No issues in this view</h2>
            <p className="mx-auto mt-1 max-w-md text-[13px] text-[var(--color-fg-dim)]">
              Point a Sentry SDK at a project DSN and throw an error. Events in the selected range show up here.
            </p>
          </div>
        ) : null}
        {issues.length > 0 ? (
          <table className="data-table min-w-[960px]">
            <thead>
              <tr>
                <th className="w-8">
                  <input
                    type="checkbox"
                    checked={allChecked}
                    onChange={() => setSelected(allChecked ? [] : issues.map((issue) => issue.id))}
                    aria-label="Select all issues"
                  />
                </th>
                <th>Issue</th>
                <th>Last seen</th>
                <th>Age</th>
                <th>
                  <span className="inline-flex items-center gap-2">
                    Trend
                    <span className="inline-flex overflow-hidden rounded border border-[var(--coollabs-line)]">
                      {(["24h", "14d"] as const).map((item) => (
                        <button
                          key={item}
                          type="button"
                          className={cn("px-1.5 py-0.5 text-[11px]", trend === item && "bg-[var(--nav-active)]")}
                          onClick={() => setTrend(item)}
                        >
                          {item}
                        </button>
                      ))}
                    </span>
                  </span>
                </th>
                <th>Events</th>
                <th>Users</th>
                <th>Priority</th>
              </tr>
            </thead>
            <tbody>
              {issues.map((issue) => (
                <IssueRowView
                  key={issue.id}
                  issue={issue}
                  trend={trend}
                  checked={selected.includes(issue.id)}
                  onCheck={(checked) =>
                    setSelected((current) => (checked ? [...current, issue.id] : current.filter((id) => id !== issue.id)))
                  }
                  onOpen={() => navigate(`/errors/${issue.shortId || issue.id}`)}
                />
              ))}
            </tbody>
          </table>
        ) : null}
      </div>
    </div>
  );
}

function IssueRowView({
  issue,
  trend,
  checked,
  onCheck,
  onOpen,
}: {
  issue: Issue;
  trend: "24h" | "14d";
  checked: boolean;
  onCheck: (checked: boolean) => void;
  onOpen: () => void;
}) {
  const parts = splitTitle(issue.title);
  return (
    <tr onClick={onOpen}>
      <td onClick={(event) => event.stopPropagation()}>
        <input type="checkbox" checked={checked} aria-label={`Select ${issue.shortId}`} onChange={(event) => onCheck(event.target.checked)} />
      </td>
      <td>
        <div className="font-medium">{parts.type}</div>
        {parts.message ? <div className="text-[13px] text-[var(--color-fg-dim)]">{parts.message}</div> : null}
        <div className="mt-1 flex flex-wrap items-center gap-2 text-[12px] text-[var(--color-fg-faint)]">
          <span className="font-medium text-[var(--color-fg)]">{issue.shortId}</span>
          {issue.unhandled ? <Badge tone="fatal">Unhandled</Badge> : null}
          <span>{issue.culprit || "No culprit"}</span>
        </div>
      </td>
      <td className="whitespace-nowrap" title={fullTime(issue.lastSeen)}>
        {formatTime(issue.lastSeen)}
      </td>
      <td className="whitespace-nowrap" title={fullTime(issue.firstSeen)}>
        {formatTime(issue.firstSeen)}
      </td>
      <td>{trend === "24h" ? <Sparkline values={issue.trend24h} /> : <Sparkline values={issue.trend14d} />}</td>
      <td>{issue.periodEventCount}</td>
      <td>{issue.userCount}</td>
      <td>
        <PriorityMark priority={issue.priority} />
      </td>
    </tr>
  );
}

function Token({ label, onRemove }: { label: string; onRemove: () => void }) {
  return (
    <span className="inline-flex items-center gap-1 rounded bg-[var(--nav-active)] px-1.5 py-0.5 text-[12px]">
      {label}
      <button type="button" className="text-[var(--color-fg-faint)]" onClick={onRemove} aria-label={`Remove ${label}`}>
        ×
      </button>
    </span>
  );
}

export function splitTitle(title: string) {
  const index = title.indexOf(": ");
  if (index <= 0) return { type: title, message: "" };
  return { type: title.slice(0, index), message: title.slice(index + 2) };
}
