import { useQuery } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight, Menu, Search, X } from "reicon-react";
import { useEffect, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router";
import { dashboard, manage, type NavLinkItem, type Product, products } from "@/app/nav";
import { CommandPalette, modKeyLabel } from "@/components/command-palette";
import { ProfileMenu } from "@/components/profile-menu";
import { api, type Meta, type User } from "@/lib/api";
import { cn } from "@/lib/utils";

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
            <NavLink
              to={dashboard.to}
              end
              title={dashboard.label}
              onClick={onNavigate}
              className={({ isActive }) => cn("menu-item", isActive && "menu-item-active")}
            >
              <dashboard.icon className="menu-item-icon" />
              <span className="menu-item-label">{dashboard.label}</span>
            </NavLink>
          </li>
          <li>
            <div className="nav-section mt-3">Products</div>
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
  const [collapsed, setCollapsed] = useState(() => localStorage.getItem("sidebarCollapsed") === "true");
  const [searchOpen, setSearchOpen] = useState(false);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const target = event.target as HTMLElement;
      const typing = target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setSearchOpen((value) => !value);
      } else if (event.key === "/" && !typing) {
        event.preventDefault();
        setSearchOpen(true);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
  const routeProduct = products.find((product) =>
    productLinks(product).some((link) => linkActive(location.pathname, link)),
  );
  const shownId = pinned === "root" ? null : (pinned ?? routeProduct?.id ?? null);
  const shown = products.find((product) => product.id === shownId) ?? null;
  const section = [dashboard, ...products.flatMap(productLinks), ...manage].find((link) => linkActive(location.pathname, link));

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

  const searchButton = (onClick?: () => void) => (
    <button
      type="button"
      className="menu-item sidebar-search mb-3"
      title={`Search (${modKeyLabel} or /)`}
      onClick={() => {
        onClick?.();
        setSearchOpen(true);
      }}
    >
      <span className="flex min-w-0 items-center gap-2.5">
        <Search className="menu-item-icon" />
        <span className="menu-item-label">Search</span>
      </span>
      <kbd>{modKeyLabel}</kbd>
    </button>
  );

  function toggleSidebar() {
    setCollapsed((value) => {
      localStorage.setItem("sidebarCollapsed", String(!value));
      return !value;
    });
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
      <header className="fixed inset-x-0 top-0 z-40 hidden h-12 items-center border-b border-[var(--ui-chrome-border)] bg-[var(--ui-panel)]/95 backdrop-blur lg:flex">
        <div
          className={cn(
            "flex h-full shrink-0 items-center gap-2 border-r border-[var(--ui-chrome-border)] bg-[var(--ui-canvas)] transition-[width] duration-200",
            collapsed ? "w-16 justify-center" : "w-56 px-4",
          )}
        >
          <NavLink
            to="/"
            title="Haruvo"
            className="flex items-center gap-2 text-[15px] font-semibold tracking-tight text-[var(--ui-fg)]"
          >
            {collapsed ? <Logo className="size-5" /> : "Haruvo"}
          </NavLink>
          {collapsed ? null : (
            <span className="truncate text-[10.5px] font-medium text-[var(--ui-fg-faint)]">
              v{meta.data?.version ?? "0.1.0"}
            </span>
          )}
        </div>
        <div className="flex h-full min-w-0 flex-1 items-center px-4">
          <div className="truncate text-[13px] font-medium text-[var(--color-fg-dim)]">{section?.label ?? "Dashboard"}</div>
        </div>
      </header>

      <div className="sticky top-0 z-40 flex items-center justify-between border-b border-[var(--ui-chrome-border)] bg-[var(--ui-panel)]/95 px-4 py-3 backdrop-blur lg:hidden">
        <NavLink to="/" className="flex items-center gap-2 text-[15px] font-semibold tracking-tight">
          <Logo className="size-5" />
          Haruvo
        </NavLink>
        <button type="button" className="icon-button" onClick={() => setOpen(true)} aria-label="Open menu">
          <Menu className="size-5" />
        </button>
      </div>

      {open ? (
        <div className="fixed inset-0 z-50 lg:hidden">
          <button className="absolute inset-0 bg-black/50" aria-label="Close menu" onClick={() => setOpen(false)} />
          <div className="absolute inset-y-0 right-0 flex w-72 max-w-[85vw] flex-col rounded-l-2xl border-l border-[var(--ui-card-border)] bg-[var(--ui-panel)] shadow-[var(--ui-modal-shadow)]">
            <div className="flex h-12 items-center justify-between border-b border-[var(--ui-chrome-border)] px-4">
              <span className="flex items-center gap-2 text-[15px] font-semibold">
                <Logo className="size-5" />
                Haruvo
              </span>
              <button type="button" className="icon-button" onClick={() => setOpen(false)} aria-label="Close menu">
                <X className="size-4" />
              </button>
            </div>
            <div className="flex-1 overflow-y-auto px-3 py-2">{searchButton(() => setOpen(false))}
              {nav(() => setOpen(false))}
            </div>
            <div className="border-t border-[var(--ui-chrome-border)] p-3">
              <ProfileMenu user={me.data} onLogout={() => void logout()} onNavigate={() => setOpen(false)} />
            </div>
          </div>
        </div>
      ) : null}

      <aside
        className={cn(
          "fixed top-12 bottom-0 left-0 z-30 hidden flex-col border-r border-[var(--ui-chrome-border)] bg-[var(--ui-canvas)] transition-[width] duration-200 lg:flex",
          collapsed ? "sidebar-collapsed w-16" : "w-56",
        )}
      >
        <nav className="flex min-h-0 flex-1 flex-col overflow-y-auto px-3 pt-2 pb-2">
          {searchButton()}
          {nav()}
        </nav>
        <div
          className={cn(
            "sidebar-footer flex shrink-0 items-center gap-1 bg-[var(--ui-canvas)] px-3 py-2",
            collapsed && "flex-col",
          )}
        >
          <ProfileMenu user={me.data} onLogout={() => void logout()} />
          <button
            type="button"
            className="menu-item sidebar-toggle"
            title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            aria-expanded={!collapsed}
            onClick={toggleSidebar}
          >
            <svg className="sidebar-toggle-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <rect x="3" y="5" width="18" height="14" rx="4" stroke="currentColor" strokeWidth="1.6" />
              <path d="M16 9v6" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
            </svg>
          </button>
        </div>
      </aside>

      <main
        className={cn(
          "min-h-svh bg-[var(--ui-canvas)] px-5 py-6 transition-[margin] duration-200 sm:px-8 lg:px-10 lg:pt-[calc(3rem+1.75rem)] lg:pb-10",
          collapsed ? "lg:ml-16" : "lg:ml-56",
        )}
      >
        <div className="mx-auto w-full max-w-[1400px]">
          <Outlet />
        </div>
      </main>
      <CommandPalette open={searchOpen} onClose={() => setSearchOpen(false)} />
    </div>
  );
}

export function Logo({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 352 352" className={cn("size-6 shrink-0", className)} role="img" aria-label="Haruvo">
      <path d="M0 0h64v128h192V0h64v320h-64V192H64v128H0Z" transform="translate(32 32)" fill="#1e3a8a" />
      <path d="M0 0h64v128h192V0h64v320h-64V192H64v128H0Z" fill="#2b7fff" />
    </svg>
  );
}
