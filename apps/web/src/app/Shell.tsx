import { useQuery } from "@tanstack/react-query";
import {
  Activity,
  Bell,
  Boxes,
  Bug,
  CalendarClock,
  ChevronLeft,
  ChevronRight,
  LayoutDashboard,
  LineChart,
  Link2,
  type LucideIcon,
  Menu,
  Network,
  Radio,
  Settings,
  Siren,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router";
import { ProfileMenu } from "@/components/profile-menu";
import { api, type Meta, type User } from "@/lib/api";
import { cn } from "@/lib/utils";

type NavLinkItem = {
  to: string;
  label: string;
  icon: LucideIcon;
  end?: boolean;
};

type NavGroup = {
  label?: string;
  links: NavLinkItem[];
};

type Product = {
  id: string;
  label: string;
  icon: LucideIcon;
  groups: NavGroup[];
};

const products: Product[] = [
  {
    id: "status",
    label: "Status",
    icon: Activity,
    groups: [
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
    ],
  },
  {
    id: "errors",
    label: "Errors",
    icon: Bug,
    groups: [
      {
        links: [
          { to: "/errors", label: "Issues", icon: Bug },
          { to: "/error-projects", label: "Projects", icon: Boxes },
        ],
      },
    ],
  },
  {
    id: "analytics",
    label: "Analytics",
    icon: LineChart,
    groups: [
      {
        links: [
          { to: "/analytics", label: "Sites", icon: LineChart },
          { to: "/links", label: "Links", icon: Link2 },
        ],
      },
    ],
  },
];

const manage: NavLinkItem[] = [
  { to: "/agents", label: "Agents", icon: Network },
  { to: "/settings", label: "Settings", icon: Settings },
];

function linkActive(pathname: string, link: NavLinkItem) {
  if (link.end || link.to === "/") return pathname === link.to;
  return pathname === link.to || pathname.startsWith(`${link.to}/`);
}

function productLinks(product: Product) {
  return product.groups.flatMap((group) => group.links);
}

function NavItems({
  pathname,
  shown,
  swap,
  onOpenProduct,
  onBack,
  onNavigate,
}: {
  pathname: string;
  shown: Product | null;
  swap: "" | "forward" | "back";
  onOpenProduct: (id: string) => void;
  onBack: () => void;
  onNavigate?: () => void;
}) {
  return (
    <ul
      key={`${shown?.id ?? "root"}-${swap}`}
      className={cn(
        "flex flex-col gap-0.5",
        swap === "forward" && "nav-swap-forward",
        swap === "back" && "nav-swap-back",
      )}
    >
      {shown ? (
        <>
          <li className="mb-1">
            <button
              type="button"
              className="menu-item menu-back"
              title="Back to the main menu"
              aria-label="Back to the main menu"
              onClick={onBack}
            >
              <span className="menu-back-icon">
                <ChevronLeft className="size-3" />
              </span>
              <span className="menu-back-label">{shown.label}</span>
            </button>
          </li>
          {shown.groups.map((group, index) => (
            <li key={group.label ?? index}>
              {group.label ? <div className={cn("nav-section", index > 0 && "mt-3")}>{group.label}</div> : null}
              <ul className="flex flex-col gap-0.5">
                {group.links.map((link) => (
                  <li key={link.to}>
                    <NavLink
                      to={link.to}
                      end={link.end}
                      title={link.label}
                      onClick={onNavigate}
                      className={({ isActive }) => cn("menu-item", isActive && "menu-item-active")}
                    >
                      <link.icon className="menu-item-icon" />
                      <span className="menu-item-label">{link.label}</span>
                    </NavLink>
                  </li>
                ))}
              </ul>
            </li>
          ))}
        </>
      ) : (
        <>
          <li>
            <div className="nav-section">Products</div>
            <ul className="flex flex-col gap-0.5">
              {products.map((product) => {
                const active = productLinks(product).some((link) => linkActive(pathname, link));
                return (
                  <li key={product.id}>
                    <button
                      type="button"
                      className={cn("menu-item", active && "menu-item-active")}
                      title={product.label}
                      onClick={() => onOpenProduct(product.id)}
                    >
                      <product.icon className="menu-item-icon" />
                      <span className="menu-item-label">{product.label}</span>
                      <ChevronRight className="menu-chevron" aria-hidden="true" />
                    </button>
                  </li>
                );
              })}
            </ul>
          </li>
          <li>
            <div className="nav-section mt-3">Manage</div>
            <ul className="flex flex-col gap-0.5">
              {manage.map((link) => (
                <li key={link.to}>
                  <NavLink
                    to={link.to}
                    end={link.end}
                    title={link.label}
                    onClick={onNavigate}
                    className={({ isActive }) => cn("menu-item", isActive && "menu-item-active")}
                  >
                    <link.icon className="menu-item-icon" />
                    <span className="menu-item-label">{link.label}</span>
                  </NavLink>
                </li>
              ))}
            </ul>
          </li>
        </>
      )}
    </ul>
  );
}

export function Shell() {
  const navigate = useNavigate();
  const location = useLocation();
  const me = useQuery({ queryKey: ["me"], queryFn: () => api<User>("/api/auth/me") });
  const meta = useQuery({ queryKey: ["meta"], queryFn: () => api<Meta>("/api/meta") });
  const [open, setOpen] = useState(false);
  const [pinned, setPinned] = useState<string | "root" | null>(null);
  const [swap, setSwap] = useState<"" | "forward" | "back">("");
  const routeProduct = products.find((product) =>
    productLinks(product).some((link) => linkActive(location.pathname, link)),
  );
  const shownId = pinned === "root" ? null : (pinned ?? routeProduct?.id ?? null);
  const shown = products.find((product) => product.id === shownId) ?? null;
  const section = [...products.flatMap(productLinks), ...manage].find((link) => linkActive(location.pathname, link));

  useEffect(() => {
    setPinned(null);
  }, [location.pathname]);

  function openProduct(id: string) {
    setSwap("forward");
    setPinned(id);
  }

  function backToProducts() {
    setSwap("back");
    setPinned("root");
  }

  async function logout() {
    await api("/api/auth/logout", { method: "POST" });
    navigate("/login");
  }

  const nav = (onNavigate?: () => void) => (
    <NavItems
      pathname={location.pathname}
      shown={shown}
      swap={swap}
      onOpenProduct={openProduct}
      onBack={backToProducts}
      onNavigate={onNavigate}
    />
  );

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
        <div className="flex h-full min-w-0 flex-1 items-center px-4">
          <div className="truncate text-[13px] font-medium text-[var(--color-fg-dim)]">{section?.label ?? "Haruvo"}</div>
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
            <div className="flex-1 overflow-y-auto px-3 py-2">{nav(() => setOpen(false))}</div>
            <div className="border-t border-neutral-200 p-3 dark:border-white/[0.06]">
              <ProfileMenu user={me.data} onLogout={() => void logout()} onNavigate={() => setOpen(false)} />
            </div>
          </div>
        </div>
      ) : null}

      <aside className="fixed top-12 bottom-0 left-0 z-30 hidden w-56 flex-col border-r border-neutral-200 bg-white lg:flex dark:border-white/[0.06] dark:bg-[var(--color-panel)]">
        <nav className="flex min-h-0 flex-1 flex-col overflow-y-auto px-3 pt-2 pb-2">{nav()}</nav>
        <div className="sidebar-footer shrink-0 border-t border-neutral-200 bg-white px-3 py-2 dark:border-white/[0.06] dark:bg-[var(--color-panel)]">
          <ProfileMenu user={me.data} onLogout={() => void logout()} />
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
