import { NavLink, useLocation } from "react-router-dom";

import { cn } from "@/shared/lib/cn";
import { SlidingSelectionIndicator } from "@/shared/ui/SlidingSelectionIndicator";

interface PageTabItem {
  label: string;
  path: string;
  end?: boolean;
}

interface PageTabsProps {
  items: readonly PageTabItem[];
  ariaLabel: string;
}

export function PageTabs({ items, ariaLabel }: PageTabsProps) {
  const location = useLocation();
  const selectedPath = items.find((item) => tabIsActive(location.pathname, item))?.path ?? "";

  return (
    <nav
      aria-label={ariaLabel}
      className="relative isolate -mx-1 flex gap-1 overflow-x-auto px-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
    >
      <SlidingSelectionIndicator
        selected={selectedPath}
        className="rounded-full bg-nav-active"
      />
      {items.map((item) => (
        <NavLink
          key={item.path}
          to={item.path}
          end={item.end ?? true}
          data-sliding-selection-item={item.path}
          className={cn(
            "focus-ring relative z-10 shrink-0 rounded-full px-3 py-1.5 text-[13px] font-medium tracking-tight transition-colors",
            tabIsActive(location.pathname, item)
              ? "text-nav-active-fg"
              : "text-secondary hover:text-primary",
          )}
        >
          {item.label}
        </NavLink>
      ))}
    </nav>
  );
}

function tabIsActive(pathname: string, item: PageTabItem) {
  if (item.end ?? true) {
    return pathname === item.path;
  }
  return pathname === item.path || pathname.startsWith(`${item.path}/`);
}
