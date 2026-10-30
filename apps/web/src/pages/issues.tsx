import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router";
import { PriorityMark, Sparkline } from "@/components/error-charts";
import { Badge } from "@/components/issue-badge";
import { Button, Input, LayerCard, PageHeader, Select } from "@/components/ui";
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
    const raw = localStorage.getItem("owlee-error-views");
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
    localStorage.setItem("owlee-error-views", JSON.stringify(next));
    setSaveName("");
    setSaveOpen(false);
  }

  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Issues" description="Errors from your Sentry SDKs, grouped into issues. The list refreshes every few seconds." />
      <LayerCard
        title="Issue feed"
        flush
        actions={
          <>
            {selected.length > 0 ? (
              <>
                <span className="text-[12px] text-[var(--ui-fg-faint)]">{selected.length} selected</span>
                <Button variant="highlighted" size="sm" onClick={() => bulk.mutate("resolved")}>
                  Resolve
                </Button>
                <Button size="sm" onClick={() => bulk.mutate("ignored")}>
                  Archive
                </Button>
              </>
            ) : null}
            <div className="relative">
              <Button type="button" size="sm" onClick={() => setSaveOpen((open) => !open)}>
                Save as
              </Button>
              {saveOpen ? (
                <div className="absolute right-0 z-20 mt-1 w-56 rounded-[10px] border border-[var(--ui-menu-border)] bg-[var(--ui-menu-bg)] p-2 shadow-[var(--ui-menu-shadow)]">
                  <Input placeholder="View name" value={saveName} onChange={(event) => setSaveName(event.target.value)} />
                  <Button className="mt-2 w-full" variant="highlighted" type="button" onClick={saveView}>
                    Save view
                  </Button>
                  {saved.length > 0 ? (
                    <div className="mt-2 flex flex-col gap-px border-t border-[var(--ui-divider)] pt-2">
                      {saved.map((view) => (
                        <button
                          key={view.name}
                          className="rounded-md px-2 py-1.5 text-left text-[13px] text-[var(--ui-fg)] hover:bg-[var(--ui-hover)]"
                          type="button"
                          onClick={() => applySaved(view)}
                        >
                          {view.name}
                        </button>
                      ))}
                    </div>
                  ) : null}
                </div>
              ) : null}
            </div>
          </>
        }
      >
        <div className="flex flex-wrap items-center gap-2 border-b border-[var(--ui-divider)] p-3">
          <form
            className="flex h-8 min-w-[240px] flex-1 items-center gap-1 overflow-hidden rounded-lg border border-[var(--ui-control-border)] bg-[var(--ui-control-bg)] px-1.5 focus-within:border-[var(--ui-accent)]"
            onSubmit={(event) => {
              event.preventDefault();
              setQ(draft.trim());
            }}
          >
            {status && status !== "all" ? <Token label={`is: ${status}`} onRemove={() => setStatus("all")} /> : null}
            {q ? <Token label={q} onRemove={() => { setQ(""); setDraft(""); }} /> : null}
            <input
              className="h-full min-w-[8rem] flex-1 bg-transparent px-1 text-[14px] text-[var(--ui-fg)] outline-none placeholder:text-[var(--ui-fg-faint)]"
              placeholder="Search title, culprit, or id"
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
            />
          </form>
          <div className="w-40">
            <Select
              aria-label="Project"
              value={projectId}
              onChange={setProjectId}
              options={[{ value: "", label: "All projects" }, ...(projects.data ?? []).map((project) => ({ value: project.id, label: project.name }))]}
            />
          </div>
          <div className="w-44">
            <Select
              aria-label="Environment"
              value={environment}
              onChange={setEnvironment}
              options={[{ value: "", label: "All environments" }, ...(feed.data?.environments ?? []).map((item) => ({ value: item, label: item }))]}
            />
          </div>
          <div className="w-28">
            <Select
              aria-label="Time range"
              value={range}
              onChange={setRange}
              options={ranges.map((item) => ({ value: item, label: `Last ${item}` }))}
            />
          </div>
          <div className="w-32">
            <Select
              aria-label="Sort"
              value={sort}
              onChange={setSort}
              options={sorts.map((item) => ({ value: item.id, label: item.label }))}
            />
          </div>
        </div>
        {feed.isLoading ? <p className="p-4 text-[13px] text-[var(--ui-fg-faint)]">Loading issues...</p> : null}
        {feed.isError ? <p className="p-4 text-[13px] text-[var(--ui-error)]">{(feed.error as Error).message}</p> : null}
        {feed.data && issues.length === 0 ? (
          <div className="flex min-h-40 flex-col items-center justify-center px-4 py-10 text-center">
            <h2 className="text-[14px] font-medium text-[var(--ui-fg)]">No issues in this view</h2>
            <p className="mx-auto mt-1 max-w-[28rem] text-[13px] text-[var(--ui-fg-faint)]">
              Point a Sentry SDK at a project DSN and throw an error. Events in the selected range show up here.
            </p>
          </div>
        ) : null}
        {issues.length > 0 ? (
          <div className="overflow-x-auto">
            <table className="cool-table min-w-[960px]">
              <thead>
                <tr>
                  <th className="w-10 !pr-0">
                    <input
                      type="checkbox"
                      className="align-middle"
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
                      <span className="inline-flex gap-px rounded-md border border-[var(--ui-control-border)] bg-[var(--ui-control-bg)] p-px">
                        {(["24h", "14d"] as const).map((item) => (
                          <button
                            key={item}
                            type="button"
                            className={cn(
                              "rounded-[5px] px-1.5 py-0.5 text-[11px] text-[var(--ui-fg-faint)] hover:text-[var(--ui-fg)]",
                              trend === item && "bg-[var(--ui-hover)] text-[var(--ui-fg)]",
                            )}
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
          </div>
        ) : null}
      </LayerCard>
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
    <tr className="clickable-row" onClick={onOpen}>
      <td className="!pr-0" onClick={(event) => event.stopPropagation()}>
        <input type="checkbox" className="align-middle" checked={checked} aria-label={`Select ${issue.shortId}`} onChange={(event) => onCheck(event.target.checked)} />
      </td>
      <td>
        <div className="font-medium text-[var(--ui-fg)]">{parts.type}</div>
        {parts.message ? <div className="text-[13px] text-[var(--ui-fg-dim)]">{parts.message}</div> : null}
        <div className="mt-1 flex flex-wrap items-center gap-2 text-[12px] text-[var(--ui-fg-faint)]">
          <span className="font-mono text-[var(--ui-fg-dim)]">{issue.shortId}</span>
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
      <td className="tabular-nums">{issue.periodEventCount}</td>
      <td className="tabular-nums">{issue.userCount}</td>
      <td>
        <PriorityMark priority={issue.priority} />
      </td>
    </tr>
  );
}

function Token({ label, onRemove }: { label: string; onRemove: () => void }) {
  return (
    <span className="inline-flex h-6 shrink-0 items-center gap-1 rounded-md border border-[var(--ui-control-border)] bg-[var(--ui-hover)] px-1.5 text-[12px] text-[var(--ui-fg)]">
      {label}
      <button type="button" className="text-[var(--ui-fg-faint)] hover:text-[var(--ui-fg)]" onClick={onRemove} aria-label={`Remove ${label}`}>
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
