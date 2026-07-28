import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test } from "vitest";

import { RequestUsageStats } from "./RequestUsageStats";

test("highlights a hovered usage slot without moving or scaling it", () => {
  render(
    <RequestUsageStats
      label="测试凭据"
      usage={{
        totalRequests: 20,
        successfulRequests: 19,
        failedRequests: 1,
        windowMinutes: 2,
        windowSlots: [
          {
            startedAtMs: 0,
            totalRequests: 20,
            successfulRequests: 19,
            failedRequests: 1,
          },
        ],
      }}
    />,
  );

  const timeline = screen.getByRole("group", { name: /测试凭据 近 1 小时/ });
  const slot = within(timeline).getByRole("button");

  fireEvent.mouseEnter(slot);

  expect(slot).toHaveClass("brightness-[1.08]", "saturate-[1.12]");
  expect(slot).not.toHaveClass("scale-y-125");
  expect(slot.className).not.toContain("transition-[transform");
});
