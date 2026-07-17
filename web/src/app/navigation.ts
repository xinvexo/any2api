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

export interface NavigationItem {
  label: string;
  path: string;
  icon: LucideIcon;
}

export const navigationItems: NavigationItem[] = [
  { label: "总览", path: "/", icon: Gauge },
  { label: "代理", path: "/proxies", icon: Network },
  { label: "Provider", path: "/providers", icon: Server },
  { label: "模型路由", path: "/routes", icon: Route },
  { label: "负载均衡", path: "/balancing", icon: Activity },
  { label: "会话粘性", path: "/affinity", icon: Waypoints },
  { label: "网关密钥", path: "/keys", icon: KeyRound },
  { label: "请求日志", path: "/logs", icon: ScrollText },
  { label: "设置", path: "/settings", icon: Settings },
];
