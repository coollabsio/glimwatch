import { ChevronDown, Logout, Profile, Setting2 } from "reicon-react";
import { useEffect, useId, useRef, useState } from "react";
import { Link } from "react-router";
import type { User } from "@/lib/api";
import { cn } from "@/lib/utils";

function initial(name: string, email: string) {
  const source = name.trim() || email.trim() || "A";
  return source.charAt(0).toUpperCase();
}

function roleLabel(role: string) {
  if (!role) return "";
  return role.charAt(0).toUpperCase() + role.slice(1);
}

export function ProfileMenu({
  user,
  onLogout,
  onNavigate,
  compact = false,
}: {
  user?: User;
  onLogout: () => void;
  onNavigate?: () => void;
  /** Avatar-only trigger with the menu opening down, for the mobile top bar. */
  compact?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const panelId = useId();
  const name = user?.display_name || "Account";
  const email = user?.email ?? "";
  const role = roleLabel(user?.role ?? "");

  function close() {
    setOpen(false);
  }

  useEffect(() => {
    if (!open) return;
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.stopPropagation();
        close();
      }
    }
    function onPointer(event: PointerEvent) {
      if (!root.current?.contains(event.target as Node)) close();
    }
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onPointer);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onPointer);
    };
  }, [open]);

  return (
    <div className={cn("profile-menu", compact && "is-compact")} ref={root}>
      <button
        type="button"
        className="button profile-trigger"
        title={name}
        aria-label={`Account menu for ${name}`}
        aria-haspopup="true"
        aria-expanded={open}
        aria-controls={panelId}
        onClick={() => setOpen((value) => !value)}
      >
        <span className="profile-avatar" aria-hidden="true">
          {initial(name, email)}
        </span>
        <span className="profile-trigger-name">{name}</span>
        <ChevronDown className={cn("profile-chevron", open && "is-open")} aria-hidden="true" />
      </button>
      {open ? (
        <div id={panelId} className="profile-menu-panel">
          <div className="profile-menu-identity">
            <div className="profile-menu-name">{name}</div>
            {email ? <div className="profile-menu-email">{email}</div> : null}
            {role ? <div className="profile-menu-email">{role}</div> : null}
          </div>
          <div className="profile-menu-divider" />
          <Link
            className="listbox-option"
            to="/settings"
            onClick={() => {
              close();
              onNavigate?.();
            }}
          >
            <span className="flex items-center gap-2">
              <Profile className="size-4 opacity-80" />
              Profile
            </span>
          </Link>
          <Link
            className="listbox-option"
            to="/settings/appearance"
            onClick={() => {
              close();
              onNavigate?.();
            }}
          >
            <span className="flex items-center gap-2">
              <Setting2 className="size-4 opacity-80" />
              Appearance
            </span>
          </Link>
          <div className="profile-menu-divider" />
          <button
            type="button"
            className="listbox-option is-danger"
            onClick={() => {
              close();
              onNavigate?.();
              onLogout();
            }}
          >
            <span className="flex items-center gap-2">
              <Logout className="size-4 opacity-90" />
              Log out
            </span>
          </button>
        </div>
      ) : null}
    </div>
  );
}
