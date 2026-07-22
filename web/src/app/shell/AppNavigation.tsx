import { ChevronRight } from "lucide-react";
import { useState } from "react";
import { NavLink, useLocation, useNavigate } from "react-router-dom";

import {
  isNavigationPathActive,
  navigationItems,
  type NavigationItem,
} from "@/app/navigation";
import { cn } from "@/shared/lib/cn";

interface AppNavigationProps {
  collapsed?: boolean;
  onNavigate?: () => void;
}

export function AppNavigation({ collapsed = false, onNavigate }: AppNavigationProps) {
  const location = useLocation();
  const navigate = useNavigate();
  // Explicit open/closed state for groups with children. Missing key = closed.
  const [openSections, setOpenSections] = useState<Record<string, boolean>>(() =>
    defaultOpenSections(location.pathname),
  );
  const [trackedPath, setTrackedPath] = useState(location.pathname);

  // Deep-link / first entry into a section: open that group. Other groups keep their
  // manual expand/collapse state (selecting Settings must not collapse Providers).
  if (trackedPath !== location.pathname) {
    const previousPath = trackedPath;
    setTrackedPath(location.pathname);
    setOpenSections((current) => {
      let changed = false;
      const next = { ...current };
      for (const item of navigationItems) {
        if (!item.children?.length) {
          continue;
        }
        const wasActive = isSectionActive(previousPath, item);
        const isActive = isSectionActive(location.pathname, item);
        if (isActive && !wasActive && !next[item.path]) {
          next[item.path] = true;
          changed = true;
        }
      }
      return changed ? next : current;
    });
  }

  function toggleSection(path: string) {
    setOpenSections((current) => {
      const open = Boolean(current[path]);
      if (open) {
        const next = { ...current };
        delete next[path];
        return next;
      }
      return { ...current, [path]: true };
    });
  }

  return (
    <nav aria-label="主导航" className="grid gap-0.5">
      {navigationItems.map((item) => {
        const { icon: Icon, label, path, children } = item;
        const hasChildren = Boolean(children?.length);
        const sectionActive = isSectionActive(location.pathname, item);
        const sectionOpen = Boolean(openSections[path]);
        const showChildren = hasChildren && !collapsed && sectionOpen;
        const parentHighlighted = sectionActive && !showChildren;

        if (!hasChildren) {
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
              <Icon size={16} strokeWidth={sectionActive ? 2.1 : 1.85} aria-hidden="true" />
              {collapsed ? <span className="sr-only">{label}</span> : <span>{label}</span>}
            </NavLink>
          );
        }

        const landingPath = children![0]?.path ?? path;

        return (
          <div key={path} className="grid gap-0.5">
            <button
              type="button"
              title={collapsed ? label : undefined}
              aria-label={
                collapsed
                  ? label
                  : sectionOpen
                    ? `收起${label}`
                    : `展开${label}`
              }
              aria-expanded={collapsed ? undefined : sectionOpen}
              onClick={() => {
                if (collapsed) {
                  // Icon-only mode has no room for children; go to the section landing page.
                  navigate(landingPath);
                  onNavigate?.();
                  return;
                }
                toggleSection(path);
              }}
              className={cn(
                "focus-ring flex h-9 w-full items-center rounded-[10px] text-[13px] font-medium tracking-tight transition-colors",
                collapsed ? "justify-center px-0" : "gap-2.5 px-3",
                parentHighlighted
                  ? "bg-nav-active text-nav-active-fg hover:bg-nav-active hover:text-nav-active-fg"
                  : "text-secondary hover:text-primary",
                sectionActive && showChildren && "text-primary",
              )}
            >
              <Icon size={16} strokeWidth={sectionActive ? 2.1 : 1.85} aria-hidden="true" />
              {collapsed ? (
                <span className="sr-only">{label}</span>
              ) : (
                <>
                  <span className="min-w-0 flex-1 truncate text-left">{label}</span>
                  <ChevronRight
                    size={14}
                    className={cn(
                      "shrink-0 bg-transparent text-tertiary transition-transform duration-150",
                      sectionOpen && "rotate-90",
                      parentHighlighted && "text-nav-active-fg/70",
                    )}
                    aria-hidden="true"
                  />
                </>
              )}
            </button>

            {showChildren ? (
              <div className="mb-0.5 grid gap-0.5 pl-7">
                {children!.map((child) => (
                  <NavLink
                    key={child.path}
                    to={child.path}
                    end
                    onClick={onNavigate}
                    className={({ isActive }) =>
                      cn(
                        "focus-ring flex h-8 items-center rounded-[9px] px-2.5 text-[12px] font-medium tracking-tight transition-colors",
                        "text-secondary hover:bg-surface-hover hover:text-primary",
                        isActive
                          && "bg-nav-active text-nav-active-fg hover:bg-nav-active hover:text-nav-active-fg",
                      )
                    }
                  >
                    {child.label}
                  </NavLink>
                ))}
              </div>
            ) : null}
          </div>
        );
      })}
    </nav>
  );
}

function defaultOpenSections(pathname: string) {
  const open: Record<string, boolean> = {};
  for (const item of navigationItems) {
    if (item.children?.length && isSectionActive(pathname, item)) {
      open[item.path] = true;
    }
  }
  return open;
}

function isSectionActive(pathname: string, item: NavigationItem) {
  if (!item.children?.length) {
    return isNavigationPathActive(pathname, item.path);
  }
  return item.children.some(
    (child) => pathname === child.path || pathname.startsWith(`${child.path}/`),
  );
}
