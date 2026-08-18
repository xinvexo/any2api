import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { Button } from "@/shared/ui/Button";
import { IconButton } from "@/shared/ui/IconButton";
import { RowActionButton } from "@/shared/ui/RowActionButton";

test("button defaults to a non-submitting type", () => {
  render(<Button>保存</Button>);

  expect(screen.getByRole("button", { name: "保存" })).toHaveAttribute("type", "button");
});

test("uses one neutral command family without accent-blue fills", () => {
  render(
    <>
      <Button variant="primary">主操作</Button>
      <Button variant="secondary">次操作</Button>
      <Button variant="ghost">轻操作</Button>
      <Button variant="danger">危险操作</Button>
    </>,
  );

  const primary = screen.getByRole("button", { name: "主操作" });
  expect(primary).toHaveClass("bg-control-strong", "text-on-control-strong");
  expect(primary).not.toHaveClass("bg-accent", "ui-btn-fill");

  expect(screen.getByRole("button", { name: "次操作" })).toHaveClass(
    "bg-control",
    "text-primary",
  );
  expect(screen.getByRole("button", { name: "轻操作" })).toHaveClass(
    "bg-control",
    "text-primary",
  );
  expect(screen.getByRole("button", { name: "危险操作" })).toHaveClass(
    "bg-danger/10",
    "text-danger",
  );
});

test("keeps compact icon and row actions in the same interaction palette", () => {
  render(
    <>
      <IconButton label="刷新"><span>R</span></IconButton>
      <IconButton label="删除" tone="danger"><span>D</span></IconButton>
      <RowActionButton label="编辑"><span>E</span></RowActionButton>
      <RowActionButton label="停用" tone="danger"><span>S</span></RowActionButton>
    </>,
  );

  expect(screen.getByRole("button", { name: "刷新" })).toHaveClass(
    "bg-control",
    "active:bg-control-active",
  );
  expect(screen.getByRole("button", { name: "删除" })).toHaveClass(
    "bg-danger/10",
    "active:bg-danger/18",
  );
  expect(screen.getByRole("button", { name: "编辑" })).toHaveClass(
    "bg-transparent",
    "text-primary",
    "hover:bg-control-hover",
    "active:bg-control-active",
  );
  expect(screen.getByRole("button", { name: "停用" })).toHaveClass(
    "bg-transparent",
    "text-danger",
    "hover:bg-danger/10",
    "active:bg-danger/14",
  );
});
