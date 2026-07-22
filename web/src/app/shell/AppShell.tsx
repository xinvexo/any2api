import { LogOut, Menu, Network, PanelLeftClose, PanelLeftOpen, X } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";

import { navigationItems } from "@/app/navigation";
import { ThemeSelector } from "@/app/theme/ThemeSelector";
import { useThemeMode } from "@/app/theme/useThemeMode";
import { AdminSecurityBanner, useAdminAuth } from "@/features/admin-auth";
import { cn } from "@/shared/lib/cn";

const SIDEBAR_EXPANDED = "w-[256px]";
const SIDEBAR_COLLAPSED = "w-[72px]";
const SIDEBAR_STORAGE_KEY = "any2api.sidebar-collapsed";

export function AppShell() {
  const [mobileOpen, setMobileOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(readSidebarCollapsed);
  const [themeMode, setThemeMode] = useThemeMode();
  const adminAuth = useAdminAuth();
  const location = useLocation();
  const mainRef = useRef<HTMLElement>(null);
  const previousPath = useRef(location.pathname);
  const titleId = useId();
  const pageTitle = getPageTitle(location.pathname);

  useEffect(() => {
    document.title = pageTitle === "总览" ? "any2api" : `${pageTitle} · any2api`;
    if (previousPath.current !== location.pathname) {
      previousPath.current = location.pathname;
      setMobileOpen(false);
      mainRef.current?.focus();
    }
  }, [location.pathname, pageTitle]);

  useEffect(() => {
    window.localStorage.setItem(SIDEBAR_STORAGE_KEY, collapsed ? "1" : "0");
  }, [collapsed]);

  useEffect(() => {
    if (!mobileOpen) {
      return;
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setMobileOpen(false);
      }
    };

    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    window.addEventListener("keydown", onKeyDown);

    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [mobileOpen]);

  return (
    <div className="flex h-dvh flex-col overflow-hidden bg-canvas text-primary">
      <a
        href="#main-content"
        className="focus-ring fixed left-4 top-3 z-50 -translate-y-[calc(100%+1rem)] whitespace-nowrap rounded-full bg-accent px-3 py-2 text-sm font-semibold text-on-accent focus:translate-y-0"
      >
        跳到主要内容
      </a>

      {/* Chrome: header + sidebar share the same outer surface */}
      <header className="z-30 shrink-0">
        <div className="flex h-14 items-center gap-2 px-3 sm:h-16 sm:gap-3 sm:px-4">
          <button
            type="button"
            className="focus-ring grid size-10 shrink-0 place-items-center rounded-full text-secondary transition-colors hover:bg-surface-hover hover:text-primary lg:hidden"
            aria-label={mobileOpen ? "关闭导航" : "打开导航"}
            aria-expanded={mobileOpen}
            aria-controls="responsive-navigation"
            onClick={() => setMobileOpen((open) => !open)}
          >
            {mobileOpen ? <X size={20} aria-hidden="true" /> : <Menu size={20} aria-hidden="true" />}
          </button>

          <button
            type="button"
            className="focus-ring hidden size-10 shrink-0 place-items-center rounded-full text-secondary transition-colors hover:bg-surface-hover hover:text-primary lg:grid"
            aria-label={collapsed ? "展开侧栏" : "收起侧栏"}
            aria-expanded={!collapsed}
            aria-controls="desktop-navigation"
            title={collapsed ? "展开侧栏" : "收起侧栏"}
            onClick={() => setCollapsed((value) => !value)}
          >
            {collapsed ? (
              <PanelLeftOpen size={20} aria-hidden="true" />
            ) : (
              <PanelLeftClose size={20} aria-hidden="true" />
            )}
          </button>

          <Brand onNavigate={() => setMobileOpen(false)} />

          <div className="ml-auto flex shrink-0 items-center gap-2 sm:gap-3">
            <ThemeSelector mode={themeMode} onModeChange={setThemeMode} compact />
            <LogoutButton
              pending={adminAuth.submitting}
              onLogout={() => void adminAuth.logout()}
            />
          </div>
        </div>
      </header>

      {mobileOpen ? (
        <div className="fixed inset-0 top-14 z-40 sm:top-16 lg:hidden" role="presentation">
          <button
            type="button"
            className="absolute inset-0 bg-scrim"
            aria-label="关闭导航遮罩"
            onClick={() => setMobileOpen(false)}
          />
          <aside
            id="responsive-navigation"
            className={cn(
              "absolute inset-y-0 left-0 flex h-full max-w-[86vw] flex-col bg-canvas shadow-panel",
              SIDEBAR_EXPANDED,
            )}
            aria-labelledby={titleId}
          >
            <div className="flex h-14 items-center justify-between gap-3 px-4">
              <span id={titleId} className="text-sm font-semibold tracking-tight text-primary">
                导航
              </span>
              <button
                type="button"
                className="focus-ring grid size-10 place-items-center rounded-full text-secondary transition-colors hover:bg-surface-hover hover:text-primary"
                aria-label="关闭导航"
                onClick={() => setMobileOpen(false)}
              >
                <X size={18} aria-hidden="true" />
              </button>
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto px-3 pb-4">
              <Navigation onNavigate={() => setMobileOpen(false)} />
            </div>
          </aside>
        </div>
      ) : null}

      <div className="flex min-h-0 flex-1">
        <aside
          id="desktop-navigation"
          className={cn(
            "hidden h-full shrink-0 flex-col overflow-hidden transition-[width] duration-200 ease-out lg:flex",
            collapsed ? SIDEBAR_COLLAPSED : SIDEBAR_EXPANDED,
          )}
          aria-label="应用侧栏"
        >
          <div className={cn("min-h-0 flex-1 overflow-y-auto pb-4", collapsed ? "px-2" : "px-3")}>
            <Navigation collapsed={collapsed} />
          </div>
        </aside>

        <div className="flex min-w-0 flex-1 flex-col gap-3 px-2 pb-2 sm:px-3 sm:pb-3">
          <AdminSecurityBanner />
          <p className="sr-only" aria-live="polite">
            当前页面：{pageTitle}
          </p>
          <main
            id="main-content"
            ref={mainRef}
            tabIndex={-1}
            className="min-h-0 flex-1 overflow-y-auto rounded-panel bg-surface shadow-panel outline-none"
          >
            <div className="w-full px-5 py-6 sm:px-7 sm:py-8 lg:px-8 lg:py-9">
              <Outlet />
            </div>
          </main>
        </div>
      </div>
    </div>
  );
}

function LogoutButton({
  pending,
  onLogout,
}: {
  pending: boolean;
  onLogout: () => void;
}) {
  return (
    <button
      type="button"
      className="focus-ring grid size-10 place-items-center rounded-full text-secondary transition-colors hover:bg-surface-hover hover:text-primary disabled:opacity-50"
      disabled={pending}
      aria-label="退出"
      title="退出"
      onClick={onLogout}
    >
      <LogOut size={18} aria-hidden="true" />
    </button>
  );
}

function Brand({ onNavigate }: { onNavigate: () => void }) {
  return (
    <NavLink
      to="/"
      onClick={onNavigate}
      className="focus-ring flex min-w-0 items-center gap-2.5 rounded-full sm:gap-3"
      aria-label="any2api 总览"
    >
      <span className="grid size-8 shrink-0 place-items-center rounded-[9px] bg-primary text-surface">
        <Network size={17} strokeWidth={2.2} aria-hidden="true" />
      </span>
      <span className="truncate text-[18px] font-medium tracking-tight sm:text-[20px]">any2api</span>
    </NavLink>
  );
}

function Navigation({
  collapsed = false,
  onNavigate,
}: {
  collapsed?: boolean;
  onNavigate?: () => void;
}) {
  return (
    <nav aria-label="主导航" className="grid gap-0.5">
      {navigationItems.map(({ icon: Icon, label, path }) => (
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
              isActive && "bg-nav-active text-nav-active-fg hover:bg-nav-active hover:text-nav-active-fg",
            )
          }
        >
          {({ isActive }) => (
            <>
              <Icon size={16} strokeWidth={isActive ? 2.1 : 1.85} aria-hidden="true" />
              {collapsed ? <span className="sr-only">{label}</span> : <span>{label}</span>}
            </>
          )}
        </NavLink>
      ))}
    </nav>
  );
}

function getPageTitle(pathname: string) {
  return (
    navigationItems.find(
      (item) =>
        item.path === pathname ||
        (item.path !== "/" && pathname.startsWith(`${item.path}/`)),
    )?.label ?? "页面不存在"
  );
}

function readSidebarCollapsed() {
  if (typeof window === "undefined") {
    return false;
  }
  return window.localStorage.getItem(SIDEBAR_STORAGE_KEY) === "1";
}
