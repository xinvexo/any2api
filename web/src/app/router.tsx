import { Suspense, lazy } from "react";
import type { ComponentType } from "react";
import { Navigate, createBrowserRouter, type RouteObject } from "react-router-dom";

import { PageLoadingFallback } from "@/app/error-recovery/PageLoadingFallback";
import { RouteErrorPage } from "@/app/error-recovery/RouteErrorPage";
import { AppShell } from "@/app/shell/AppShell";

// Each page ships as its own chunk so the initial bundle stays small.
const page = (load: () => Promise<Record<string, ComponentType>>, name: string) => {
  const Component = lazy(() =>
    load().then((module) => ({ default: module[name] as ComponentType })),
  );
  return (
    <Suspense fallback={<PageLoadingFallback />}>
      <Component />
    </Suspense>
  );
};

export const appRoutes = [
  {
    path: "/",
    element: <AppShell />,
    errorElement: <RouteErrorPage />,
    children: [
      { index: true, element: page(() => import("@/pages/OverviewPage"), "OverviewPage") },
      { path: "proxies", element: page(() => import("@/pages/ProxiesPage"), "ProxiesPage") },
      { path: "providers", element: page(() => import("@/pages/ProvidersPage"), "ProvidersPage") },
      { path: "oauth", element: page(() => import("@/pages/OAuthPage"), "OAuthPage") },
      { path: "routes", element: page(() => import("@/pages/RoutesPage"), "RoutesPage") },
      {
        path: "quota-rates",
        element: page(() => import("@/pages/QuotaRatesPage"), "QuotaRatesPage"),
      },
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
] satisfies RouteObject[];

export const router = createBrowserRouter(appRoutes);
