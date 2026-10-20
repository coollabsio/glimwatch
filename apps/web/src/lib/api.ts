export class ApiError extends Error {
  status: number;
  fields?: Record<string, string>;

  constructor(message: string, status: number, fields?: Record<string, string>) {
    super(message);
    this.status = status;
    this.fields = fields;
  }
}

export async function api<T>(path: string, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers);
  if (init?.body && !headers.has("content-type")) {
    headers.set("content-type", "application/json");
  }
  const response = await fetch(path, { ...init, headers });
  if (response.status === 204) return undefined as T;
  const text = await response.text();
  const data = text ? (JSON.parse(text) as Record<string, unknown>) : {};
  if (!response.ok) {
    throw new ApiError(
      typeof data.detail === "string" ? data.detail : "Request failed",
      response.status,
      data.fields as Record<string, string> | undefined,
    );
  }
  return data as T;
}

export type Bar = {
  day: string;
  status: "up" | "degraded" | "down" | "maintenance" | "empty" | string;
  uptime_ratio: number | null;
  total: number;
  avg_latency_ms: number | null;
};

export type Check = {
  id: string;
  status: string;
  status_code: number | null;
  latency_ms: number;
  error: string | null;
  checked_at: string;
};

export type HeaderPair = { name: string; value: string };
export type Assertion = { type: string; compare?: string; target?: string | number; key?: string };

export type Monitor = {
  id: string;
  name: string;
  description: string;
  job_type: "http" | "tcp";
  url: string;
  method: string;
  headers: HeaderPair[];
  body: string;
  timeout_ms: number;
  interval_seconds: number;
  degraded_after_ms: number | null;
  expected_status: number | null;
  assertions: Assertion[];
  follow_redirects: boolean;
  failure_threshold: number;
  active: boolean;
  state: "pending" | "up" | "degraded" | "down";
  consecutive_failures: number;
  last_checked_at: string | null;
  next_check_at: string;
  created_at: string;
  uptime_24h: number | null;
  uptime_30d: number | null;
  uptime_90d: number | null;
  avg_latency_ms: number | null;
  bars: Bar[];
  recent?: Check[];
};

export type Component = {
  id: string;
  monitor_id: string;
  name: string;
  description: string;
  position: number;
};

export type StatusPage = {
  id: string;
  title: string;
  description: string;
  slug: string;
  published: boolean;
  homepage_url: string | null;
  created_at: string;
  components: Component[];
};

export type IncidentUpdate = {
  id: string;
  status: string;
  message: string;
  created_at: string;
};

export type Incident = {
  id: string;
  page_id: string | null;
  monitor_id: string | null;
  title: string;
  status: "investigating" | "identified" | "monitoring" | "resolved";
  impact: "none" | "minor" | "major" | "critical";
  auto: boolean;
  started_at: string;
  resolved_at: string | null;
  updates: IncidentUpdate[];
};

export type Maintenance = {
  id: string;
  page_id: string | null;
  title: string;
  message: string;
  starts_at: string;
  ends_at: string;
  monitor_ids: string[];
  active: boolean;
};

export type Delivery = {
  id: string;
  event: string;
  status: string;
  error: string | null;
  created_at: string;
};

export type Channel = {
  id: string;
  name: string;
  provider: "email" | "webhook" | "slack" | "discord";
  config: Record<string, string>;
  enabled: boolean;
  monitor_ids: string[];
  deliveries: Delivery[];
};

export type User = { id: string; email: string; display_name: string; role: string };

export type Audit = {
  id: string;
  action: string;
  resource_type: string;
  resource_id: string | null;
  occurred_at: string;
};

export type Overview = { monitors: Monitor[]; incidents: Incident[]; activity: Audit[] };

export type PublicComponent = {
  id: string;
  name: string;
  description: string;
  state: Monitor["state"];
  uptime_90d: number | null;
  bars: Bar[];
};

export type PublicPage = {
  title: string;
  description: string;
  slug: string;
  homepage_url: string | null;
  banner: "operational" | "degraded" | "partial_outage" | "major_outage";
  components: PublicComponent[];
  incidents: Incident[];
  maintenances: Maintenance[];
};

export type Meta = {
  version: string;
  signup_open: boolean;
  database: string;
  demo: { email: string; password: string } | null;
};
