import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { FloatingPopover } from "./FloatingPopover";

test("escapes a short clamp so the bubble is not crushed into the row", () => {
  // Simulate a compact credential/OAuth usage row (~28px tall). Without
  // expanding tight bounds, the tip would be forced inside that strip.
  const shortRow = new DOMRect(40, 200, 280, 28);
  const original = HTMLElement.prototype.getBoundingClientRect;
  HTMLElement.prototype.getBoundingClientRect = function getBoundingClientRect() {
    if (this.getAttribute("role") === "tooltip") {
      return new DOMRect(0, 0, 120, 44);
    }
    return original.call(this);
  };

  try {
    render(
      <FloatingPopover open anchor={{ x: 260, y: 214 }} bounds={shortRow}>
        <p className="whitespace-nowrap">11:20–11:22</p>
        <p className="whitespace-nowrap">成功 0 · 失败 0</p>
      </FloatingPopover>,
    );

    const tip = screen.getByRole("tooltip");
    const top = Number.parseFloat(tip.style.top);
    expect(Number.isFinite(top)).toBe(true);
    // Prefer sitting above the anchor; must not land inside the short row.
    expect(top).toBeLessThan(shortRow.top);
    expect(top + 44).toBeLessThanOrEqual(214);
  } finally {
    HTMLElement.prototype.getBoundingClientRect = original;
  }
});
