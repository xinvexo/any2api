import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { RequestModeBadges } from "./RequestModeBadges";

test("shows confirmed request modes", () => {
  render(
    <RequestModeBadges
      isStream
      requestedSpeedTier="fast"
      effectiveSpeedTier="fast"
    />,
  );

  expect(screen.getByLabelText("请求模式：流式")).toHaveTextContent("流");
  expect(screen.getByLabelText("Fast 模式")).toHaveTextContent("Fast");
});

test("suppresses requested Fast when the effective tier is standard", () => {
  render(
    <RequestModeBadges
      isStream={false}
      requestedSpeedTier="fast"
      effectiveSpeedTier="standard"
    />,
  );

  expect(screen.getByLabelText("请求模式：非流式")).toHaveTextContent("非流");
  expect(screen.queryByText("Fast")).not.toBeInTheDocument();
});

test("shows requested Fast as unconfirmed while omitting an unknown stream mode", () => {
  render(
    <RequestModeBadges
      isStream={null}
      requestedSpeedTier="fast"
      effectiveSpeedTier={null}
    />,
  );

  expect(screen.queryByText("流")).not.toBeInTheDocument();
  expect(screen.queryByText("非流")).not.toBeInTheDocument();
  expect(screen.getByLabelText("请求 Fast，上游尚未确认")).toHaveTextContent("Fast");
});
