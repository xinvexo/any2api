import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { Button } from "@/shared/ui/Button";

test("button defaults to a non-submitting type", () => {
  render(<Button>保存</Button>);

  expect(screen.getByRole("button", { name: "保存" })).toHaveAttribute("type", "button");
});
