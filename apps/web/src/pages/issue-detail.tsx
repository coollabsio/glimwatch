import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDown } from "reicon-react";
import { useMemo, useState, type ReactNode } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { Histogram } from "@/components/error-charts";
import { StackFrames } from "@/components/event-view";
import { Badge } from "@/components/issue-badge";
import { Button, Input, LayerCard, PageHeader, Select } from "@/components/ui";
import { api, type ExceptionInfo, type IssueDetail, type NormalizedEvent } from "@/lib/api";
import { cn, fullTime } from "@/lib/utils";

const ranges = ["24h", "7d", "14d", "30d"] as const;

export function IssueDetailPage() {
  const { id = "", eventId } = useParams();
  const navigate = useNavigate();
  const client = useQueryClient();
  const [range, setRange] = useState<(typeof ranges)[number]>("14d");
  const [environment, setEnvironment] = useState("");
  const [q, setQ] = useState("");
  const [inAppOnly, setInAppOnly] = useState(true);
  const [open, setOpen] = useState({ highlights: true, stack: true, breadcrumbs: true, tags: true, context: true, raw: false });
  const params = new URLSearchParams({ range, environment, q });
  const issue = useQuery({
    queryKey: ["issue", id, range, environment, q],
    queryFn: () => api<IssueDetail>(`/api/error-issues/${encodeURIComponent(id)}?${params}`),
  });
  const selectedId = eventId || issue.data?.latest?.eventId;
  const selected = useQuery({
    queryKey: ["event", selectedId],
    queryFn: () => api<NormalizedEvent>(`/api/error-events/${selectedId}`),
    enabled: Boolean(selectedId) && selectedId !== issue.data?.latest?.eventId,
  });
  const update = useMutation({
    mutationFn: (status: string) =>
      api(`/api/error-issues/${issue.data?.id ?? id}`, { method: "PATCH", body: JSON.stringify({ status }) }),
    onSuccess: () => client.invalidateQueries({ queryKey: ["issue", id] }),
  });

  const event = selected.data ?? (selectedId === issue.data?.latest?.eventId ? issue.data?.latest : undefined) ?? issue.data?.latest ?? null;
  const events = issue.data?.events ?? [];
  const index = events.findIndex((item) => item.id === event?.eventId);
  const parts = splitTitle(issue.data?.title ?? "");

  const environments = useMemo(() => {
    const values = new Set<string>();
    for (const item of events) if (item.environment) values.add(item.environment);
    if (issue.data?.environment) values.add(issue.data.environment);
    return [...values];
  }, [events, issue.data?.environment]);

  if (issue.isLoading) return <p className="text-[13px] text-[var(--ui-fg-faint)]">Loading issue...</p>;
  if (issue.isError || !issue.data) {
    return <p className="text-[13px] text-[var(--ui-error)]">{(issue.error as Error)?.message || "Issue not found"}</p>;
  }
  const data = issue.data;

  function openEvent(nextId: string) {
    navigate(`/errors/${data.shortId || data.id}/events/${nextId}`);
  }

  const toggle = (key: keyof typeof open) => () => setOpen((state) => ({ ...state, [key]: !state[key] }));

  return (
    <div className="flex flex-col gap-6">
      <div>
        <nav className="mb-2 flex items-center gap-2 text-[13px] text-[var(--ui-fg-faint)]">
          <Link to="/errors">
            Issues
          </Link>
          <span>/</span>
          <span className="font-mono text-[var(--ui-fg-dim)]">{data.shortId}</span>
        </nav>
        <PageHeader
          title={parts.type}
          description={parts.message || undefined}
          actions={
            <>
              {data.status !== "resolved" ? (
                <Button variant="highlighted" onClick={() => update.mutate("resolved")}>
                  Resolve
                </Button>
              ) : (
                <Button onClick={() => update.mutate("unresolved")}>Reopen</Button>
              )}
              {data.status !== "ignored" ? (
                <Button onClick={() => update.mutate("ignored")}>Archive</Button>
              ) : (
                <Button onClick={() => update.mutate("unresolved")}>Unarchive</Button>
              )}
            </>
          }
        />
        <div className="-mt-3 flex flex-wrap items-center gap-2">
          <Badge tone={data.level}>{data.level}</Badge>
          <Badge tone={data.status}>{data.status}</Badge>
          {data.unhandled ? <Badge tone="fatal">Unhandled</Badge> : null}
        </div>
      </div>

      <div className="grid min-w-0 gap-6 xl:grid-cols-[minmax(0,1fr)_300px]">
        <div className="flex min-w-0 flex-col gap-6">
          <LayerCard
            title="Events"
            description={`In this range: ${data.periodEvents} events, ${data.users} users`}
            actions={
              <>
                <div className="w-44">
                  <Input placeholder="Filter events" value={q} onChange={(event) => setQ(event.target.value)} />
                </div>
                <div className="w-40">
                  <Select
                    aria-label="Environment"
                    value={environment}
                    onChange={setEnvironment}
                    options={[{ value: "", label: "All environments" }, ...environments.map((item) => ({ value: item, label: item }))]}
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
              </>
            }
          >
            <Histogram values={data.histogram} />
          </LayerCard>

          <LayerCard
            title={event ? `Event ${event.eventId.slice(0, 8)}` : "Event"}
            description={event ? fullTime(event.timestamp) : undefined}
            actions={
              <>
                <Button
                  size="sm"
                  disabled={index < 0 || index >= events.length - 1}
                  onClick={() => events[index + 1] && openEvent(events[index + 1].id)}
                >
                  Older
                </Button>
                <Button size="sm" disabled={index <= 0} onClick={() => events[index - 1] && openEvent(events[index - 1].id)}>
                  Newer
                </Button>
                <Button size="sm" disabled={events.length === 0} onClick={() => openEvent(events[events.length - 1].id)}>
                  First
                </Button>
                <Button size="sm" disabled={events.length === 0} onClick={() => openEvent(events[0].id)}>
                  Latest
                </Button>
                <Button size="sm" disabled={!event} onClick={() => setOpen((state) => ({ ...state, raw: true }))}>
                  JSON
                </Button>
              </>
            }
          >
            {event ? (
              <div className="flex flex-col gap-3">
                <EventHeader event={event} />
                <nav className="flex flex-wrap gap-4 text-[12px] font-medium text-[var(--ui-fg-faint)]">
                  <a href="#highlights">Highlights</a>
                  <a href="#stack">Stack trace</a>
                  <a href="#breadcrumbs">Breadcrumbs</a>
                  <a href="#tags">Tags</a>
                  <a href="#context">Context</a>
                </nav>
              </div>
            ) : (
              <p className="py-6 text-center text-[13px] text-[var(--ui-fg-faint)]">No events match this filter.</p>
            )}
          </LayerCard>

          {event ? (
            <>
              <Section id="highlights" title="Highlights" open={open.highlights} onToggle={toggle("highlights")}>
                <Highlights event={event} />
              </Section>
              <Section
                id="stack"
                title="Stack trace"
                open={open.stack}
                onToggle={toggle("stack")}
                flush
                actions={
                  <>
                    <label className="flex items-center gap-1.5 text-[12px] text-[var(--ui-fg-faint)]">
                      <input type="checkbox" checked={inAppOnly} onChange={(event) => setInAppOnly(event.target.checked)} />
                      In-app only
                    </label>
                    <Button size="sm" onClick={() => navigator.clipboard.writeText(stackText(event.exceptions))}>
                      Copy as text
                    </Button>
                  </>
                }
              >
                {event.exceptions.length === 0 ? (
                  <p className="p-4 text-[13px] text-[var(--ui-fg-faint)]">{event.message || "No exception payload."}</p>
                ) : (
                  event.exceptions.map((exception, exceptionIndex) => (
                    <div key={`${exception.type}-${exceptionIndex}`} className={exceptionIndex > 0 ? "border-t border-[var(--ui-divider)]" : undefined}>
                      <div className="flex flex-wrap items-center gap-2 px-4 py-3 text-[13px]">
                        <span className="font-medium text-[var(--ui-fg)]">{exception.type || "Error"}</span>
                        {exception.value ? <span className="text-[var(--ui-fg-dim)]">{exception.value}</span> : null}
                        {exception.handled === false ? <Badge tone="fatal">Unhandled</Badge> : null}
                        {exception.handled === true ? <Badge tone="info">Handled</Badge> : null}
                      </div>
                      <StackFrames frames={exception.frames} inAppOnly={inAppOnly && exception.frames.some((frame) => frame.inApp)} />
                    </div>
                  ))
                )}
              </Section>
              <Section id="breadcrumbs" title="Breadcrumbs" open={open.breadcrumbs} onToggle={toggle("breadcrumbs")}>
                {event.breadcrumbs.length === 0 ? (
                  <p className="text-center text-[13px] text-[var(--ui-fg-faint)]">No breadcrumbs on this event.</p>
                ) : (
                  <ol className="flex flex-col gap-3">
                    {event.breadcrumbs.map((crumb, crumbIndex) => (
                      <li key={crumbIndex} className="border-l-2 border-[var(--ui-divider)] pl-3 text-[13px] text-[var(--ui-fg)]">
                        <div className="text-[12px] text-[var(--ui-fg-faint)]">
                          {crumb.category || crumb.type || "default"}
                          {crumb.timestamp ? ` · ${fullTime(crumb.timestamp)}` : ""}
                        </div>
                        <div>{crumb.message}</div>
                      </li>
                    ))}
                  </ol>
                )}
              </Section>
              <Section id="tags" title="Tags" open={open.tags} onToggle={toggle("tags")}>
                {event.tags.length === 0 ? (
                  <p className="text-center text-[13px] text-[var(--ui-fg-faint)]">No tags.</p>
                ) : (
                  <dl className="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-1.5 text-[13px]">
                    {event.tags.map((tag) => (
                      <div key={tag.key} className="contents">
                        <dt className="text-[var(--ui-fg-faint)]">{tag.key}</dt>
                        <dd className="break-all text-[var(--ui-fg)]">{tag.value}</dd>
                      </div>
                    ))}
                  </dl>
                )}
              </Section>
              <Section id="context" title="Context" open={open.context} onToggle={toggle("context")}>
                <ContextBlock event={event} />
              </Section>
              <Section id="raw" title="Raw JSON" open={open.raw} onToggle={toggle("raw")}>
                <pre className="max-h-96 overflow-auto rounded-lg border border-[var(--ui-control-border)] bg-[var(--ui-control-bg)] p-3 font-mono text-[12px] leading-5 text-[var(--ui-fg-dim)]">{JSON.stringify(event.raw, null, 2)}</pre>
              </Section>
            </>
          ) : null}
        </div>

        <aside className="flex flex-col gap-6 xl:sticky xl:top-[calc(3rem+1.75rem)] xl:self-start">
          <LayerCard title="Details">
            <dl className="flex flex-col gap-3 text-[13px]">
              <Detail label="Last seen">{fullTime(data.lastSeen)}</Detail>
              <Detail label="First seen">{fullTime(data.firstSeen)}</Detail>
              <Detail label="Total events">{data.eventCount}</Detail>
              <Detail label="Environment">{data.environment || "-"}</Detail>
              <Detail label="Release">{data.release || "-"}</Detail>
              <Detail label="Culprit">{data.culprit || "-"}</Detail>
              <Detail label="Project">
                <span className="text-[var(--ui-accent-light)]">
                  <Link to={`/error-projects/${data.projectId}`}>
                    {data.projectName}
                  </Link>
                </span>
              </Detail>
            </dl>
          </LayerCard>
          <LayerCard title="Tag distribution">
            {data.tags.length === 0 ? (
              <p className="text-center text-[13px] text-[var(--ui-fg-faint)]">No tags in this range.</p>
            ) : (
              <ul className="flex flex-col gap-3">
                {data.tags.map((tag) => (
                  <li key={`${tag.key}:${tag.value}`}>
                    <div className="flex justify-between gap-2 text-[12px] text-[var(--ui-fg-dim)]">
                      <span className="truncate">
                        {tag.key}: {tag.value}
                      </span>
                      <span className="tabular-nums">{tag.count}</span>
                    </div>
                    <div className="mt-1 h-1.5 rounded-full bg-[var(--ui-hover)]">
                      <div
                        className="h-1.5 rounded-full bg-[var(--ui-accent)]"
                        style={{ width: `${Math.max(8, (tag.count / Math.max(data.periodEvents, 1)) * 100)}%` }}
                      />
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </LayerCard>
        </aside>
      </div>
    </div>
  );
}

function Detail({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div>
      <dt className="text-[12px] text-[var(--ui-fg-faint)]">{label}</dt>
      <dd className="mt-0.5 break-all text-[var(--ui-fg)]">{children}</dd>
    </div>
  );
}

function EventHeader({ event }: { event: NormalizedEvent }) {
  const runtime = record(event.contexts.runtime);
  const os = record(event.contexts.os);
  const user = stringField(event.user, "email") || stringField(event.user, "username") || stringField(event.user, "id");
  const chips = [
    user,
    runtime.name ? `${runtime.name} ${runtime.version ?? ""}`.trim() : "",
    os.name ? `${os.name} ${os.version ?? ""}`.trim() : "",
    event.release || "",
    event.environment || "",
  ].filter(Boolean);
  return (
    <div className="flex flex-wrap gap-2">
      {chips.map((chip) => (
        <span key={chip} className="inline-flex h-6 items-center rounded-full border border-[var(--ui-control-border)] bg-[var(--ui-control-bg)] px-2 text-[12px] text-[var(--ui-fg-dim)]">
          {chip}
        </span>
      ))}
    </div>
  );
}

function Highlights({ event }: { event: NormalizedEvent }) {
  const rows: [string, string][] = [
    ["Handled", event.exceptions.some((item) => item.handled === false) ? "no" : "yes"],
    ["Level", event.level],
    ["Transaction", event.transaction || ""],
    ["URL", stringField(event.request, "url")],
    ["Environment", event.environment || ""],
    ["Release", event.release || ""],
  ].filter((entry): entry is [string, string] => Boolean(entry[1]));
  return (
    <dl className="grid gap-3 sm:grid-cols-2">
      {rows.map(([key, value]) => (
        <div key={key}>
          <dt className="text-[12px] text-[var(--ui-fg-faint)]">{key}</dt>
          <dd className="mt-0.5 break-all text-[13px] text-[var(--ui-fg)]">{value}</dd>
        </div>
      ))}
    </dl>
  );
}

function ContextBlock({ event }: { event: NormalizedEvent }) {
  const names = Object.keys(event.contexts).filter((name) => name !== "trace");
  if (names.length === 0) return <p className="text-center text-[13px] text-[var(--ui-fg-faint)]">No context.</p>;
  return (
    <div className="flex flex-col gap-4">
      {names.map((name) => (
        <div key={name}>
          <div className="mb-1.5 text-[12px] font-medium text-[var(--ui-fg-dim)] first-letter:uppercase">{name}</div>
          <dl className="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-1 text-[13px]">
            {Object.entries(record(event.contexts[name])).map(([key, value]) => (
              <div key={key} className="contents">
                <dt className="text-[var(--ui-fg-faint)]">{key}</dt>
                <dd className="break-all text-[var(--ui-fg)]">{String(value)}</dd>
              </div>
            ))}
          </dl>
        </div>
      ))}
    </div>
  );
}

/* Collapsible layer card: same anatomy as LayerCard, the well hides when closed. */
function Section({
  id,
  title,
  open,
  onToggle,
  actions,
  flush,
  children,
}: {
  id: string;
  title: string;
  open: boolean;
  onToggle: () => void;
  actions?: ReactNode;
  flush?: boolean;
  children: ReactNode;
}) {
  return (
    <section id={id} className="layer-card scroll-mt-20">
      <header className={open ? undefined : "!pb-1"}>
        <button type="button" className="flex items-center gap-1.5 py-0.5 text-left" onClick={onToggle} aria-expanded={open}>
          <ChevronDown className={cn("size-3.5 text-[var(--ui-fg-faint)] transition-transform", !open && "-rotate-90")} aria-hidden="true" />
          <h2>{title}</h2>
        </button>
        {open && actions ? <div className="flex flex-wrap items-center gap-2">{actions}</div> : null}
      </header>
      {open ? <div className={cn("layer-card-body", flush && "is-flush")}>{children}</div> : null}
    </section>
  );
}

function splitTitle(title: string) {
  const index = title.indexOf(": ");
  if (index <= 0) return { type: title, message: "" };
  return { type: title.slice(0, index), message: title.slice(index + 2) };
}

function stackText(exceptions: ExceptionInfo[]) {
  return exceptions
    .map((exception) => {
      const head = `${exception.type || "Error"}: ${exception.value || ""}`;
      const frames = [...exception.frames].reverse().map((frame) => {
        const where = frame.filename || frame.absPath || frame.module || "unknown";
        return `    at ${frame.function || "<anonymous>"} (${where}:${frame.lineno ?? "?"})`;
      });
      return [head, ...frames].join("\n");
    })
    .join("\n\n");
}

function record(value: unknown): Record<string, string> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return {};
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .filter(([, item]) => item != null && typeof item !== "object")
      .map(([key, item]) => [key, String(item)]),
  );
}

function stringField(value: Record<string, unknown> | undefined, key: string) {
  const item = value?.[key];
  return typeof item === "string" ? item : "";
}
