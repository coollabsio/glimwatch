import { useQuery } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { Link } from "react-router";
import { Button, LayerCard, PageHeader } from "@/components/ui";
import { api, type Meta, type User } from "@/lib/api";
import { useAppearance } from "@/lib/theme";

export function SettingsPage() {
  const me = useQuery({ queryKey: ["me"], queryFn: () => api<User>("/api/auth/me") });
  const meta = useQuery({ queryKey: ["meta"], queryFn: () => api<Meta>("/api/meta") });
  const appearance = useAppearance();

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-6">
      <PageHeader
        title="Settings"
        description="This Statixo install, the signed-in operator, and the interface theme."
      />
      <LayerCard title="Account" description="The operator signed in to this browser.">
        <dl className="grid gap-4 sm:grid-cols-3">
          <Item label="Name">{me.data?.display_name ?? "…"}</Item>
          <Item label="Email">{me.data?.email ?? "…"}</Item>
          <Item label="Role"><span className="capitalize">{me.data?.role ?? "…"}</span></Item>
        </dl>
      </LayerCard>
      <LayerCard title="Instance" description="The server this dashboard talks to.">
        <dl className="grid gap-4 sm:grid-cols-2">
          <Item label="Version">{meta.data?.version ?? "…"}</Item>
          <Item label="Database"><span className="capitalize">{meta.data?.database ?? "…"}</span></Item>
        </dl>
      </LayerCard>
      <LayerCard
        title="Appearance"
        description="Theme, page width, navigation, and style in this browser."
        actions={
          <Link to="/settings/appearance">
            <Button type="button">Customize</Button>
          </Link>
        }
      >
        <dl className="grid gap-4 sm:grid-cols-4">
          <Item label="Theme"><span className="capitalize">{appearance.theme}</span></Item>
          <Item label="Page width">{appearance.pageWidth === "full" ? "Full width" : "Centered"}</Item>
          <Item label="Navigation"><span className="capitalize">{appearance.navStyle}</span></Item>
          <Item label="Style">{appearance.navItems === "flat" ? "Flat" : "3D"}</Item>
        </dl>
      </LayerCard>
    </div>
  );
}

function Item({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="min-w-0">
      <dt className="text-[11px] font-medium text-[var(--ui-fg-faint)]">{label}</dt>
      <dd className="mt-1 truncate text-[13px] text-[var(--ui-fg)]">{children}</dd>
    </div>
  );
}
