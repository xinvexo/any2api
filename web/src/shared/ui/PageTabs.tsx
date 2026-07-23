import { NavLink } from "react-router-dom";

import { cn } from "@/shared/lib/cn";

export interface PageTabItem {
  label: string;
  path: string;
  end?: boolean;
}

interface PageTabsProps {
  items: readonly PageTabItem[];
  ariaLabel: string;
}

export function PageTabs({ items, ariaLabel }: PageTabsProps) {
  return (
    <nav
      aria-label={ariaLabel}
      className="-mx-1 flex gap-1 overflow-x-auto px-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
    >
      {items.map((item) => (
        <NavLink
          key={item.path}
          to={item.path}
          end={item.end ?? true}
          className={({ isActive }) =>
            cn(
              "focus-ring shrink-0 rounded-full px-3 py-1.5 text-[13px] font-medium tracking-tight transition-colors",
              isActive
                ? "bg-nav-active text-nav-active-fg"
                : "text-secondary hover:bg-surface-muted hover:text-primary",
            )
          }
        >
          {item.label}
        </NavLink>
      ))}
    </nav>
  );
}
