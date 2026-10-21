import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { Button, LayerCard, PageHeader } from "@/components/ui";
import { api, type Meta, type User } from "@/lib/api";

export function SettingsPage() {
  const me = useQuery({ queryKey: ["me"], queryFn: () => api<User>("/api/auth/me") });
  const meta = useQuery({ queryKey: ["meta"], queryFn: () => api<Meta>("/api/meta") });
  const [dark, setDark] = useState(() => document.documentElement.classList.contains("dark"));

  function toggleTheme() {
    const next = document.documentElement.classList.toggle("dark");
    localStorage.setItem("haruvo-theme", next ? "dark" : "light");
    setDark(next);
  }

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-6">
      <PageHeader
        title="Settings"
        description="This Haruvo install, the signed-in operator, and the interface theme."
      />
      <LayerCard title="Account">
        <dl className="grid gap-3 text-[13px] sm:grid-cols-3">
          <div>
            <dt className="text-[var(--coollabs-subtle)]">Name</dt>
            <dd className="mt-1 text-[var(--color-fg)]">{me.data?.display_name ?? "…"}</dd>
          </div>
          <div>
            <dt className="text-[var(--coollabs-subtle)]">Email</dt>
            <dd className="mt-1 text-[var(--color-fg)]">{me.data?.email ?? "…"}</dd>
          </div>
          <div>
            <dt className="text-[var(--coollabs-subtle)]">Role</dt>
            <dd className="mt-1 text-[var(--color-fg)] capitalize">{me.data?.role ?? "…"}</dd>
          </div>
        </dl>
      </LayerCard>
      <LayerCard title="Instance">
        <dl className="grid gap-3 text-[13px] sm:grid-cols-2">
          <div>
            <dt className="text-[var(--coollabs-subtle)]">Version</dt>
            <dd className="mt-1 text-[var(--color-fg)]">{meta.data?.version ?? "…"}</dd>
          </div>
          <div>
            <dt className="text-[var(--coollabs-subtle)]">Database</dt>
            <dd className="mt-1 text-[var(--color-fg)] capitalize">{meta.data?.database ?? "…"}</dd>
          </div>
        </dl>
      </LayerCard>
      <LayerCard title="Appearance" description="Coolify is dark first. Both modes use the blue accent.">
        <Button type="button" onClick={toggleTheme}>
          {dark ? "Switch to light" : "Switch to dark"}
        </Button>
      </LayerCard>
    </div>
  );
}
