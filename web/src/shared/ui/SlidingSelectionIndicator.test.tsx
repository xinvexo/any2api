import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, expect, test, vi } from "vitest";

import { SlidingSelectionIndicator } from "./SlidingSelectionIndicator";

afterEach(() => vi.restoreAllMocks());

test("measures and slides to variable-size selected items", () => {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
    this: HTMLElement,
  ) {
    const value = this.dataset.slidingSelectionItem;
    if (value === "short") {
      return rect(18, 24, 54, 28);
    }
    if (value === "long") {
      return rect(80, 24, 112, 28);
    }
    return rect(10, 20, 240, 36);
  });

  const { container } = render(<Harness />);
  const indicator = container.querySelector<HTMLElement>("[data-sliding-selection-indicator]");
  expect(indicator).toHaveStyle({
    width: "54px",
    height: "28px",
    transform: "translate3d(8px, 4px, 0)",
  });

  fireEvent.click(screen.getByRole("button", { name: "Long label" }));

  expect(indicator).toHaveAttribute("data-active-value", "long");
  expect(indicator).toHaveStyle({
    width: "112px",
    transform: "translate3d(70px, 4px, 0)",
  });
  expect(indicator).toHaveClass("duration-300");
});

function Harness() {
  const [selected, setSelected] = useState("short");
  return (
    <div className="relative" role="group" aria-label="Example">
      <SlidingSelectionIndicator selected={selected} />
      <button data-sliding-selection-item="short" onClick={() => setSelected("short")}>Short</button>
      <button data-sliding-selection-item="long" onClick={() => setSelected("long")}>Long label</button>
    </div>
  );
}

function rect(left: number, top: number, width: number, height: number): DOMRect {
  return {
    x: left,
    y: top,
    left,
    top,
    right: left + width,
    bottom: top + height,
    width,
    height,
    toJSON: () => ({}),
  } as DOMRect;
}
