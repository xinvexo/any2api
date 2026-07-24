import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { FloatingPopover } from "./FloatingPopover";

test("renders portal content when open with an anchor", () => {
  render(
    <FloatingPopover open anchor={{ x: 120, y: 80 }} bounds={new DOMRect(0, 0, 400, 300)}>
      <p>悬浮内容</p>
    </FloatingPopover>,
  );

  expect(screen.getByRole("tooltip")).toHaveTextContent("悬浮内容");
});

test("renders nothing when closed", () => {
  render(
    <FloatingPopover open={false} anchor={{ x: 10, y: 10 }}>
      <p>隐藏内容</p>
    </FloatingPopover>,
  );

  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  expect(screen.queryByText("隐藏内容")).not.toBeInTheDocument();
});
