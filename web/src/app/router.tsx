import { createBrowserRouter } from "react-router-dom";

import { AppShell } from "@/app/shell/AppShell";
import { OverviewPage } from "@/pages/OverviewPage";
import { NotFoundPage } from "@/pages/NotFoundPage";
import { PlaceholderPage } from "@/pages/PlaceholderPage";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <OverviewPage /> },
      { path: "proxies", element: <PlaceholderPage title="代理" /> },
      { path: "providers", element: <PlaceholderPage title="Provider" /> },
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
