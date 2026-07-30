import { NavLink, useLocation } from "react-router-dom";

import { isNavigationPathActive, navigationItems } from "@/app/navigation";
import { cn } from "@/shared/lib/cn";
import { SlidingSelectionIndicator } from "@/shared/ui/SlidingSelectionIndicator";

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
  const selectedPath = navigationItems.find((item) =>
    isNavigationPathActive(location.pathname, item.path))?.path ?? "";

  return (
    <nav
      aria-label="主导航"
      className={cn("relative isolate grid", mobile ? "gap-1" : "gap-0.5")}
    >
      <SlidingSelectionIndicator
        selected={selectedPath}
        className={cn("rounded-[10px]", mobile ? "bg-accent/10" : "bg-nav-active")}
      />
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
            data-sliding-selection-item={path}
            className={cn(
              "group focus-ring relative z-10 flex items-center font-medium transition-colors",
              mobile
                ? "h-10 gap-3 rounded-[8px] px-3 text-sm"
                : "h-9 rounded-[10px] text-[13px]",
              !mobile && (collapsed ? "px-4" : "gap-2.5 pl-4 pr-3"),
              active
                ? mobile ? "text-accent" : "text-nav-active-fg"
                : mobile
                  ? "text-primary hover:text-accent"
                  : "text-secondary hover:text-primary",
            )}
          >
            <Icon
              className="shrink-0"
              size={mobile ? 17 : 16}
              strokeWidth={1.9}
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
