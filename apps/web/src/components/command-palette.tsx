import { useQuery } from "@tanstack/react-query";
import { type KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router";
import { ArrowRight, Search } from "reicon-react";
import { manage, products } from "@/app/nav";
import {
  api,
  type Incident,
  type InsightLink,
  type InsightSite,
  listAgents,
  type Monitor,
  type Project,
  type StatusPage,
} from "@/lib/api";
import { cn } from "@/lib/utils";

type Item = { group: string; label: string; meta?: string; to: string };

const RESULTS_PER_GROUP = 6;

export const modKeyLabel = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘K" : "Ctrl K";

/* Global search, after Coolify's command palette: a search row on top and the
   results in an inset well, grouped by type. Lists share the page query keys. */
export function CommandPalette({ open, onClose }: { open: boolean; onClose: () => void }) {
  const navigate = useNavigate();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);

  const monitors = useQuery({ queryKey: ["monitors"], queryFn: () => api<Monitor[]>("/api/monitors"), enabled: open });
  const pages = useQuery({ queryKey: ["pages"], queryFn: () => api<StatusPage[]>("/api/pages"), enabled: open });
  const incidents = useQuery({ queryKey: ["incidents"], queryFn: () => api<Incident[]>("/api/incidents"), enabled: open });
  const sites = useQuery({ queryKey: ["sites"], queryFn: () => api<InsightSite[]>("/api/sites"), enabled: open });
  const links = useQuery({ queryKey: ["links"], queryFn: () => api<InsightLink[]>("/api/links"), enabled: open });
  const projects = useQuery({ queryKey: ["projects"], queryFn: () => api<Project[]>("/api/error-projects"), enabled: open });
  const agents = useQuery({ queryKey: ["agents"], queryFn: listAgents, enabled: open });

  const items = useMemo<Item[]>(() => {
    const pagesNav = [
      { group: "Go to", label: "Dashboard", to: "/" },
      ...products.flatMap((product) =>
        product.groups.flatMap((group) =>
          group.links.map((link) => ({ group: "Go to", label: link.label, meta: product.label, to: link.to })),
        ),
      ),
      ...manage.map((link) => ({ group: "Go to", label: link.label, to: link.to })),
    ];
    return [
      ...pagesNav,
      ...(monitors.data ?? []).map((m) => ({ group: "Monitors", label: m.name, meta: m.url, to: `/monitors/${m.id}` })),
      ...(pages.data ?? []).map((p) => ({ group: "Status pages", label: p.title, meta: `/status/${p.slug}`, to: `/pages/${p.id}` })),
      ...(incidents.data ?? []).map((i) => ({ group: "Incidents", label: i.title, meta: i.status, to: `/incidents/${i.id}` })),
      ...(agents.data ?? []).map((a) => ({ group: "Agents", label: a.name, to: "/agents" })),
      ...(projects.data ?? []).map((p) => ({ group: "Error projects", label: p.name, meta: p.platform, to: `/error-projects/${p.id}` })),
      ...(sites.data ?? []).map((s) => ({ group: "Sites", label: s.name, meta: s.domain, to: `/analytics/${s.id}` })),
      ...(links.data ?? []).map((l) => ({ group: "Links", label: `/${l.slug}`, meta: l.destination, to: `/links/${l.id}` })),
    ];
  }, [monitors.data, pages.data, incidents.data, agents.data, projects.data, sites.data, links.data]);

  const results = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const matches = needle
      ? items.filter((item) => `${item.label} ${item.meta ?? ""} ${item.group}`.toLowerCase().includes(needle))
      : items;
    const counts = new Map<string, number>();
    return matches.filter((item) => {
      if (item.group === "Go to" && !needle) return true;
      const count = counts.get(item.group) ?? 0;
      counts.set(item.group, count + 1);
      return count < RESULTS_PER_GROUP;
    });
  }, [items, query]);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setActive(0);
    input.current?.focus();
  }, [open]);

  useEffect(() => setActive(0), [query]);

  useEffect(() => {
    list.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [active]);

  function go(item: Item | undefined) {
    if (!item) return;
    onClose();
    navigate(item.to);
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "ArrowDown") setActive((index) => Math.min(results.length - 1, index + 1));
    else if (event.key === "ArrowUp") setActive((index) => Math.max(0, index - 1));
    else if (event.key === "Enter") go(results[active]);
    else if (event.key === "Escape") onClose();
    else return;
    event.preventDefault();
  }

  if (!open) return null;
  const loading = [monitors, pages, incidents, sites, links, projects, agents].some((q) => q.isLoading);
  return createPortal(
    <div className="command-palette-root" role="dialog" aria-modal="true" aria-label="Search">
      <button type="button" className="command-palette-backdrop" aria-label="Close search" onClick={onClose} />
      <div className="command-palette">
        <div className="command-palette-header">
          <Search className="size-4 shrink-0 text-[var(--ui-fg-faint)]" aria-hidden="true" />
          <input
            ref={input}
            className="command-palette-input"
            placeholder="Search pages, monitors, sites, links..."
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={onKeyDown}
            role="combobox"
            aria-expanded="true"
            aria-controls="command-palette-results"
            aria-activedescendant={results[active] ? `command-palette-item-${active}` : undefined}
            autoComplete="off"
            spellCheck={false}
          />
          <kbd className="command-palette-kbd">Esc</kbd>
        </div>
        <div ref={list} id="command-palette-results" role="listbox" className="command-palette-body">
          {results.length === 0 ? (
            <p className="px-4 py-10 text-center text-[13px] text-[var(--ui-fg-faint)]">
              {loading ? "Loading..." : `No results for "${query.trim()}"`}
            </p>
          ) : (
            results.map((item, index) => (
              <div key={`${item.group}-${item.to}-${item.label}`}>
                {index === 0 || results[index - 1].group !== item.group ? (
                  <div className={cn("command-palette-group-label", index > 0 && "has-divider")}>{item.group}</div>
                ) : null}
                <button
                  type="button"
                  id={`command-palette-item-${index}`}
                  role="option"
                  aria-selected={index === active}
                  data-index={index}
                  className={cn("command-palette-item", index === active && "is-active")}
                  onMouseMove={() => setActive(index)}
                  onClick={() => go(item)}
                >
                  <span className="min-w-0 flex-1">
                    <span className="command-palette-item-name">{item.label}</span>
                    {item.meta ? <span className="command-palette-item-meta">{item.meta}</span> : null}
                  </span>
                  {index === active ? <ArrowRight className="size-3.5 shrink-0 text-[var(--ui-fg-faint)]" aria-hidden="true" /> : null}
                </button>
              </div>
            ))
          )}
        </div>
        <div className="command-palette-footer">
          <span><kbd>↑</kbd><kbd>↓</kbd> to move</span>
          <span><kbd>Enter</kbd> to open</span>
          <span><kbd>{modKeyLabel}</kbd> or <kbd>/</kbd> to search</span>
        </div>
      </div>
    </div>,
    document.body,
  );
}
