import { act, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

vi.mock("@/features/admin-auth", () => ({
  AdminPasswordDrawer: () => null,
  AdminSecurityBanner: () => null,
  useAdminAuth: () => ({ submitting: false, logout: vi.fn() }),
}));
vi.mock("@/app/theme/ThemeSelector", () => ({ ThemeSelector: () => null }));
vi.mock("@/app/theme/useThemeMode", () => ({
  useThemeMode: () => ["light", vi.fn()],
}));

import { AppShell } from "@/app/shell/AppShell";

import { ConfirmDialog } from "./ConfirmDialog";
import { SideDrawer } from "./SideDrawer";

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  document.body.style.overflow = "";
  document.body.style.paddingRight = "";
});

test("keeps body locked until interleaved drawer and dialog lifecycles both finish", () => {
  vi.useFakeTimers();
  vi.spyOn(window, "innerWidth", "get").mockReturnValue(1_200);
  vi.spyOn(document.documentElement, "clientWidth", "get").mockReturnValue(1_180);
  document.body.style.overflow = "auto";
  document.body.style.paddingRight = "6px";

  const rendered = render(<Overlays drawerOpen dialogOpen />);
  expect(document.body.style.overflow).toBe("hidden");
  expect(document.body.style.paddingRight).toBe("20px");

  rendered.rerender(<Overlays drawerOpen={false} dialogOpen />);
  act(() => vi.advanceTimersByTime(250));
  expect(document.body.style.overflow).toBe("hidden");
  expect(document.body.style.paddingRight).toBe("20px");

  rendered.rerender(<Overlays drawerOpen={false} dialogOpen={false} />);
  act(() => vi.advanceTimersByTime(250));
  expect(document.body.style.overflow).toBe("auto");
  expect(document.body.style.paddingRight).toBe("6px");
});

test("mobile navigation cannot unlock the body while a dialog remains mounted", () => {
  vi.useFakeTimers();
  document.body.style.overflow = "auto";
  const rendered = render(<ShellWithDialog dialogOpen={false} />);

  fireEvent.click(screen.getByRole("button", { name: "打开导航" }));
  expect(document.body.style.overflow).toBe("hidden");

  rendered.rerender(<ShellWithDialog dialogOpen />);
  act(() => vi.advanceTimersByTime(20));
  expect(screen.getByRole("alertdialog", { name: "Confirm" })).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "关闭导航" }));
  expect(document.body.style.overflow).toBe("hidden");

  rendered.rerender(<ShellWithDialog dialogOpen={false} />);
  act(() => vi.advanceTimersByTime(200));
  expect(document.body.style.overflow).toBe("auto");
});

function Overlays({
  drawerOpen,
  dialogOpen,
}: {
  drawerOpen: boolean;
  dialogOpen: boolean;
}) {
  return (
    <>
      <SideDrawer
        open={drawerOpen}
        title="Editor"
        onClose={() => undefined}
      >
        Drawer content
      </SideDrawer>
      <ConfirmDialog
        open={dialogOpen}
        title="Confirm"
        onConfirm={() => undefined}
        onClose={() => undefined}
      />
    </>
  );
}

function ShellWithDialog({ dialogOpen }: { dialogOpen: boolean }) {
  return (
    <MemoryRouter>
      <AppShell />
      <ConfirmDialog
        open={dialogOpen}
        title="Confirm"
        onConfirm={() => undefined}
        onClose={() => undefined}
      />
    </MemoryRouter>
  );
}
