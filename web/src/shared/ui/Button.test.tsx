import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { Button } from "@/shared/ui/Button";

test("button defaults to a non-submitting type", () => {
  render(<Button>保存</Button>);

  expect(screen.getByRole("button", { name: "保存" })).toHaveAttribute("type", "button");
});

test("primary filled button uses white-on-color fill class", () => {
  render(
    <Button variant="primary" size="sm">
      新增
    </Button>,
  );

  const button = screen.getByRole("button", { name: "新增" });
  expect(button.className).toContain("ui-btn-fill");
  expect(button.className).toContain("bg-accent");
  expect(button.className).toContain("h-7");
  expect(button).toHaveAttribute("data-variant", "primary");
});

test("dangerSolid also forces white label fill class", () => {
  render(
    <Button variant="dangerSolid" size="sm">
      删除
    </Button>,
  );

  const button = screen.getByRole("button", { name: "删除" });
  expect(button.className).toContain("ui-btn-fill");
  expect(button.className).toContain("bg-danger");
});

test("ghost cancel stays quiet chrome", () => {
  render(<Button variant="ghost">取消</Button>);

  const button = screen.getByRole("button", { name: "取消" });
  expect(button.className).toContain("bg-transparent");
  expect(button.className).toContain("text-secondary");
  expect(button.className).not.toContain("ui-btn-fill");
});
