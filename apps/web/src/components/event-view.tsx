import { useState, type ReactNode } from "react";
import type { Frame, NormalizedEvent } from "@/lib/api";
import { cn, fullTime } from "@/lib/utils";
import { Badge } from "@/components/issue-badge";
import { Button, LayerCard } from "@/components/ui";

export function EventView({ event }: { event: NormalizedEvent }) {
  const [inAppOnly, setInAppOnly] = useState(true);
  const [showRaw, setShowRaw] = useState(false);
  const hasInApp = event.exceptions.some((exception) => exception.frames.some((frame) => frame.inApp));

  return (
    <div className="flex flex-col gap-6">
      {event.exceptions.length === 0 ? (
        <LayerCard title="Message">
          <h2 className="text-[14px] font-medium text-[var(--ui-fg)]">{event.title}</h2>
          {event.message ? (
            <p className="mt-2 whitespace-pre-wrap text-[13px] text-[var(--ui-fg-dim)]">{event.message}</p>
          ) : null}
        </LayerCard>
      ) : (
        event.exceptions.map((exception, index) => (
          <LayerCard
            key={`${exception.type}-${index}`}
            title={event.exceptions.length > 1 ? `Exception ${index + 1}` : "Exception"}
            description={`${exception.frames.length} frames`}
            flush
            actions={
              <>
                {exception.handled === false ? <Badge tone="fatal">Unhandled</Badge> : null}
                {exception.handled === true ? <Badge tone="info">Handled</Badge> : null}
                {exception.mechanism ? <Badge>{exception.mechanism}</Badge> : null}
                {hasInApp ? (
                  <label className="flex items-center gap-1.5 text-[12px] text-[var(--ui-fg-faint)]">
                    <input type="checkbox" checked={inAppOnly} onChange={(event) => setInAppOnly(event.target.checked)} />
                    In-app frames only
                  </label>
                ) : null}
              </>
            }
          >
            <div className="px-4 py-3 text-[14px] font-medium text-[var(--ui-fg)]">
              {exception.type || "Error"}
              {exception.value ? <span className="font-normal text-[var(--ui-fg-dim)]">: {exception.value}</span> : null}
            </div>
            <StackFrames frames={exception.frames} inAppOnly={inAppOnly && hasInApp} />
          </LayerCard>
        ))
      )}

      <div className="grid gap-6 xl:grid-cols-2">
        <LayerCard title="Breadcrumbs">
          {event.breadcrumbs.length === 0 ? (
            <Muted>No breadcrumbs were sent with this event.</Muted>
          ) : (
            <ol className="flex flex-col gap-3">
              {event.breadcrumbs.map((crumb, index) => (
                <li key={index} className="border-l-2 border-[var(--ui-divider)] pl-3">
                  <div className="flex flex-wrap items-center gap-2 text-[12px] text-[var(--ui-fg-faint)]">
                    <span>{crumb.category || crumb.type || "default"}</span>
                    {crumb.level ? <Badge tone={crumb.level}>{crumb.level}</Badge> : null}
                    {crumb.timestamp ? <span title={fullTime(crumb.timestamp)}>{fullTime(crumb.timestamp)}</span> : null}
                  </div>
                  {crumb.message ? <div className="text-[13px] text-[var(--ui-fg)]">{crumb.message}</div> : null}
                  {Object.keys(crumb.data || {}).length > 0 ? (
                    <pre className="mt-1 overflow-auto rounded-lg border border-[var(--ui-control-border)] bg-[var(--ui-control-bg)] p-2 font-mono text-[12px] text-[var(--ui-fg-dim)]">
                      {JSON.stringify(crumb.data, null, 2)}
                    </pre>
                  ) : null}
                </li>
              ))}
            </ol>
          )}
        </LayerCard>

        <div className="flex flex-col gap-6">
          <KeyValueCard title="Tags" entries={event.tags.map((tag) => [tag.key, tag.value])} />
          <ContextCard contexts={event.contexts} />
          <KeyValueCard title="User" entries={entries(event.user)} />
          <RequestCard request={event.request} />
          <KeyValueCard title="SDK and release" entries={sdkEntries(event)} />
          <KeyValueCard title="Extra" entries={entries(event.extra)} />
        </div>
      </div>

      <LayerCard
        title="Raw event JSON"
        actions={
          <Button size="sm" onClick={() => setShowRaw((value) => !value)}>
            {showRaw ? "Hide" : "Show"}
          </Button>
        }
      >
        {showRaw ? (
          <pre className="max-h-[480px] overflow-auto rounded-lg border border-[var(--ui-control-border)] bg-[var(--ui-control-bg)] p-3 font-mono text-[12px] leading-5 text-[var(--ui-fg-dim)]">
            {JSON.stringify(event.raw, null, 2)}
          </pre>
        ) : (
          <Muted>Hidden. Show it to inspect the payload as received.</Muted>
        )}
      </LayerCard>
    </div>
  );
}

export function StackFrames({ frames, inAppOnly }: { frames: Frame[]; inAppOnly: boolean }) {
  const visible = [...frames].reverse().filter((frame) => (inAppOnly ? frame.inApp : true));
  if (visible.length === 0) {
    return <p className="border-t border-[var(--ui-divider)] px-4 py-4 text-[13px] text-[var(--ui-fg-faint)]">No stack frames on this exception.</p>;
  }
  return (
    <ol>
      {visible.map((frame, index) => (
        <li key={index} className="border-t border-[var(--ui-divider)]">
          <div className="flex items-start justify-between gap-3 px-4 py-2.5">
            <div className="min-w-0">
              <div className={frame.inApp ? "text-[13px] font-medium text-[var(--ui-fg)]" : "text-[13px] text-[var(--ui-fg-dim)]"}>
                {frame.function || "<anonymous>"}
              </div>
              <div className="break-all font-mono text-[12px] text-[var(--ui-fg-faint)]">
                {frame.filename || frame.absPath || frame.module || "unknown"}
                {frame.lineno ? `:${frame.lineno}` : ""}
                {frame.colno ? `:${frame.colno}` : ""}
              </div>
            </div>
            {frame.inApp ? <Badge tone="info">In app</Badge> : <Badge>Library</Badge>}
          </div>
          <Context frame={frame} />
        </li>
      ))}
    </ol>
  );
}

function Context({ frame }: { frame: Frame }) {
  const line = frame.lineno ?? 0;
  const rows: { n: number; text: string; current: boolean }[] = [];
  frame.preContext.forEach((text, index) => {
    rows.push({ n: line - frame.preContext.length + index, text, current: false });
  });
  if (frame.contextLine != null) rows.push({ n: line, text: frame.contextLine, current: true });
  frame.postContext.forEach((text, index) => {
    rows.push({ n: line + 1 + index, text, current: false });
  });
  if (rows.length === 0) return null;
  return (
    <div className="mx-4 mb-3 overflow-auto rounded-lg border border-[var(--ui-control-border)] bg-[var(--ui-control-bg)] py-1 font-mono text-[12px] leading-5">
      {rows.map((row) => (
        <div
          key={`${row.n}-${row.text}`}
          className={cn(
            "grid min-w-max grid-cols-[3rem_1fr] gap-3 whitespace-pre pr-3",
            row.current ? "bg-[color-mix(in_oklab,var(--ui-accent)_14%,transparent)] text-[var(--ui-fg)]" : "text-[var(--ui-fg-dim)]",
          )}
        >
          <span className="select-none text-right text-[var(--ui-fg-faint)]">{row.n || ""}</span>
          <span>{row.text || " "}</span>
        </div>
      ))}
    </div>
  );
}

function Muted({ children }: { children: ReactNode }) {
  return <p className="text-center text-[13px] text-[var(--ui-fg-faint)]">{children}</p>;
}

function Pairs({ entries }: { entries: [string, string][] }) {
  return (
    <dl className="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-1 text-[13px]">
      {entries.map(([key, value]) => (
        <div key={key} className="contents">
          <dt className="text-[var(--ui-fg-faint)]">{key}</dt>
          <dd className="break-all text-[var(--ui-fg)]">{value}</dd>
        </div>
      ))}
    </dl>
  );
}

function KeyValueCard({ title, entries }: { title: string; entries: [string, string][] }) {
  return (
    <LayerCard title={title}>
      {entries.length === 0 ? <Muted>None</Muted> : <Pairs entries={entries} />}
    </LayerCard>
  );
}

function ContextCard({ contexts }: { contexts: Record<string, unknown> }) {
  const names = Object.keys(contexts).filter((name) => name !== "trace");
  return (
    <LayerCard title="Contexts">
      {names.length === 0 ? (
        <Muted>No runtime, OS, or browser context.</Muted>
      ) : (
        <div className="flex flex-col gap-4">
          {names.map((name) => (
            <div key={name}>
              <div className="mb-1.5 text-[12px] font-medium text-[var(--ui-fg-dim)] first-letter:uppercase">{name}</div>
              <Pairs entries={entries(contexts[name])} />
            </div>
          ))}
        </div>
      )}
    </LayerCard>
  );
}

function RequestCard({ request }: { request: Record<string, unknown> }) {
  const method = stringOf(request.method);
  const url = stringOf(request.url);
  const query = stringOf(request.query_string);
  const headers = request.headers;
  return (
    <LayerCard title="Request">
      {!method && !url ? (
        <Muted>No request data.</Muted>
      ) : (
        <div className="text-[13px]">
          <div className="break-all font-mono text-[12px] text-[var(--ui-fg)]">
            {method} {url}
          </div>
          {query ? <div className="mt-1 text-[var(--ui-fg-dim)]">Query: {query}</div> : null}
          {headers && typeof headers === "object" ? (
            <div className="mt-3">
              <Pairs entries={entries(headers)} />
            </div>
          ) : null}
        </div>
      )}
    </LayerCard>
  );
}

function entries(value: unknown): [string, string][] {
  if (!value || typeof value !== "object" || Array.isArray(value)) return [];
  return Object.entries(value as Record<string, unknown>)
    .map(([key, item]) => [key, stringOf(item)] as [string, string])
    .filter(([, item]) => item);
}

function sdkEntries(event: NormalizedEvent): [string, string][] {
  const rows: [string, string][] = [
    ["SDK", event.sdk ? `${event.sdk.name} ${event.sdk.version}` : ""],
    ["Platform", event.platform || ""],
    ["Release", event.release || ""],
    ["Environment", event.environment || ""],
    ["Server", event.serverName || ""],
    ["Transaction", event.transaction || ""],
    ["Logger", event.logger || ""],
  ];
  return rows.filter((entry) => entry[1]);
}

function stringOf(value: unknown): string {
  if (value == null) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value);
}
