import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { expect, test } from "vitest";

import { SlidingKindNav } from "./SlidingKindNav";
import { OpenAiIcon } from "@/shared/icons/brand-icons";

const options = [
  { value: "codex", label: "Codex", icon: OpenAiIcon },
  { value: "claude", label: "Claude", icon: OpenAiIcon },
  { value: "grok", label: "Grok", icon: OpenAiIcon },
] as const;

test("slides the active background to the selected item", () => {
  const { container } = render(<Harness />);
  const indicator = container.querySelector<HTMLElement>("[data-sliding-selection-indicator]");

  expect(indicator).toHaveAttribute("data-active-value", "codex");

  const grok = screen.getByRole("button", { name: /Grok/ });
  const grokCount = grok.lastElementChild;
  expect(grok.className).not.toContain("hover:bg-");
  expect(grok.querySelector("svg")).toHaveClass("group-hover:text-primary");
  expect(grokCount).toHaveClass("group-hover:text-secondary");
  expect(grokCount).toHaveClass("font-medium");

  fireEvent.click(grok);

  expect(grok).toHaveAttribute("aria-current", "page");
  expect(grok.className).not.toContain("hover:bg-");
  expect(grok.querySelector("svg")).not.toHaveClass("group-hover:text-primary");
  expect(grokCount).not.toHaveClass("group-hover:text-secondary");
  expect(grokCount).toHaveClass("font-medium");
  expect(indicator).toHaveAttribute("data-active-value", "grok");
  expect(indicator).toHaveClass("duration-300");
});

function Harness() {
  const [selected, setSelected] = useState<(typeof options)[number]["value"]>("codex");
  return (
    <SlidingKindNav
      ariaLabel="Provider 类型"
      selected={selected}
      options={options}
      counts={{ codex: 1, claude: 0, grok: 1 }}
      onSelect={setSelected}
    />
  );
}
