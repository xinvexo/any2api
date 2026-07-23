import {
  Activity,
  Fingerprint,
  Gauge,
  KeyRound,
  Network,
  ScrollText,
  Server,
  Settings,
  Waypoints,
  type LucideIcon,
} from "lucide-react";

export interface NavigationItem {
  label: string;
  path: string;
  icon: LucideIcon;
}

export const navigationItems: NavigationItem[] = [
  { label: "总览", path: "/", icon: Gauge },
  { label: "出口代理", path: "/proxies", icon: Network },
  { label: "上游提供", path: "/providers", icon: Server },
  { label: "认证文件", path: "/oauth", icon: Fingerprint },
  { label: "负载均衡", path: "/balancing", icon: Activity },
  { label: "会话粘性", path: "/affinity", icon: Waypoints },
  { label: "网关密钥", path: "/keys", icon: KeyRound },
  { label: "请求日志", path: "/logs", icon: ScrollText },
  { label: "系统设置", path: "/settings", icon: Settings },
];

export function isNavigationPathActive(pathname: string, path: string) {
  if (path === "/") {
    return pathname === "/";
  }
  return pathname === path || pathname.startsWith(`${path}/`);
}

export function findNavigationMatch(pathname: string): NavigationItem | null {
  for (const item of navigationItems) {
    if (isNavigationPathActive(pathname, item.path)) {
      return item;
    }
  }
  return null;
}

export function getPageTitle(pathname: string) {
  return findNavigationMatch(pathname)?.label ?? "页面不存在";
}
