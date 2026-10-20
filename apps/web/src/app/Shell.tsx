import { useQuery } from "@tanstack/react-query";
import { Activity, Bell, CalendarClock, LayoutDashboard, LogOut, Moon, Radio, Siren, Sun } from "lucide-react";
import { useState } from "react";
import { NavLink, Outlet, useNavigate } from "react-router";
import { api, type User } from "@/lib/api";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui";

const links = [
  { to: "/", label: "Overview", icon: LayoutDashboard, end: true },
  { to: "/monitors", label: "Monitors", icon: Activity },
  { to: "/pages", label: "Status pages", icon: Radio },
  { to: "/incidents", label: "Incidents", icon: Siren },
  { to: "/maintenance", label: "Maintenance", icon: CalendarClock },
  { to: "/notifications", label: "Notifications", icon: Bell },
];

export function Shell() {
  const navigate = useNavigate();
  const me = useQuery({ queryKey: ["me"], queryFn: () => api<User>("/api/auth/me") });
  const [dark, setDark] = useState(() => document.documentElement.classList.contains("dark"));

  async function logout() {
    await api("/api/auth/logout", { method: "POST" });
    navigate("/login");
  }

  function toggleTheme() {
    const next = document.documentElement.classList.toggle("dark");
    localStorage.setItem("coolstatus-theme", next ? "dark" : "light");
    setDark(next);
  }

  return (
    <div className="min-h-svh lg:grid lg:grid-cols-[240px_1fr]">
      <aside className="border-b bg-sidebar text-sidebar-foreground lg:sticky lg:top-0 lg:flex lg:h-svh lg:flex-col lg:border-r lg:border-b-0">
        <div className="flex items-center gap-3 px-4 py-4">
          <Mark />
          <div>
            <div className="text-sm font-semibold tracking-tight">CoolStatus</div>
            <div className="text-xs text-muted-foreground">coolLabs</div>
          </div>
        </div>
        <nav className="flex gap-1 overflow-x-auto px-2 pb-3 lg:flex-1 lg:flex-col lg:overflow-visible lg:px-3">
          {links.map((link) => (
            <NavLink
              key={link.to}
              to={link.to}
              end={link.end}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-2 rounded-lg px-3 py-2 text-sm whitespace-nowrap",
                  isActive ? "bg-primary/15 font-medium text-primary" : "text-muted-foreground hover:bg-muted hover:text-foreground",
                )
              }
            >
              <link.icon className="size-4" />
              {link.label}
            </NavLink>
          ))}
        </nav>
        <div className="hidden items-center justify-between gap-2 border-t px-3 py-3 lg:flex">
          <div className="min-w-0">
            <div className="truncate text-sm font-medium">{me.data?.display_name ?? "…"}</div>
            <div className="truncate text-xs text-muted-foreground">{me.data?.email}</div>
          </div>
          <Button variant="ghost" size="icon" onClick={() => void logout()} aria-label="Sign out">
            <LogOut />
          </Button>
        </div>
      </aside>
      <div className="min-w-0">
        <header className="flex items-center justify-end gap-2 border-b px-4 py-3 lg:px-8">
          <Button variant="outline" size="sm" onClick={toggleTheme}>
            {dark ? <Sun /> : <Moon />}
            {dark ? "Light" : "Dark"}
          </Button>
          <Button variant="outline" size="sm" className="lg:hidden" onClick={() => void logout()}>
            <LogOut /> Sign out
          </Button>
        </header>
        <main className="mx-auto w-full max-w-6xl px-4 py-6 lg:px-8 lg:py-8">
          <Outlet />
        </main>
      </div>
    </div>
  );
}

export function Mark({ className }: { className?: string }) {
  return (
    <span className={cn("grid size-9 place-items-center rounded-lg bg-foreground text-background", className)}>
      <span className="size-2.5 rounded-full bg-primary shadow-[0_0_0_4px_color-mix(in_oklch,var(--primary),transparent_70%)]" />
    </span>
  );
}
