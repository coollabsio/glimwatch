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
  region: string;
  checked_at: string;
};

export type HeaderPair = { name: string; value: string };
export type Assertion = { type: string; compare?: string; target?: string | number; key?: string };

export type Monitor = {
  id: string;
  name: string;
  description: string;
  job_type: "http" | "tcp";
  check_type: "http" | "tcp" | "postgres" | "mysql" | "redis";
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
  agent_id: string | null;
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

export type Agent = {
  id: string;
  name: string;
  last_seen_at: string | null;
  online: boolean;
  monitor_count: number;
  created_at: string;
};

export type CreatedAgent = Agent & { token: string };

export const listAgents = () => api<Agent[]>("/api/agents");

export const createAgent = (name: string) =>
  api<CreatedAgent>("/api/agents", { method: "POST", body: JSON.stringify({ name }) });

export const deleteAgent = (id: string) => api<void>(`/api/agents/${id}`, { method: "DELETE" });

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
  public_origin: string;
  demo: { email: string; password: string } | null;
};

export type InsightSite = {
  id: string;
  public_id: string;
  name: string;
  domain: string;
  color: string;
  public_slug: string;
  public_enabled: boolean;
  public_threshold: number;
  public_sections: string[];
  created_at: number;
};

export type InsightPoint = {
  day: number;
  views?: number;
  visitors?: number;
  events?: number;
  visits?: number;
  bounces?: number;
  time?: number;
  clicks?: number;
};

export type InsightOverview = {
  pageviews: number;
  visitors: number;
  events: number;
  visits: number;
  bounces: number;
  total_time: number;
  from: number;
  to: number;
  series: InsightPoint[];
};

export type InsightRow = { label: string; count: number; visitors?: number };

export type InsightDomain = {
  id: string;
  site_id: string | null;
  hostname: string;
  note: string;
  enabled: boolean;
  created_at: number;
};

export type InsightLink = {
  id: string;
  domain_id: string;
  hostname: string;
  slug: string;
  destination: string;
  redirect_type: "temporary" | "permanent";
  enabled: boolean;
  public_slug: string;
  public_enabled: boolean;
  public_threshold: number;
  public_sections: string[];
  created_at: number;
  total_clicks: number;
};

export type LinkOverview = {
  clicks: number;
  visitors: number;
  from: number;
  to: number;
  series: InsightPoint[];
};

export type PublicInsight = {
  name: string;
  subtitle: string;
  sections: string[];
  pageviews?: number | null;
  clicks?: number | null;
  visitors: number | null;
  from: number;
  to: number;
  series: InsightPoint[];
};

export type Project = {
  id: string;
  name: string;
  slug: string;
  platform: string;
  publicKey: string;
  dsn: string;
  createdAt: string;
};

export type Issue = {
  id: string;
  projectId: string;
  projectName: string;
  projectSlug: string;
  shortId: string;
  title: string;
  culprit?: string | null;
  status: "unresolved" | "resolved" | "ignored";
  level: string;
  platform?: string | null;
  environment?: string | null;
  release?: string | null;
  firstSeen: string;
  lastSeen: string;
  eventCount: number;
  unhandled: boolean;
  priority: string;
  userCount: number;
  periodEventCount: number;
  trend24h: number[];
  trend14d: number[];
};

export type IssueFeed = {
  issues: Issue[];
  environments: string[];
};

export type TagCount = { key: string; value: string; count: number };

export type EventSummary = {
  id: string;
  timestamp: string;
  level: string;
  title: string;
  environment?: string | null;
  release?: string | null;
};

export type Frame = {
  filename?: string | null;
  absPath?: string | null;
  function?: string | null;
  module?: string | null;
  lineno?: number | null;
  colno?: number | null;
  contextLine?: string | null;
  preContext: string[];
  postContext: string[];
  inApp: boolean;
};

export type ExceptionInfo = {
  type?: string | null;
  value?: string | null;
  mechanism?: string | null;
  handled?: boolean | null;
  frames: Frame[];
};

export type Breadcrumb = {
  timestamp?: string | null;
  type?: string | null;
  category?: string | null;
  level?: string | null;
  message?: string | null;
  data: Record<string, unknown>;
};

export type NormalizedEvent = {
  eventId: string;
  timestamp: string;
  level: string;
  title: string;
  message?: string | null;
  culprit?: string | null;
  platform?: string | null;
  environment?: string | null;
  release?: string | null;
  serverName?: string | null;
  transaction?: string | null;
  logger?: string | null;
  fingerprint: string;
  sdk?: { name: string; version: string } | null;
  exceptions: ExceptionInfo[];
  breadcrumbs: Breadcrumb[];
  tags: { key: string; value: string }[];
  contexts: Record<string, unknown>;
  user: Record<string, unknown>;
  request: Record<string, unknown>;
  extra: Record<string, unknown>;
  raw: unknown;
};

export type IssueDetail = Issue & {
  events: EventSummary[];
  latest: NormalizedEvent | null;
  histogram: number[];
  periodEvents: number;
  users: number;
  tags: TagCount[];
};
