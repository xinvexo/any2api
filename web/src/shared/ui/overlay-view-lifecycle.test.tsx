import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

import { ConfirmDialog } from "./ConfirmDialog";
import { SideDrawer } from "./SideDrawer";

let animationFrames: FrameRequestCallback[];
let requestAnimationFrameMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  vi.useFakeTimers();
  animationFrames = [];
  requestAnimationFrameMock = vi.fn((callback: FrameRequestCallback) => {
    animationFrames.push(callback);
    return animationFrames.length;
  });
  vi.stubGlobal("requestAnimationFrame", requestAnimationFrameMock);
  vi.stubGlobal("cancelAnimationFrame", vi.fn());
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  document.body.style.overflow = "";
  document.body.style.paddingRight = "";
});

test("drawer child updates do not enqueue view snapshots", () => {
  const rendered = render(
    <SideDrawer open title="Editor" onClose={() => undefined}>
      <span>first editor</span>
    </SideDrawer>,
  );
  flushAnimationFrames();
  requestAnimationFrameMock.mockClear();

  rendered.rerender(
    <SideDrawer open title="Editor" onClose={() => undefined}>
      <span>second editor</span>
    </SideDrawer>,
  );

  expect(screen.getByText("second editor")).toBeInTheDocument();
  expect(requestAnimationFrameMock).not.toHaveBeenCalled();

  rendered.rerender(
    <SideDrawer open={false} title="" onClose={() => undefined}>
      <span>closed editor</span>
    </SideDrawer>,
  );
  expect(screen.getByRole("dialog", { name: "Editor" })).toBeInTheDocument();
  expect(screen.queryByText("second editor")).not.toBeInTheDocument();
  expect(screen.queryByText("closed editor")).not.toBeInTheDocument();

  act(() => vi.advanceTimersByTime(200));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  rendered.unmount();
});

test("structured dialog descriptions update live without entering closing state", () => {
  const rendered = render(
    <ConfirmDialog
      open
      title="Confirm"
      description={<span>first description</span>}
      onConfirm={() => undefined}
      onClose={() => undefined}
    />,
  );
  flushAnimationFrames();
  requestAnimationFrameMock.mockClear();

  rendered.rerender(
    <ConfirmDialog
      open
      title="Confirm"
      description={<span>second description</span>}
      onConfirm={() => undefined}
      onClose={() => undefined}
    />,
  );

  expect(screen.getByText("second description")).toBeInTheDocument();
  expect(requestAnimationFrameMock).not.toHaveBeenCalled();

  rendered.rerender(
    <ConfirmDialog
      open={false}
      title=""
      onConfirm={() => undefined}
      onClose={() => undefined}
    />,
  );
  expect(screen.getByRole("alertdialog", { name: "Confirm" })).toBeInTheDocument();
  expect(screen.queryByText("second description")).not.toBeInTheDocument();

  act(() => vi.advanceTimersByTime(160));
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  rendered.unmount();
});

test("plain dialog descriptions remain visible through the exit animation", () => {
  const rendered = render(
    <ConfirmDialog
      open
      title="Delete"
      description="This cannot be undone."
      confirmLabel="Delete now"
      onConfirm={() => undefined}
      onClose={() => undefined}
    />,
  );
  flushAnimationFrames();

  rendered.rerender(
    <ConfirmDialog
      open={false}
      title=""
      onConfirm={() => undefined}
      onClose={() => undefined}
    />,
  );

  expect(screen.getByRole("alertdialog", { name: "Delete" })).toHaveAccessibleDescription(
    "This cannot be undone.",
  );
  expect(screen.getByRole("button", { name: "Delete now" })).toBeInTheDocument();

  act(() => vi.advanceTimersByTime(160));
  rendered.unmount();
});

function flushAnimationFrames() {
  act(() => {
    for (const callback of animationFrames.splice(0)) {
      callback(0);
    }
  });
}
