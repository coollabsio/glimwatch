import { useQuery } from "@tanstack/react-query";
import { Navigate, Outlet, Route, Routes, useLocation } from "react-router";
import { Shell } from "@/app/Shell";
import { ApiError, api, type User } from "@/lib/api";
import { LoginPage, RegisterPage, SetupPage } from "@/pages/auth";
import { AgentsPage } from "@/pages/agents";
import { AnalyticsPage, VisitorPage } from "@/pages/analytics";
import { DashboardPage } from "@/pages/dashboard";
import { HomePage } from "@/pages/home";
import { LinkDetailPage, LinksPage } from "@/pages/links";
import { ErrorProjectsPage, ProjectDetailPage } from "@/pages/error-projects";
import { IncidentDetailPage, IncidentsPage } from "@/pages/incidents";
import { IssueDetailPage } from "@/pages/issue-detail";
import { IssuesPage } from "@/pages/issues";
import { MaintenancePage } from "@/pages/maintenance";
import { MonitorDetailPage, MonitorsPage } from "@/pages/monitors";
import { NotificationsPage } from "@/pages/notifications";
import { PageEditor, PagesPage } from "@/pages/pages";
import { PublicStatusPage } from "@/pages/public";
import { SharePage } from "@/pages/share";
import { SettingsPage } from "@/pages/settings";
import { AppearancePage } from "@/pages/appearance";

function RequireAuth() {
  const location = useLocation();
  const me = useQuery({
    queryKey: ["me"],
    queryFn: () => api<User>("/api/auth/me"),
  });
  if (me.isLoading) {
    return <div className="grid min-h-svh place-items-center text-sm text-muted-foreground">Opening Owlee…</div>;
  }
  if (me.error instanceof ApiError && me.error.status === 401) {
    return <Navigate to="/login" replace state={{ from: location.pathname }} />;
  }
  if (me.error) {
    return <div className="grid min-h-svh place-items-center px-6 text-sm text-destructive">{me.error.message}</div>;
  }
  return <Outlet />;
}

export function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route path="/register" element={<RegisterPage />} />
      <Route path="/setup" element={<SetupPage />} />
      <Route path="/status/:slug" element={<PublicStatusPage />} />
      <Route path="/share/:kind/:slug" element={<SharePage />} />
      <Route element={<RequireAuth />}>
        <Route element={<Shell />}>
          <Route index element={<HomePage />} />
          <Route path="status" element={<DashboardPage />} />
          <Route path="monitors" element={<MonitorsPage />} />
          <Route path="monitors/:id" element={<MonitorDetailPage />} />
          <Route path="pages" element={<PagesPage />} />
          <Route path="pages/:id" element={<PageEditor />} />
          <Route path="incidents" element={<IncidentsPage />} />
          <Route path="incidents/:id" element={<IncidentDetailPage />} />
          <Route path="maintenance" element={<MaintenancePage />} />
          <Route path="notifications" element={<NotificationsPage />} />
          <Route path="agents" element={<AgentsPage />} />
          <Route path="analytics" element={<AnalyticsPage />} />
          <Route path="analytics/:siteId" element={<AnalyticsPage />} />
          <Route path="analytics/:siteId/visitors/:visitorId" element={<VisitorPage />} />
          <Route path="links" element={<LinksPage />} />
          <Route path="links/:linkId" element={<LinkDetailPage />} />
          <Route path="errors" element={<IssuesPage />} />
          <Route path="errors/:id" element={<IssueDetailPage />} />
          <Route path="errors/:id/events/:eventId" element={<IssueDetailPage />} />
          <Route path="error-projects" element={<ErrorProjectsPage />} />
          <Route path="error-projects/:id" element={<ProjectDetailPage />} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="settings/appearance" element={<AppearancePage />} />
        </Route>
      </Route>
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
