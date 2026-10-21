import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState, type ReactNode } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { Histogram } from "@/components/error-charts";
import { StackFrames } from "@/components/event-view";
import { PageHeader } from "@/components/ui";
import { Badge } from "@/components/issue-badge";
import { Button } from "@/components/ui";
import { api, type ExceptionInfo, type IssueDetail, type NormalizedEvent } from "@/lib/api";
import { fullTime } from "@/lib/utils";

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

  if (issue.isLoading) return <p className="p-4 text-[13px] text-[var(--color-fg-dim)]">Loading issue...</p>;
  if (issue.isError || !issue.data) {
    return <p className="p-4 text-[13px] text-error">{(issue.error as Error)?.message || "Issue not found"}</p>;
  }
  const data = issue.data;

  function openEvent(nextId: string) {
    navigate(`/errors/${data.shortId || data.id}/events/${nextId}`);
  }

  return (
    <div className="flex min-h-full flex-col">
      <PageHeader
        title={
          <span className="flex items-center gap-2 text-[13px] font-normal">
            <Link to="/errors" className="text-[var(--color-fg-dim)]">
              Issues
            </Link>
            <span className="text-[var(--color-fg-faint)]">/</span>
            <span className="font-medium">{data.shortId}</span>
          </span>
        }
        actions={
          <div className="flex gap-2">
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
          </div>
        }
      />
      <div className="grid min-w-0 gap-4 px-4 py-4 xl:grid-cols-[minmax(0,1fr)_280px]">
        <div className="min-w-0">
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <Badge tone={data.level}>{data.level}</Badge>
            <Badge tone={data.status}>{data.status}</Badge>
            {data.unhandled ? <Badge tone="fatal">Unhandled</Badge> : null}
          </div>
          <h2 className="text-xl font-medium">{parts.type}</h2>
          {parts.message ? <p className="mt-1 text-[14px] text-[var(--color-fg-dim)]">{parts.message}</p> : null}
          <p className="mt-2 text-[13px] text-[var(--color-fg-faint)]">
            {data.environment || "No environment"}
            {data.culprit ? ` · ${data.culprit}` : ""}
            {data.release ? ` · ${data.release}` : ""}
          </p>

          <div className="mt-4 flex flex-wrap items-center gap-2">
            <select className="field w-40" value={environment} onChange={(event) => setEnvironment(event.target.value)}>
              <option value="">All envs</option>
              {environments.map((item) => (
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
            <input
              className="field w-full max-w-xs"
              placeholder="Filter events"
              value={q}
              onChange={(event) => setQ(event.target.value)}
            />
          </div>

          <div className="mt-4 grid gap-3 lg:grid-cols-[minmax(0,1fr)_260px]">
            <section className="card p-3">
              <div className="mb-2 flex items-baseline justify-between text-[13px]">
                <span>
                  Events <strong>{data.periodEvents}</strong>
                </span>
                <span>
                  Users <strong>{data.users}</strong>
                </span>
              </div>
              <Histogram values={data.histogram} />
            </section>
            <section className="card p-3">
              <h3 className="mb-2 text-[13px] font-medium">Tag distribution</h3>
              {data.tags.length === 0 ? (
                <p className="text-[13px] text-[var(--color-fg-faint)]">No tags in this range.</p>
              ) : (
                <ul className="flex flex-col gap-2">
                  {data.tags.map((tag) => (
                    <li key={`${tag.key}:${tag.value}`}>
                      <div className="flex justify-between gap-2 text-[12px]">
                        <span className="truncate">
                          {tag.key}: {tag.value}
                        </span>
                        <span>{tag.count}</span>
                      </div>
                      <div className="mt-1 h-1.5 rounded bg-[var(--coollabs-fill)]">
                        <div
                          className="h-1.5 rounded bg-coollabs dark:bg-warning"
                          style={{ width: `${Math.max(8, (tag.count / Math.max(data.periodEvents, 1)) * 100)}%` }}
                        />
                      </div>
                    </li>
                  ))}
                </ul>
              )}
            </section>
          </div>

          <div className="mt-4 border-b border-[var(--coollabs-hairline)] text-[13px]">
            <span className="inline-block border-b-2 border-coollabs px-2 py-2 font-medium dark:border-warning">
              Events {data.periodEvents}
            </span>
          </div>

          <div className="mt-3 flex flex-wrap items-center gap-2 text-[13px]">
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
            {event ? (
              <span className="text-[var(--color-fg-faint)]">
                {event.eventId.slice(0, 8)} · {fullTime(event.timestamp)}
              </span>
            ) : null}
            <button className="text-coollabs dark:text-warning" type="button" onClick={() => setOpen((state) => ({ ...state, raw: true }))}>
              JSON
            </button>
          </div>

          {event ? <EventHeader event={event} /> : null}

          <nav className="mt-3 flex flex-wrap gap-3 text-[12px] text-[var(--color-fg-dim)]">
            <a href="#highlights">Highlights</a>
            <a href="#stack">Stack trace</a>
            <a href="#breadcrumbs">Breadcrumbs</a>
            <a href="#tags">Tags</a>
            <a href="#context">Context</a>
          </nav>

          {event ? (
            <div className="mt-2">
              <Section id="highlights" title="Highlights" open={open.highlights} onToggle={() => setOpen((s) => ({ ...s, highlights: !s.highlights }))}>
                <Highlights event={event} />
              </Section>
              <Section
                id="stack"
                title="Stack trace"
                open={open.stack}
                onToggle={() => setOpen((s) => ({ ...s, stack: !s.stack }))}
                actions={
                  <span className="flex items-center gap-2 text-[12px] font-normal normal-case">
                    <label className="flex items-center gap-1">
                      <input type="checkbox" checked={inAppOnly} onChange={(event) => setInAppOnly(event.target.checked)} />
                      In-app
                    </label>
                    <button
                      type="button"
                      className="text-coollabs dark:text-warning"
                      onClick={() => navigator.clipboard.writeText(stackText(event.exceptions))}
                    >
                      Copy as text
                    </button>
                  </span>
                }
              >
                {event.exceptions.length === 0 ? (
                  <p className="text-[13px] text-[var(--color-fg-faint)]">{event.message || "No exception payload."}</p>
                ) : (
                  event.exceptions.map((exception, exceptionIndex) => (
                    <div key={`${exception.type}-${exceptionIndex}`} className="mb-3">
                      <div className="mb-1 text-[13px]">
                        <span className="font-medium">{exception.type || "Error"}</span>
                        {exception.value ? <span className="text-[var(--color-fg-dim)]">: {exception.value}</span> : null}
                        {exception.handled === false ? <Badge className="ml-2" tone="fatal">Unhandled</Badge> : null}
                        {exception.handled === true ? <Badge className="ml-2" tone="info">Handled</Badge> : null}
                      </div>
                      <StackFrames frames={exception.frames} inAppOnly={inAppOnly && exception.frames.some((frame) => frame.inApp)} />
                    </div>
                  ))
                )}
              </Section>
              <Section id="breadcrumbs" title="Breadcrumbs" open={open.breadcrumbs} onToggle={() => setOpen((s) => ({ ...s, breadcrumbs: !s.breadcrumbs }))}>
                {event.breadcrumbs.length === 0 ? (
                  <p className="text-[13px] text-[var(--color-fg-faint)]">No breadcrumbs on this event.</p>
                ) : (
                  <ol className="flex flex-col gap-2">
                    {event.breadcrumbs.map((crumb, crumbIndex) => (
                      <li key={crumbIndex} className="border-l-2 border-[var(--coollabs-hairline)] pl-3 text-[13px]">
                        <div className="text-[12px] text-[var(--color-fg-faint)]">
                          {crumb.category || crumb.type || "default"}
                          {crumb.timestamp ? ` · ${fullTime(crumb.timestamp)}` : ""}
                        </div>
                        <div>{crumb.message}</div>
                      </li>
                    ))}
                  </ol>
                )}
              </Section>
              <Section id="tags" title="Tags" open={open.tags} onToggle={() => setOpen((s) => ({ ...s, tags: !s.tags }))}>
                {event.tags.length === 0 ? (
                  <p className="text-[13px] text-[var(--color-fg-faint)]">No tags.</p>
                ) : (
                  <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-[13px]">
                    {event.tags.map((tag) => (
                      <div key={tag.key} className="contents">
                        <dt className="text-[var(--color-fg-faint)]">{tag.key}</dt>
                        <dd className="break-all">{tag.value}</dd>
                      </div>
                    ))}
                  </dl>
                )}
              </Section>
              <Section id="context" title="Context" open={open.context} onToggle={() => setOpen((s) => ({ ...s, context: !s.context }))}>
                <ContextBlock event={event} />
              </Section>
              <Section id="raw" title="Raw JSON" open={open.raw} onToggle={() => setOpen((s) => ({ ...s, raw: !s.raw }))}>
                <pre className="max-h-96 overflow-auto rounded-md bg-black/20 p-3 text-[12px] leading-5">{JSON.stringify(event.raw, null, 2)}</pre>
              </Section>
            </div>
          ) : (
            <p className="mt-4 text-[13px]">No events match this filter.</p>
          )}
        </div>
        <aside className="flex flex-col gap-3 xl:sticky xl:top-4 xl:self-start">
          <section className="card p-3 text-[13px]">
            <div className="text-[11px] uppercase tracking-wide text-[var(--coollabs-subtle)]">Last seen</div>
            <div className="mt-1 font-medium">{fullTime(data.lastSeen)}</div>
            <div className="mt-1 text-[var(--color-fg-faint)]">{data.release || "No release"}</div>
          </section>
          <section className="card p-3 text-[13px]">
            <div className="text-[11px] uppercase tracking-wide text-[var(--coollabs-subtle)]">First seen</div>
            <div className="mt-1 font-medium">{fullTime(data.firstSeen)}</div>
            <div className="mt-1 text-[var(--color-fg-faint)]">{data.eventCount} events total</div>
          </section>
          <section className="card p-3 text-[13px]">
            <div className="text-[11px] uppercase tracking-wide text-[var(--coollabs-subtle)]">Project</div>
            <Link className="mt-1 block text-coollabs dark:text-warning" to={`/error-projects/${data.projectId}`}>
              {data.projectName}
            </Link>
          </section>
        </aside>
      </div>
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
    <div className="mt-3 flex flex-wrap gap-2">
      {chips.map((chip) => (
        <span key={chip} className="rounded bg-[var(--nav-active)] px-2 py-1 text-[12px]">
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
    <dl className="grid gap-2 sm:grid-cols-2">
      {rows.map(([key, value]) => (
        <div key={key}>
          <dt className="text-[12px] text-[var(--color-fg-faint)]">{key}</dt>
          <dd className="break-all text-[13px]">{value}</dd>
        </div>
      ))}
    </dl>
  );
}

function ContextBlock({ event }: { event: NormalizedEvent }) {
  const names = Object.keys(event.contexts).filter((name) => name !== "trace");
  if (names.length === 0) return <p className="text-[13px] text-[var(--color-fg-faint)]">No context.</p>;
  return (
    <div className="flex flex-col gap-3">
      {names.map((name) => (
        <div key={name}>
          <div className="mb-1 text-[12px] capitalize text-[var(--coollabs-subtle)]">{name}</div>
          <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-0.5 text-[13px]">
            {Object.entries(record(event.contexts[name])).map(([key, value]) => (
              <div key={key} className="contents">
                <dt className="text-[var(--color-fg-faint)]">{key}</dt>
                <dd className="break-all">{String(value)}</dd>
              </div>
            ))}
          </dl>
        </div>
      ))}
    </div>
  );
}

function Section({
  id,
  title,
  open,
  onToggle,
  actions,
  children,
}: {
  id: string;
  title: string;
  open: boolean;
  onToggle: () => void;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section id={id} className="scroll-mt-4 border-b border-[var(--coollabs-hairline)] py-3">
      <div className="flex items-center gap-2">
        <button type="button" className="text-[14px] font-medium" onClick={onToggle}>
          {open ? "▾" : "▸"} {title}
        </button>
        <div className="ml-auto">{actions}</div>
      </div>
      {open ? <div className="mt-3">{children}</div> : null}
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
