import { LogOut, Menu, Network, X } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";

import { navigationItems } from "@/app/navigation";
import { ThemeSelector } from "@/app/theme/ThemeSelector";
import { useThemeMode } from "@/app/theme/useThemeMode";
import { AdminSecurityBanner, useAdminAuth } from "@/features/admin-auth";
import { cn } from "@/shared/lib/cn";

const SIDEBAR_WIDTH = "w-[260px]";

export function AppShell() {
  const [mobileOpen, setMobileOpen] = useState(false);
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
    <div className="min-h-dvh bg-canvas text-primary lg:flex">
      <a
        href="#main-content"
        className="focus-ring fixed left-4 top-3 z-50 -translate-y-[calc(100%+1rem)] whitespace-nowrap rounded-control bg-accent px-3 py-2 text-sm font-semibold text-on-accent focus:translate-y-0"
      >
        跳到主要内容
      </a>

      {/* Desktop sidebar */}
      <aside
        className={cn(
          "sticky top-0 z-30 hidden h-dvh shrink-0 flex-col border-r border-subtle bg-sidebar lg:flex",
          SIDEBAR_WIDTH,
        )}
        aria-label="应用侧栏"
      >
        <SidebarChrome
          onNavigate={() => setMobileOpen(false)}
          themeMode={themeMode}
          onThemeModeChange={setThemeMode}
          logoutPending={adminAuth.submitting}
          onLogout={() => void adminAuth.logout()}
        />
      </aside>

      {/* Mobile drawer */}
      {mobileOpen ? (
        <div className="fixed inset-0 z-40 lg:hidden" role="presentation">
          <button
            type="button"
            className="absolute inset-0 bg-scrim backdrop-blur-[2px]"
            aria-label="关闭导航遮罩"
            onClick={() => setMobileOpen(false)}
          />
          <aside
            id="responsive-navigation"
            className={cn(
              "absolute inset-y-0 left-0 flex h-full max-w-[86vw] flex-col border-r border-subtle bg-sidebar shadow-panel",
              SIDEBAR_WIDTH,
            )}
            aria-labelledby={titleId}
          >
            <div className="flex h-14 items-center justify-between gap-3 border-b border-subtle px-4">
              <Brand id={titleId} onNavigate={() => setMobileOpen(false)} />
              <button
                type="button"
                className="focus-ring grid size-10 place-items-center rounded-control text-secondary hover:bg-surface-hover hover:text-primary"
                aria-label="关闭导航"
                onClick={() => setMobileOpen(false)}
              >
                <X size={18} aria-hidden="true" />
              </button>
            </div>
            <SidebarChrome
              showBrand={false}
              onNavigate={() => setMobileOpen(false)}
              themeMode={themeMode}
              onThemeModeChange={setThemeMode}
              logoutPending={adminAuth.submitting}
              onLogout={() => void adminAuth.logout()}
            />
          </aside>
        </div>
      ) : null}

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="sticky top-0 z-20 border-b border-subtle bg-canvas/85 backdrop-blur-xl lg:hidden">
          <div className="flex h-14 items-center gap-3 px-3 sm:px-4">
            <button
              type="button"
              className="focus-ring grid size-10 place-items-center rounded-control text-secondary hover:bg-surface-hover hover:text-primary"
              aria-label={mobileOpen ? "关闭导航" : "打开导航"}
              aria-expanded={mobileOpen}
              aria-controls="responsive-navigation"
              onClick={() => setMobileOpen((open) => !open)}
            >
              {mobileOpen ? <X size={20} aria-hidden="true" /> : <Menu size={20} aria-hidden="true" />}
            </button>
            <Brand onNavigate={() => setMobileOpen(false)} compact />
            <span className="ml-auto truncate text-sm font-medium text-secondary">{pageTitle}</span>
          </div>
        </header>

        <p className="sr-only" aria-live="polite">
          当前页面：{pageTitle}
        </p>
        <AdminSecurityBanner />
        <main
          id="main-content"
          ref={mainRef}
          tabIndex={-1}
          className="min-w-0 flex-1 px-4 py-7 outline-none sm:px-6 sm:py-9 lg:px-10 lg:py-10"
        >
          <div className="mx-auto w-full max-w-6xl">
            <Outlet />
          </div>
        </main>
      </div>
    </div>
  );
}

function SidebarChrome({
  showBrand = true,
  onNavigate,
  themeMode,
  onThemeModeChange,
  logoutPending,
  onLogout,
}: {
  showBrand?: boolean;
  onNavigate: () => void;
  themeMode: ReturnType<typeof useThemeMode>[0];
  onThemeModeChange: ReturnType<typeof useThemeMode>[1];
  logoutPending: boolean;
  onLogout: () => void;
}) {
  return (
    <>
      {showBrand ? (
        <div className="flex h-16 items-center px-4 pt-1">
          <Brand onNavigate={onNavigate} />
        </div>
      ) : null}

      <div className="min-h-0 flex-1 overflow-y-auto px-3 py-3">
        <Navigation onNavigate={onNavigate} />
      </div>

      <div className="mt-auto space-y-3 border-t border-subtle px-3 py-4">
        <ThemeSelector mode={themeMode} onModeChange={onThemeModeChange} />
        <LogoutButton pending={logoutPending} onLogout={onLogout} className="w-full" />
      </div>
    </>
  );
}

function LogoutButton({
  className,
  pending,
  onLogout,
}: {
  className?: string;
  pending: boolean;
  onLogout: () => void;
}) {
  return (
    <button
      type="button"
      className={cn(
        "focus-ring inline-flex h-10 items-center justify-center gap-2 rounded-control px-3 text-sm font-medium text-secondary transition-colors",
        "hover:bg-surface-hover hover:text-primary disabled:opacity-50",
        className,
      )}
      disabled={pending}
      onClick={onLogout}
    >
      <LogOut size={16} aria-hidden="true" />
      退出
    </button>
  );
}

function Brand({
  onNavigate,
  compact = false,
  id,
}: {
  onNavigate: () => void;
  compact?: boolean;
  id?: string;
}) {
  return (
    <NavLink
      to="/"
      id={id}
      onClick={onNavigate}
      className="focus-ring flex min-w-0 items-center gap-3 rounded-control"
      aria-label="any2api 总览"
    >
      <span
        className={cn(
          "grid shrink-0 place-items-center rounded-[9px] bg-accent text-on-accent shadow-accent",
          compact ? "size-8" : "size-9",
        )}
      >
        <Network size={compact ? 16 : 18} strokeWidth={2.2} aria-hidden="true" />
      </span>
      <span className={cn("truncate font-semibold tracking-tight", compact ? "text-[15px]" : "text-[17px]")}>
        any2api
      </span>
    </NavLink>
  );
}

function Navigation({ onNavigate }: { onNavigate?: () => void }) {
  return (
    <nav aria-label="主导航" className="grid gap-1">
      {navigationItems.map(({ icon: Icon, label, path }) => (
        <NavLink
          key={path}
          to={path}
          end={path === "/"}
          onClick={onNavigate}
          className={({ isActive }) =>
            cn(
              "focus-ring relative flex h-10 items-center gap-3 rounded-[10px] px-3 text-[13.5px] font-medium transition-colors",
              "text-secondary hover:bg-surface-hover hover:text-primary",
              isActive && "bg-nav-active text-nav-active-fg shadow-none",
            )
          }
        >
          {({ isActive }) => (
            <>
              <span
                className={cn(
                  "absolute left-0 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-full bg-accent transition-opacity",
                  isActive ? "opacity-100" : "opacity-0",
                )}
                aria-hidden="true"
              />
              <Icon size={17} strokeWidth={isActive ? 2.15 : 1.9} aria-hidden="true" />
              <span>{label}</span>
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
