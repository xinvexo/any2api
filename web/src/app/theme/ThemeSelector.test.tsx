import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { expect, test } from "vitest";

import type { ThemeMode } from "./theme";
import { ThemeSelector } from "./ThemeSelector";

test("moves one selection background between theme buttons", () => {
  const { container } = render(<Harness />);
  const indicator = container.querySelector("[data-sliding-selection-indicator]");
  const light = screen.getByRole("button", { name: "浅色" });
  const dark = screen.getByRole("button", { name: "深色" });

  expect(indicator).toHaveAttribute("data-active-value", "light");
  expect(light.className).not.toContain("bg-");
  expect(dark.className).not.toContain("bg-");

  fireEvent.click(dark);

  expect(indicator).toHaveAttribute("data-active-value", "dark");
  expect(dark).toHaveAttribute("aria-pressed", "true");
});

function Harness() {
  const [mode, setMode] = useState<ThemeMode>("light");
  return <ThemeSelector mode={mode} onModeChange={setMode} compact />;
}
