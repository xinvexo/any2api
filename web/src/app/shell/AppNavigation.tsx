import { NavLink, useLocation } from "react-router-dom";

import { isNavigationPathActive, navigationItems } from "@/app/navigation";
import { cn } from "@/shared/lib/cn";

interface AppNavigationProps {
  collapsed?: boolean;
  onNavigate?: () => void;
  variant?: "desktop" | "mobile";
}

export function AppNavigation({
  collapsed = false,
  onNavigate,
  variant = "desktop",
}: AppNavigationProps) {
  const location = useLocation();
  const mobile = variant === "mobile";

  return (
    <nav aria-label="主导航" className={cn("grid", mobile ? "gap-1" : "gap-0.5")}>
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
                "focus-ring flex items-center font-medium transition-colors",
                mobile
                  ? "h-10 gap-3 rounded-[8px] px-3 text-sm"
                  : "h-9 rounded-[10px] text-[13px]",
                !mobile && (collapsed ? "px-4" : "gap-2.5 pl-4 pr-3"),
                mobile
                  ? "text-primary hover:bg-accent/10 hover:text-accent"
                  : "text-secondary hover:bg-surface-hover hover:text-primary",
                isActive &&
                  (mobile
                    ? "bg-accent/10 text-accent hover:bg-accent/15 hover:text-accent"
                    : "bg-nav-active text-nav-active-fg hover:bg-nav-active hover:text-nav-active-fg"),
              )
            }
          >
            <Icon
              className="shrink-0"
              size={mobile ? 17 : 16}
              strokeWidth={active ? 2.1 : 1.85}
              aria-hidden="true"
            />
            {collapsed ? (
              <span className="sr-only">{label}</span>
            ) : (
              <span className={cn("whitespace-nowrap", !mobile && "min-w-0 overflow-hidden")}>
                {label}
              </span>
            )}
          </NavLink>
        );
      })}
    </nav>
  );
}
