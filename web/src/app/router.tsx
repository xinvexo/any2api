import { Suspense, lazy } from "react";
import type { ComponentType } from "react";
import { Navigate, createBrowserRouter } from "react-router-dom";

import { AppShell } from "@/app/shell/AppShell";

// Each page ships as its own chunk so the initial bundle stays small.
const page = (load: () => Promise<Record<string, ComponentType>>, name: string) => {
  const Component = lazy(() =>
    load().then((module) => ({ default: module[name] as ComponentType })),
  );
  return (
    <Suspense fallback={null}>
      <Component />
    </Suspense>
  );
};

export const router = createBrowserRouter([
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: page(() => import("@/pages/OverviewPage"), "OverviewPage") },
      { path: "proxies", element: page(() => import("@/pages/ProxiesPage"), "ProxiesPage") },
      { path: "providers", element: page(() => import("@/pages/ProvidersPage"), "ProvidersPage") },
      { path: "oauth", element: page(() => import("@/pages/OAuthPage"), "OAuthPage") },
      // Legacy kind-scoped deep links collapse into the unified providers page.
      { path: "providers/:kind", element: <Navigate to="/providers" replace /> },
      { path: "balancing", element: <Navigate to="/settings/routing" replace /> },
      { path: "affinity", element: <Navigate to="/settings/routing" replace /> },
      {
        path: "keys",
        element: page(() => import("@/pages/GatewayApiKeysPage"), "GatewayApiKeysPage"),
      },
      { path: "logs", element: page(() => import("@/pages/RequestLogsPage"), "RequestLogsPage") },
      {
        path: "logs/:requestId",
        element: page(() => import("@/pages/RequestLogDetailPage"), "RequestLogDetailPage"),
      },
      {
        path: "system-logs",
        element: page(() => import("@/pages/SystemLogsPage"), "SystemLogsPage"),
      },
      { path: "settings", element: <Navigate to="/settings/basic" replace /> },
      {
        path: "settings/:section",
        element: page(() => import("@/pages/SettingsPage"), "SettingsPage"),
      },
      { path: "*", element: page(() => import("@/pages/NotFoundPage"), "NotFoundPage") },
    ],
  },
]);
