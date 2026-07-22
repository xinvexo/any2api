import {
  Activity,
  Gauge,
  KeyRound,
  Network,
  Route,
  ScrollText,
  Server,
  Settings,
  Waypoints,
  type LucideIcon,
} from "lucide-react";

import { PROVIDER_KIND_OPTIONS } from "@/features/providers";
import { SETTING_SECTIONS } from "@/features/settings";

export interface NavigationChild {
  label: string;
  path: string;
}

export interface NavigationItem {
  label: string;
  /** Default landing path for the section (usually first child or the page itself). */
  path: string;
  icon: LucideIcon;
  children?: readonly NavigationChild[];
}

export const SETTINGS_NAV_CHILDREN: readonly NavigationChild[] = [
  { label: "管理员密码", path: "/settings/password" },
  { label: "全局代理", path: "/settings/proxy" },
  ...SETTING_SECTIONS.map((section) => ({
    label: section.label,
    path: `/settings/${section.id}`,
  })),
];

export const PROVIDER_NAV_CHILDREN: readonly NavigationChild[] = PROVIDER_KIND_OPTIONS.map(
  (option) => ({
    label: option.label,
    path: `/providers/${option.kind}`,
  }),
);

export const navigationItems: NavigationItem[] = [
  { label: "总览", path: "/", icon: Gauge },
  { label: "代理", path: "/proxies", icon: Network },
  {
    label: "上游提供商",
    path: PROVIDER_NAV_CHILDREN[0]?.path ?? "/providers/codex",
    icon: Server,
    children: PROVIDER_NAV_CHILDREN,
  },
  { label: "模型路由", path: "/routes", icon: Route },
  { label: "负载均衡", path: "/balancing", icon: Activity },
  { label: "会话粘性", path: "/affinity", icon: Waypoints },
  { label: "网关密钥", path: "/keys", icon: KeyRound },
  { label: "请求日志", path: "/logs", icon: ScrollText },
  {
    label: "设置",
    path: SETTINGS_NAV_CHILDREN[0]?.path ?? "/settings/password",
    icon: Settings,
    children: SETTINGS_NAV_CHILDREN,
  },
];

export function isNavigationPathActive(pathname: string, path: string) {
  if (path === "/") {
    return pathname === "/";
  }
  return pathname === path || pathname.startsWith(`${path}/`);
}

export function findNavigationMatch(pathname: string): {
  item: NavigationItem;
  child?: NavigationChild;
} | null {
  for (const item of navigationItems) {
    if (item.children) {
      const child = item.children.find(
        (entry) => pathname === entry.path || pathname.startsWith(`${entry.path}/`),
      );
      if (child) {
        return { item, child };
      }
      // Parent path prefix for redirects like /providers or /settings.
      const base = item.path.split("/").slice(0, 2).join("/") || item.path;
      if (pathname === base || pathname.startsWith(`${base}/`)) {
        return { item };
      }
    } else if (isNavigationPathActive(pathname, item.path)) {
      return { item };
    }
  }
  return null;
}

export function getPageTitle(pathname: string) {
  const match = findNavigationMatch(pathname);
  if (!match) {
    return "页面不存在";
  }
  if (match.child) {
    return match.child.label;
  }
  return match.item.label;
}
