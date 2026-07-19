import { createBrowserRouter } from "react-router-dom";

import { AppShell } from "@/app/shell/AppShell";
import { OverviewPage } from "@/pages/OverviewPage";
import { NotFoundPage } from "@/pages/NotFoundPage";
import { ModelRoutesPage } from "@/pages/ModelRoutesPage";
import { PlaceholderPage } from "@/pages/PlaceholderPage";
import { ProxiesPage } from "@/pages/ProxiesPage";
import { ProviderCredentialsPage } from "@/pages/ProviderCredentialsPage";
import { ProvidersPage } from "@/pages/ProvidersPage";
import { GatewayApiKeysPage } from "@/pages/GatewayApiKeysPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { AffinityPage } from "@/pages/AffinityPage";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <OverviewPage /> },
      { path: "proxies", element: <ProxiesPage /> },
      { path: "providers", element: <ProvidersPage /> },
      { path: "providers/:endpointId", element: <ProviderCredentialsPage /> },
      { path: "routes", element: <ModelRoutesPage /> },
      { path: "balancing", element: <PlaceholderPage title="负载均衡" /> },
      { path: "affinity", element: <AffinityPage /> },
      { path: "keys", element: <GatewayApiKeysPage /> },
      { path: "logs", element: <PlaceholderPage title="请求日志" /> },
      { path: "settings", element: <SettingsPage /> },
      { path: "*", element: <NotFoundPage /> },
    ],
  },
]);
