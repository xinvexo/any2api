import { act, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { clearNotifications, getNotifications, notify } from "@/shared/notifications";
import { NotificationHost } from "@/shared/notifications/NotificationHost";

afterEach(() => {
  clearNotifications();
  vi.useRealTimers();
});

test("renders a centered notification block with a countdown timer bar", () => {
  render(<NotificationHost />);

  act(() => {
    notify.success("已刷新全部 2 个 Codex 账号额度。");
  });

  const host = document.querySelector("[data-notification-host]");
  expect(host?.className).toContain("justify-center");
  expect(host?.className).not.toContain("justify-end");
  // Scroll containers flash macOS scrollbar gutters during enter animation.
  expect(screen.getByLabelText("全局通知").className).not.toContain("overflow-y-auto");

  const status = screen.getByRole("status");
  expect(status).toHaveTextContent("已刷新全部 2 个 Codex 账号额度。");
  expect(status.className).toContain("notification-card");
  expect(status.className).toContain("pointer-events-auto");
  expect(screen.getByLabelText("全局通知")).toBeInTheDocument();

  const timerBar = status.querySelector("[data-notification-timer-bar]");
  expect(timerBar).toBeInTheDocument();
  expect(timerBar).toHaveClass("notification-card__timer-bar");
  expect((timerBar as HTMLElement).style.animationDuration).toBe("3600ms");
});

test("auto-dismisses after the configured duration", () => {
  vi.useFakeTimers();
  render(<NotificationHost />);

  act(() => {
    notify.info("临时通知", 1_500);
  });
  expect(screen.getByText("临时通知")).toBeInTheDocument();

  act(() => {
    vi.advanceTimersByTime(1_500);
  });
  expect(screen.queryByText("临时通知")).not.toBeInTheDocument();
});

test("stacks multiple notifications at the same time", () => {
  vi.useFakeTimers();
  render(<NotificationHost />);

  act(() => {
    notify.success("第一条", 5_000);
    notify.warning("第二条", 5_000);
    notify.danger("第三条", 5_000);
  });

  expect(getNotifications()).toHaveLength(3);
  expect(screen.getByText("第一条")).toBeInTheDocument();
  expect(screen.getByText("第二条")).toBeInTheDocument();
  expect(screen.getByText("第三条")).toBeInTheDocument();

  const cards = screen.getByLabelText("全局通知").querySelectorAll(".notification-card");
  expect(cards).toHaveLength(3);
  // Newest is prepended to the stack.
  expect(cards[0]).toHaveTextContent("第三条");
  expect(cards[2]).toHaveTextContent("第一条");
});

test("dismisses stacked notifications independently", () => {
  vi.useFakeTimers();
  render(<NotificationHost />);

  act(() => {
    notify.info("短通知", 1_200);
    notify.success("长通知", 4_000);
  });
  expect(screen.getByText("短通知")).toBeInTheDocument();
  expect(screen.getByText("长通知")).toBeInTheDocument();

  act(() => {
    vi.advanceTimersByTime(1_200);
  });
  expect(screen.queryByText("短通知")).not.toBeInTheDocument();
  expect(screen.getByText("长通知")).toBeInTheDocument();

  act(() => {
    vi.advanceTimersByTime(2_800);
  });
  expect(screen.queryByText("长通知")).not.toBeInTheDocument();
});

test("keeps notifications when the page subtree remounts", () => {
  function Page({ label }: { label: string }) {
    return <p>{label}</p>;
  }

  const view = render(
    <>
      <NotificationHost />
      <Page label="oauth" />
    </>,
  );

  act(() => {
    notify.success("跨菜单通知");
  });
  expect(screen.getByText("跨菜单通知")).toBeInTheDocument();
  expect(getNotifications()).toHaveLength(1);

  // Simulate switching a management menu: only page content remounts.
  view.rerender(
    <>
      <NotificationHost />
      <Page label="providers" />
    </>,
  );

  expect(screen.getByText("providers")).toBeInTheDocument();
  expect(screen.getByText("跨菜单通知")).toBeInTheDocument();
  expect(getNotifications()).toHaveLength(1);
});

test("store survives host unmount and reappears after remount", () => {
  const first = render(<NotificationHost />);
  act(() => {
    notify.warning("仍在 store 中");
  });
  expect(screen.getByText("仍在 store 中")).toBeInTheDocument();

  first.unmount();
  expect(getNotifications()).toHaveLength(1);

  render(<NotificationHost />);
  expect(screen.getByText("仍在 store 中")).toBeInTheDocument();
});
