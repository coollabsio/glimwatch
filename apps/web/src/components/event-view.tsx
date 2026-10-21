import { useState } from "react";
import type { Frame, NormalizedEvent } from "@/lib/api";
import { fullTime } from "@/lib/utils";
import { Badge } from "@/components/issue-badge";

export function EventView({ event }: { event: NormalizedEvent }) {
  const [inAppOnly, setInAppOnly] = useState(true);
  const [showRaw, setShowRaw] = useState(false);
  const hasInApp = event.exceptions.some((exception) => exception.frames.some((frame) => frame.inApp));

  return (
    <div className="flex flex-col gap-4">
      {event.exceptions.length === 0 ? (
        <section className="card p-4">
          <h2 className="text-[15px] font-medium">{event.title}</h2>
          {event.message ? (
            <p className="mt-2 whitespace-pre-wrap text-[13px] text-[var(--color-fg-dim)]">{event.message}</p>
          ) : null}
        </section>
      ) : (
        event.exceptions.map((exception, index) => (
          <section className="card overflow-hidden" key={`${exception.type}-${index}`}>
            <header className="flex flex-wrap items-start justify-between gap-3 border-b border-[var(--coollabs-hairline)] px-4 py-3">
              <div>
                <div className="text-[12px] uppercase tracking-wide text-[var(--coollabs-subtle)]">
                  Exception {event.exceptions.length > 1 ? index + 1 : ""}
                </div>
                <h2 className="mt-1 text-[15px] font-medium">
                  {exception.type || "Error"}
                  {exception.value ? <span className="font-normal text-[var(--color-fg-dim)]">: {exception.value}</span> : null}
                </h2>
              </div>
              <div className="flex gap-1.5">
                {exception.handled === false ? <Badge tone="fatal">Unhandled</Badge> : null}
                {exception.handled === true ? <Badge tone="info">Handled</Badge> : null}
                {exception.mechanism ? <Badge>{exception.mechanism}</Badge> : null}
              </div>
            </header>
            <div className="flex items-center justify-between px-4 py-2 text-[12px] text-[var(--color-fg-faint)]">
              <span>{exception.frames.length} frames</span>
              {hasInApp ? (
                <label className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={inAppOnly}
                    onChange={(event) => setInAppOnly(event.target.checked)}
                  />
                  In-app frames only
                </label>
              ) : null}
            </div>
            <StackFrames frames={exception.frames} inAppOnly={inAppOnly && hasInApp} />
          </section>
        ))
      )}

      <div className="grid gap-4 xl:grid-cols-2">
        <section className="card p-4">
          <h3 className="mb-3 text-[13px] font-medium">Breadcrumbs</h3>
          {event.breadcrumbs.length === 0 ? (
            <p className="text-[13px] text-[var(--color-fg-faint)]">No breadcrumbs were sent with this event.</p>
          ) : (
            <ol className="flex flex-col gap-2">
              {event.breadcrumbs.map((crumb, index) => (
                <li key={index} className="border-l-2 border-[var(--coollabs-hairline)] pl-3">
                  <div className="flex flex-wrap gap-2 text-[12px] text-[var(--color-fg-faint)]">
                    <span>{crumb.category || crumb.type || "default"}</span>
                    {crumb.level ? <Badge tone={crumb.level}>{crumb.level}</Badge> : null}
                    {crumb.timestamp ? <span title={fullTime(crumb.timestamp)}>{fullTime(crumb.timestamp)}</span> : null}
                  </div>
                  {crumb.message ? <div className="text-[13px]">{crumb.message}</div> : null}
                  {Object.keys(crumb.data || {}).length > 0 ? (
                    <pre className="mt-1 overflow-auto text-[11px] text-[var(--color-fg-dim)]">
                      {JSON.stringify(crumb.data, null, 2)}
                    </pre>
                  ) : null}
                </li>
              ))}
            </ol>
          )}
        </section>

        <div className="flex flex-col gap-4">
          <KeyValueCard title="Tags" entries={event.tags.map((tag) => [tag.key, tag.value])} />
          <ContextCard contexts={event.contexts} />
          <KeyValueCard title="User" entries={entries(event.user)} />
          <RequestCard request={event.request} />
          <KeyValueCard
            title="SDK and release"
            entries={sdkEntries(event)}
          />
          <KeyValueCard title="Extra" entries={entries(event.extra)} />
        </div>
      </div>

      <section className="card p-4">
        <button className="text-[13px] font-medium" onClick={() => setShowRaw((value) => !value)}>
          {showRaw ? "Hide" : "Show"} raw event JSON
        </button>
        {showRaw ? (
          <pre className="mt-3 max-h-[480px] overflow-auto rounded-md bg-black/5 p-3 text-[12px] leading-5 text-[var(--color-fg-dim)] dark:bg-black/30">
            {JSON.stringify(event.raw, null, 2)}
          </pre>
        ) : null}
      </section>
    </div>
  );
}

export function StackFrames({ frames, inAppOnly }: { frames: Frame[]; inAppOnly: boolean }) {
  const visible = [...frames].reverse().filter((frame) => (inAppOnly ? frame.inApp : true));
  if (visible.length === 0) {
    return <p className="px-4 pb-4 text-[13px] text-[var(--color-fg-faint)]">No stack frames on this exception.</p>;
  }
  return (
    <ol>
      {visible.map((frame, index) => (
        <li key={index} className="border-t border-[var(--coollabs-hairline)]">
          <div className="flex items-start justify-between gap-3 px-4 py-2">
            <div>
              <div className={frame.inApp ? "text-[13px] font-medium" : "text-[13px] text-[var(--color-fg-dim)]"}>
                {frame.function || "<anonymous>"}
              </div>
              <div className="font-mono text-[12px] text-[var(--color-fg-faint)]">
                {frame.filename || frame.absPath || frame.module || "unknown"}
                {frame.lineno ? `:${frame.lineno}` : ""}
                {frame.colno ? `:${frame.colno}` : ""}
              </div>
            </div>
            {frame.inApp ? <Badge tone="info">in app</Badge> : <Badge>library</Badge>}
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
    <div className="mb-2 overflow-auto">
      {rows.map((row) => (
        <div key={`${row.n}-${row.text}`} className={`stack-line ${row.current ? "current" : ""}`}>
          <span className="text-right text-[var(--color-fg-faint)]">{row.n || ""}</span>
          <span>{row.text || " "}</span>
        </div>
      ))}
    </div>
  );
}

function KeyValueCard({ title, entries }: { title: string; entries: [string, string][] }) {
  return (
    <section className="card p-4">
      <h3 className="mb-3 text-[13px] font-medium">{title}</h3>
      {entries.length === 0 ? (
        <p className="text-[13px] text-[var(--color-fg-faint)]">None</p>
      ) : (
        <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-[13px]">
          {entries.map(([key, value]) => (
            <div key={key} className="contents">
              <dt className="text-[var(--color-fg-faint)]">{key}</dt>
              <dd className="break-all">{value}</dd>
            </div>
          ))}
        </dl>
      )}
    </section>
  );
}

function ContextCard({ contexts }: { contexts: Record<string, unknown> }) {
  const names = Object.keys(contexts).filter((name) => name !== "trace");
  return (
    <section className="card p-4">
      <h3 className="mb-3 text-[13px] font-medium">Contexts</h3>
      {names.length === 0 ? (
        <p className="text-[13px] text-[var(--color-fg-faint)]">No runtime, OS, or browser context.</p>
      ) : (
        <div className="flex flex-col gap-3">
          {names.map((name) => (
            <div key={name}>
              <div className="mb-1 text-[12px] capitalize text-[var(--coollabs-subtle)]">{name}</div>
              <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-0.5 text-[13px]">
                {entries(contexts[name]).map(([key, value]) => (
                  <div key={key} className="contents">
                    <dt className="text-[var(--color-fg-faint)]">{key}</dt>
                    <dd className="break-all">{value}</dd>
                  </div>
                ))}
              </dl>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function RequestCard({ request }: { request: Record<string, unknown> }) {
  const method = stringOf(request.method);
  const url = stringOf(request.url);
  const query = stringOf(request.query_string);
  const headers = request.headers;
  return (
    <section className="card p-4">
      <h3 className="mb-3 text-[13px] font-medium">Request</h3>
      {!method && !url ? (
        <p className="text-[13px] text-[var(--color-fg-faint)]">No request data.</p>
      ) : (
        <div className="text-[13px]">
          <div className="font-mono">
            {method} {url}
          </div>
          {query ? <div className="mt-1 text-[var(--color-fg-dim)]">Query: {query}</div> : null}
          {headers && typeof headers === "object" ? (
            <dl className="mt-3 grid grid-cols-[max-content_1fr] gap-x-4 gap-y-0.5">
              {entries(headers).map(([key, value]) => (
                <div key={key} className="contents">
                  <dt className="text-[var(--color-fg-faint)]">{key}</dt>
                  <dd className="break-all">{value}</dd>
                </div>
              ))}
            </dl>
          ) : null}
        </div>
      )}
    </section>
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
