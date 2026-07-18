import { createBrowserRouter } from "react-router-dom";

import { AppShell } from "@/app/shell/AppShell";
import { OverviewPage } from "@/pages/OverviewPage";
import { NotFoundPage } from "@/pages/NotFoundPage";
import { PlaceholderPage } from "@/pages/PlaceholderPage";
import { ProxiesPage } from "@/pages/ProxiesPage";
import { ProvidersPage } from "@/pages/ProvidersPage";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <OverviewPage /> },
      { path: "proxies", element: <ProxiesPage /> },
      { path: "providers", element: <ProvidersPage /> },
      { path: "routes", element: <PlaceholderPage title="模型路由" /> },
      { path: "balancing", element: <PlaceholderPage title="负载均衡" /> },
      { path: "affinity", element: <PlaceholderPage title="会话粘性" /> },
      { path: "keys", element: <PlaceholderPage title="网关密钥" /> },
      { path: "logs", element: <PlaceholderPage title="请求日志" /> },
      { path: "settings", element: <PlaceholderPage title="设置" /> },
      { path: "*", element: <NotFoundPage /> },
    ],
  },
]);
