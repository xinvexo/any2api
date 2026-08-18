import { render } from "@testing-library/react";
import { expect, test } from "vitest";

import { AppBrandIcon } from "./AppBrandIcon";

test("renders the brand mark as a theme-aware vector", () => {
  const { container } = render(<AppBrandIcon className="size-8" />);
  const icon = container.querySelector("svg");

  expect(icon).toHaveAttribute("viewBox", "0 0 32 32");
  expect(icon).toHaveClass("text-control-strong", "size-8");
  expect(container.querySelector("img")).not.toBeInTheDocument();
});
