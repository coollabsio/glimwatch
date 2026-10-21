import { useQuery } from "@tanstack/react-query";
import {
  Activity,
  Bug,
  Boxes,
  Bell,
  CalendarClock,
  LayoutDashboard,
  LineChart,
  Link2,
  LogOut,
  Menu,
  Moon,
  Network,
  Radio,
  Settings,
  Siren,
  Sun,
  X,
} from "lucide-react";
import { useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router";
import { api, type Meta, type User } from "@/lib/api";
import { cn } from "@/lib/utils";

const groups = [
  {
    label: "Workspace",
    links: [
      { to: "/", label: "Overview", icon: LayoutDashboard, end: true },
      { to: "/monitors", label: "Monitors", icon: Activity },
      { to: "/pages", label: "Status pages", icon: Radio },
    ],
  },
  {
    label: "Operations",
    links: [
      { to: "/incidents", label: "Incidents", icon: Siren },
      { to: "/maintenance", label: "Maintenance", icon: CalendarClock },
      { to: "/notifications", label: "Notifications", icon: Bell },
    ],
  },
  {
    label: "Errors",
    links: [
      { to: "/errors", label: "Issues", icon: Bug },
      { to: "/error-projects", label: "Projects", icon: Boxes },
    ],
  },
  {
    label: "Analytics",
    links: [
      { to: "/analytics", label: "Sites", icon: LineChart },
      { to: "/links", label: "Links", icon: Link2 },
    ],
  },
  {
    label: "Manage",
    links: [
      { to: "/agents", label: "Agents", icon: Network },
      { to: "/settings", label: "Settings", icon: Settings },
    ],
  },
];

function NavItems({ onNavigate }: { onNavigate?: () => void }) {
  return (
    <div className="flex flex-col gap-0.5">
      {groups.map((group, index) => (
        <div key={group.label} className={index > 0 ? "mt-3" : undefined}>
          <div className="nav-section">{group.label}</div>
          {group.links.map((link) => (
            <NavLink
              key={link.to}
              to={link.to}
              end={link.end}
              onClick={onNavigate}
              className={({ isActive }) => cn("menu-item", isActive && "menu-item-active")}
            >
              <link.icon className="menu-item-icon" />
              <span className="menu-item-label">{link.label}</span>
            </NavLink>
          ))}
        </div>
      ))}
    </div>
  );
}

export function Shell() {
  const navigate = useNavigate();
  const location = useLocation();
  const me = useQuery({ queryKey: ["me"], queryFn: () => api<User>("/api/auth/me") });
  const meta = useQuery({ queryKey: ["meta"], queryFn: () => api<Meta>("/api/meta") });
  const [open, setOpen] = useState(false);
  const [dark, setDark] = useState(() => document.documentElement.classList.contains("dark"));
  const section = groups.flatMap((group) => group.links).find((link) =>
    link.end
      ? location.pathname === link.to
      : location.pathname === link.to || location.pathname.startsWith(`${link.to}/`),
  );

  async function logout() {
    await api("/api/auth/logout", { method: "POST" });
    navigate("/login");
  }

  function toggleTheme() {
    const next = document.documentElement.classList.toggle("dark");
    localStorage.setItem("haruvo-theme", next ? "dark" : "light");
    setDark(next);
  }

  return (
    <div className="min-h-svh text-[var(--color-fg)]">
      <header className="fixed inset-x-0 top-0 z-40 hidden h-12 items-center border-b border-neutral-200 bg-white/95 backdrop-blur lg:flex dark:border-white/[0.06] dark:bg-[var(--color-panel)]/95">
        <div className="flex h-full w-56 shrink-0 items-center gap-2 border-r border-neutral-200 px-4 dark:border-white/[0.06]">
          <NavLink to="/" className="text-[15px] font-semibold tracking-tight text-black dark:text-white">
            Haruvo
          </NavLink>
          <span className="truncate text-[10.5px] font-medium text-neutral-400 dark:text-[var(--color-fg-faint)]">
            v{meta.data?.version ?? "0.1.0"}
          </span>
        </div>
        <div className="flex h-full min-w-0 flex-1 items-center justify-between px-4">
          <div className="truncate text-[13px] font-medium text-[var(--color-fg-dim)]">{section?.label ?? "Haruvo"}</div>
          <div className="flex items-center gap-1">
            <button type="button" className="icon-button" onClick={toggleTheme} aria-label="Toggle theme">
              {dark ? <Sun className="size-4" /> : <Moon className="size-4" />}
            </button>
            <span className="hidden max-w-48 truncate px-2 text-[13px] text-[var(--color-fg-dim)] sm:inline">
              {me.data?.display_name}
            </span>
            <button type="button" className="icon-button" onClick={() => void logout()} aria-label="Sign out">
              <LogOut className="size-4" />
            </button>
          </div>
        </div>
      </header>

      <div className="sticky top-0 z-40 flex items-center justify-between border-b border-neutral-200 bg-white/95 px-4 py-3 backdrop-blur lg:hidden dark:border-white/[0.06] dark:bg-[var(--color-panel)]/95">
        <NavLink to="/" className="text-[15px] font-semibold tracking-tight">
          Haruvo
        </NavLink>
        <button type="button" className="icon-button" onClick={() => setOpen(true)} aria-label="Open menu">
          <Menu className="size-5" />
        </button>
      </div>

      {open ? (
        <div className="fixed inset-0 z-50 lg:hidden">
          <button className="absolute inset-0 bg-black/50" aria-label="Close menu" onClick={() => setOpen(false)} />
          <div className="absolute inset-y-0 right-0 flex w-72 max-w-[85vw] flex-col rounded-l-2xl border-l border-neutral-200 bg-white shadow-[-8px_0_30px_-6px_rgba(0,0,0,0.18)] dark:border-white/[0.12] dark:bg-[var(--color-panel)]">
            <div className="flex h-12 items-center justify-between border-b border-neutral-200 px-4 dark:border-white/[0.06]">
              <span className="text-[15px] font-semibold">Haruvo</span>
              <button type="button" className="icon-button" onClick={() => setOpen(false)} aria-label="Close menu">
                <X className="size-4" />
              </button>
            </div>
            <div className="flex-1 overflow-y-auto px-3 py-2">
              <NavItems onNavigate={() => setOpen(false)} />
            </div>
            <div className="border-t border-neutral-200 p-3 dark:border-white/[0.06]">
              <button type="button" className="menu-item" onClick={() => void logout()}>
                <LogOut className="menu-item-icon" />
                <span className="menu-item-label">Sign out</span>
              </button>
            </div>
          </div>
        </div>
      ) : null}

      <aside className="fixed top-12 bottom-0 left-0 z-30 hidden w-56 flex-col border-r border-neutral-200 bg-white lg:flex dark:border-white/[0.06] dark:bg-[var(--color-panel)]">
        <nav className="flex min-h-0 flex-1 flex-col overflow-y-auto px-3 pt-2 pb-2">
          <NavItems />
        </nav>
        <div className="border-t border-neutral-200 px-4 py-3 dark:border-white/[0.06]">
          <div className="truncate text-[13px] font-medium text-[var(--color-nav-active)]">{me.data?.display_name ?? "…"}</div>
          <div className="truncate text-[11px] text-[var(--color-nav-muted)]">{me.data?.email}</div>
        </div>
      </aside>

      <main className="min-h-svh bg-neutral-50 px-5 py-6 sm:px-8 lg:ml-56 lg:px-10 lg:pt-[calc(3rem+1.75rem)] lg:pb-10 dark:bg-[var(--color-app)]">
        <div className="mx-auto w-full max-w-[1400px]">
          <Outlet />
        </div>
      </main>
    </div>
  );
}

export function Mark({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        "grid size-8 place-items-center rounded-lg bg-neutral-100 dark:bg-white/[0.06]",
        className,
      )}
    >
      <span className="size-2.5 rounded-full bg-[var(--color-brand)] shadow-[0_0_0_4px_color-mix(in_oklab,var(--color-brand)_25%,transparent)]" />
    </span>
  );
}
