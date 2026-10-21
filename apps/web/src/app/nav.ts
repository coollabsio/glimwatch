import {
  Activity,
  Bell,
  Bug,
  Category,
  CalendarDays,
  ChartLine,
  Element3,
  Global,
  Home,
  Link,
  Setting2,
  Siren,
  Sitemap,
  type IconComponent,
} from "reicon-react";

export type NavLinkItem = {
  to: string;
  label: string;
  icon: IconComponent;
  end?: boolean;
};

export type NavGroup = {
  label?: string;
  links: NavLinkItem[];
};

export type Product = {
  id: string;
  label: string;
  description: string;
  icon: IconComponent;
  groups: NavGroup[];
};

export const dashboard: NavLinkItem = { to: "/", label: "Dashboard", icon: Home, end: true };

export const products: Product[] = [
  {
    id: "status",
    label: "Status",
    description: "Uptime monitors, agents, status pages, and incidents.",
    icon: Activity,
    groups: [
      {
        label: "Workspace",
        links: [
          { to: "/status", label: "Overview", icon: Category, end: true },
          { to: "/monitors", label: "Monitors", icon: Activity },
          { to: "/agents", label: "Agents", icon: Sitemap },
          { to: "/pages", label: "Status pages", icon: Global },
        ],
      },
      {
        label: "Operations",
        links: [
          { to: "/incidents", label: "Incidents", icon: Siren },
          { to: "/maintenance", label: "Maintenance", icon: CalendarDays },
          { to: "/notifications", label: "Notifications", icon: Bell },
        ],
      },
    ],
  },
  {
    id: "errors",
    label: "Errors",
    description: "Exceptions from your apps, grouped into issues.",
    icon: Bug,
    groups: [
      {
        links: [
          { to: "/errors", label: "Issues", icon: Bug },
          { to: "/error-projects", label: "Projects", icon: Element3 },
        ],
      },
    ],
  },
  {
    id: "analytics",
    label: "Analytics",
    description: "Cookieless site analytics and short links.",
    icon: ChartLine,
    groups: [
      {
        links: [
          { to: "/analytics", label: "Sites", icon: ChartLine },
          { to: "/links", label: "Links", icon: Link },
        ],
      },
    ],
  },
];

export const manage: NavLinkItem[] = [
  { to: "/settings", label: "Settings", icon: Setting2 },
];
