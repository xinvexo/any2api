import { KeyRound, LogOut, Menu, X } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";

import { getPageTitle } from "@/app/navigation";
import { AppNavigation } from "@/app/shell/AppNavigation";
import { ThemeSelector } from "@/app/theme/ThemeSelector";
import { useThemeMode } from "@/app/theme/useThemeMode";
import {
  AdminPasswordDrawer,
  AdminSecurityBanner,
  useAdminAuth,
} from "@/features/admin-auth";
import { cn } from "@/shared/lib/cn";
import { AppBrandIcon } from "@/shared/ui/AppBrandIcon";
import { useBodyScrollLock } from "@/shared/ui/useBodyScrollLock";

const SIDEBAR_EXPANDED = "w-[256px]";
const SIDEBAR_COLLAPSED = "w-[72px]";
const SIDEBAR_STORAGE_KEY = "any2api.sidebar-collapsed";
/** Header icon controls: pill hover, not circular. */
const HEADER_ICON_BUTTON =
  "focus-ring inline-flex h-8 shrink-0 items-center justify-center rounded-full px-3 text-secondary transition-colors hover:bg-surface-hover hover:text-primary";

export function AppShell() {
  const [mobileOpen, setMobileOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(readSidebarCollapsed);
  const [passwordOpen, setPasswordOpen] = useState(false);
  const [themeMode, setThemeMode] = useThemeMode();
  const adminAuth = useAdminAuth();
  const location = useLocation();
  const mainRef = useRef<HTMLElement>(null);
  const previousPath = useRef(location.pathname);
  const titleId = useId();
  const pageTitle = getPageTitle(location.pathname);

  useEffect(() => {
    document.title = pageTitle === "系统总览" ? "any2api" : `${pageTitle} · any2api`;
    if (previousPath.current !== location.pathname) {
      previousPath.current = location.pathname;
      setMobileOpen(false);
      // Keep scroll position stable when switching management pages.
      mainRef.current?.focus({ preventScroll: true });
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

    window.addEventListener("keydown", onKeyDown);

    return () => window.removeEventListener("keydown", onKeyDown);
  }, [mobileOpen]);

  useBodyScrollLock(mobileOpen);

  return (
    <div className="app-shell flex min-h-dvh flex-col text-primary md:h-dvh md:overflow-hidden">
      <div className="app-shell-fx" aria-hidden="true">
        <div className="app-shell-fx-bloom absolute inset-0" />
        <div className="app-shell-fx-grid absolute inset-0" />
      </div>

      <a
        href="#main-content"
        className="focus-ring fixed left-4 top-3 z-50 -translate-y-[calc(100%+1rem)] whitespace-nowrap rounded-full bg-accent px-3 py-2 text-sm font-semibold text-on-accent focus:translate-y-0"
      >
        跳到主要内容
      </a>

      {/* Chrome: header + sidebar share the ambient glass canvas */}
      <header className="app-shell-header app-shell-layer z-30 shrink-0">
        <div className="flex h-14 items-center gap-2 px-3 sm:h-16 sm:gap-3 sm:px-4">
          <button
            type="button"
            className={cn(HEADER_ICON_BUTTON, "lg:hidden")}
            aria-label={mobileOpen ? "关闭导航" : "打开导航"}
            aria-expanded={mobileOpen}
            aria-controls="responsive-navigation"
            onClick={() => setMobileOpen((open) => !open)}
          >
            {mobileOpen ? <X size={18} aria-hidden="true" /> : <Menu size={18} aria-hidden="true" />}
          </button>

          <button
            type="button"
            className={cn(HEADER_ICON_BUTTON, "hidden lg:inline-flex")}
            aria-label={collapsed ? "展开侧栏" : "收起侧栏"}
            aria-expanded={!collapsed}
            aria-controls="desktop-navigation"
            title={collapsed ? "展开侧栏" : "收起侧栏"}
            onClick={() => setCollapsed((value) => !value)}
          >
            <Menu size={18} aria-hidden="true" />
          </button>

          <Brand onNavigate={() => setMobileOpen(false)} />

          <div className="ml-auto flex shrink-0 items-center gap-1 sm:gap-1.5">
            <ThemeSelector mode={themeMode} onModeChange={setThemeMode} compact />
            <button
              type="button"
              className={HEADER_ICON_BUTTON}
              aria-label="修改密码"
              title="修改密码"
              onClick={() => setPasswordOpen(true)}
            >
              <KeyRound size={17} aria-hidden="true" />
            </button>
            <LogoutButton
              pending={adminAuth.submitting}
              onLogout={() => void adminAuth.logout()}
            />
          </div>
        </div>
      </header>

      <AdminPasswordDrawer open={passwordOpen} onClose={() => setPasswordOpen(false)} />

      {mobileOpen ? (
        <div
          className="app-shell-mobile-overlay fixed inset-x-0 bottom-0 z-40 lg:hidden"
          role="presentation"
        >
          <button
            type="button"
            className="mobile-navigation-scrim absolute inset-0"
            aria-label="关闭导航遮罩"
            onClick={() => setMobileOpen(false)}
          />
          <aside
            id="responsive-navigation"
            className="app-glass-panel mobile-navigation-panel absolute left-2 top-2 flex w-[272px] max-w-[calc(100vw-1rem)] flex-col overflow-hidden rounded-[12px]"
            aria-labelledby={titleId}
          >
            <span id={titleId} className="sr-only">
              导航
            </span>
            <div className="min-h-0 overflow-y-auto p-2">
              <AppNavigation variant="mobile" onNavigate={() => setMobileOpen(false)} />
            </div>
          </aside>
        </div>
      ) : null}

      <div className="app-shell-layer flex flex-1 md:min-h-0">
        <aside
          id="desktop-navigation"
          className={cn(
            "hidden h-full shrink-0 flex-col overflow-hidden transition-[width] duration-200 ease-out lg:flex",
            collapsed ? SIDEBAR_COLLAPSED : SIDEBAR_EXPANDED,
          )}
          aria-label="应用侧栏"
        >
          <div className="min-h-0 flex-1 overflow-y-auto px-3 pb-4 pt-2 sm:pt-2.5">
            <AppNavigation collapsed={collapsed} />
          </div>
        </aside>

        <div className="app-shell-content flex min-w-0 flex-1 flex-col gap-2 p-2 sm:p-2.5 md:min-h-0">
          <AdminSecurityBanner />
          <p className="sr-only" aria-live="polite">
            当前页面：{pageTitle}
          </p>
          <main
            id="main-content"
            ref={mainRef}
            tabIndex={-1}
            className="app-glass-panel flex-1 overflow-visible rounded-panel outline-none md:min-h-0 md:overflow-y-scroll md:[scrollbar-gutter:stable_both-edges]"
          >
            {/* Mobile lets the document own vertical scrolling so iOS Safari can collapse its
                browser chrome. Desktop keeps the bounded management workspace. */}
            <div className="flex min-h-0 w-full flex-col p-4 md:h-full">
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
      className={cn(HEADER_ICON_BUTTON, "disabled:opacity-50")}
      disabled={pending}
      aria-label="退出"
      title="退出"
      onClick={onLogout}
    >
      <LogOut size={17} aria-hidden="true" />
    </button>
  );
}

function Brand({ onNavigate }: { onNavigate: () => void }) {
  return (
    <NavLink
      to="/"
      onClick={onNavigate}
      className="focus-ring flex min-w-0 items-center gap-2.5 rounded-full sm:gap-3"
      aria-label="any2api 系统总览"
    >
      <AppBrandIcon className="size-8 shrink-0 select-none rounded-[8px]" />
      <span className="truncate text-[18px] font-medium tracking-tight sm:text-[20px]">any2api</span>
    </NavLink>
  );
}

function readSidebarCollapsed() {
  if (typeof window === "undefined") {
    return false;
  }
  return window.localStorage.getItem(SIDEBAR_STORAGE_KEY) === "1";
}
