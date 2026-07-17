import { Menu, Network, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { NavLink, Outlet, useLocation } from "react-router-dom";

import { navigationItems } from "@/app/navigation";
import { ThemeSelector } from "@/app/theme/ThemeSelector";
import { useThemeMode } from "@/app/theme/useThemeMode";
import { cn } from "@/shared/lib/cn";

export function AppShell() {
  const [mobileOpen, setMobileOpen] = useState(false);
  const [themeMode, setThemeMode] = useThemeMode();
  const location = useLocation();
  const mainRef = useRef<HTMLElement>(null);
  const previousPath = useRef(location.pathname);
  const pageTitle = getPageTitle(location.pathname);

  useEffect(() => {
    document.title = pageTitle === "总览" ? "any2api" : `${pageTitle} · any2api`;
    if (previousPath.current !== location.pathname) {
      previousPath.current = location.pathname;
      mainRef.current?.focus();
    }
  }, [location.pathname, pageTitle]);

  return (
    <div className="min-h-dvh bg-canvas text-primary">
      <a
        href="#main-content"
        className="focus-ring fixed left-4 top-3 z-50 -translate-y-20 rounded-control bg-accent px-3 py-2 text-sm font-semibold text-on-accent focus:translate-y-0"
      >
        跳到主要内容
      </a>
      <header className="sticky top-0 z-40 border-b border-subtle bg-canvas/90 backdrop-blur-xl">
        <div className="mx-auto flex h-16 max-w-[1500px] items-center gap-5 px-4 sm:px-7">
          <Brand onNavigate={() => setMobileOpen(false)} />
          <div className="hidden min-w-0 flex-1 xl:block">
            <Navigation compact />
          </div>
          <div className="ml-auto hidden xl:block">
            <ThemeSelector mode={themeMode} onModeChange={setThemeMode} />
          </div>
          <button
            type="button"
            className="focus-ring ml-auto grid size-10 place-items-center rounded-control text-secondary hover:bg-surface-hover hover:text-primary xl:hidden"
            aria-label={mobileOpen ? "关闭导航" : "打开导航"}
            aria-expanded={mobileOpen}
            aria-controls="responsive-navigation"
            onClick={() => setMobileOpen((open) => !open)}
          >
            {mobileOpen ? <X size={20} /> : <Menu size={20} />}
          </button>
        </div>
      </header>

      {mobileOpen ? (
        <div
          id="responsive-navigation"
          className="fixed inset-x-0 top-16 z-30 max-h-[calc(100dvh-4rem)] overflow-y-auto border-b border-subtle bg-canvas px-4 py-4 shadow-panel xl:hidden"
        >
          <div className="mx-auto max-w-[1500px]">
            <Navigation onNavigate={() => setMobileOpen(false)} />
            <div className="mt-4 border-t border-subtle pt-4">
              <ThemeSelector mode={themeMode} onModeChange={setThemeMode} />
            </div>
          </div>
        </div>
      ) : null}

      <p className="sr-only" aria-live="polite">
        当前页面：{pageTitle}
      </p>
      <main
        id="main-content"
        ref={mainRef}
        tabIndex={-1}
        className="min-w-0 px-4 py-8 outline-none sm:px-7 sm:py-10 lg:px-10 lg:py-12"
      >
        <div className="mx-auto w-full max-w-6xl">
          <Outlet />
        </div>
      </main>
    </div>
  );
}

function Brand({ onNavigate }: { onNavigate: () => void }) {
  return (
    <NavLink to="/" onClick={onNavigate} className="focus-ring flex items-center gap-3 rounded-control" aria-label="any2api 总览">
      <span className="grid size-9 place-items-center rounded-control bg-accent text-on-accent shadow-accent">
        <Network size={19} strokeWidth={2.2} />
      </span>
      <span className="text-[17px] font-semibold">any2api</span>
    </NavLink>
  );
}

function Navigation({ compact = false, onNavigate }: { compact?: boolean; onNavigate?: () => void }) {
  return (
    <nav
      aria-label="主导航"
      className={compact ? "flex items-center justify-center gap-1" : "grid gap-1 sm:grid-cols-2"}
    >
      {navigationItems.map(({ icon: Icon, label, path }) => (
        <NavLink
          key={path}
          to={path}
          end={path === "/"}
          onClick={onNavigate}
          className={({ isActive }) =>
            cn(
              "focus-ring flex items-center rounded-control font-medium text-secondary transition-colors",
              compact ? "h-9 gap-2 px-2.5 text-[13px]" : "h-11 gap-3 px-3 text-sm",
              "hover:bg-surface-hover hover:text-primary",
              isActive && "bg-surface-selected text-primary shadow-hairline",
            )
          }
        >
          <Icon size={compact ? 15 : 18} aria-hidden="true" />
          <span>{label}</span>
        </NavLink>
      ))}
    </nav>
  );
}

function getPageTitle(pathname: string) {
  return navigationItems.find((item) => item.path === pathname)?.label ?? "页面不存在";
}
