import { Navigate, createBrowserRouter } from "react-router-dom";

import { AppShell } from "@/app/shell/AppShell";
import { OverviewPage } from "@/pages/OverviewPage";
import { NotFoundPage } from "@/pages/NotFoundPage";
import { ModelRoutesPage } from "@/pages/ModelRoutesPage";
import { ProxiesPage } from "@/pages/ProxiesPage";
import { ProvidersPage } from "@/pages/ProvidersPage";
import { GatewayApiKeysPage } from "@/pages/GatewayApiKeysPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { AffinityPage } from "@/pages/AffinityPage";
import { BalancingPage } from "@/pages/BalancingPage";
import { RequestLogDetailPage } from "@/pages/RequestLogDetailPage";
import { RequestLogsPage } from "@/pages/RequestLogsPage";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <OverviewPage /> },
      { path: "proxies", element: <ProxiesPage /> },
      { path: "providers", element: <Navigate to="/providers/codex" replace /> },
      { path: "providers/:kind", element: <ProvidersPage /> },
      { path: "routes", element: <ModelRoutesPage /> },
      { path: "balancing", element: <BalancingPage /> },
      { path: "affinity", element: <AffinityPage /> },
      { path: "keys", element: <GatewayApiKeysPage /> },
      { path: "logs", element: <RequestLogsPage /> },
      { path: "logs/:requestId", element: <RequestLogDetailPage /> },
      { path: "settings", element: <Navigate to="/settings/password" replace /> },
      { path: "settings/:section", element: <SettingsPage /> },
      { path: "*", element: <NotFoundPage /> },
    ],
  },
]);
