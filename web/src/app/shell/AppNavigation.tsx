import { NavLink, useLocation } from "react-router-dom";

import { isNavigationPathActive, navigationItems } from "@/app/navigation";
import { cn } from "@/shared/lib/cn";

interface AppNavigationProps {
  collapsed?: boolean;
  onNavigate?: () => void;
}

export function AppNavigation({ collapsed = false, onNavigate }: AppNavigationProps) {
  const location = useLocation();

  return (
    <nav aria-label="主导航" className="grid gap-0.5">
      {navigationItems.map((item) => {
        const { icon: Icon, label, path } = item;
        const active = isNavigationPathActive(location.pathname, path);
        return (
          <NavLink
            key={path}
            to={path}
            end={path === "/"}
            title={collapsed ? label : undefined}
            onClick={onNavigate}
            className={({ isActive }) =>
              cn(
                "focus-ring flex h-9 items-center rounded-[10px] text-[13px] font-medium tracking-tight transition-colors",
                collapsed ? "justify-center px-0" : "gap-2.5 px-3",
                "text-secondary hover:bg-surface-hover hover:text-primary",
                isActive
                  && "bg-nav-active text-nav-active-fg hover:bg-nav-active hover:text-nav-active-fg",
              )
            }
          >
            <Icon size={16} strokeWidth={active ? 2.1 : 1.85} aria-hidden="true" />
            {collapsed ? <span className="sr-only">{label}</span> : <span>{label}</span>}
          </NavLink>
        );
      })}
    </nav>
  );
}
