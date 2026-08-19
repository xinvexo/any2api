import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { RequestModeBadges } from "./RequestModeBadges";

test("shows confirmed request modes", () => {
  render(
    <RequestModeBadges
      isStream
      requestedSpeedTier="fast"
    />,
  );

  expect(screen.getByLabelText("请求模式：流式")).toHaveTextContent("流");
  expect(screen.getByLabelText("Fast 模式")).toHaveTextContent("Fast");
});

test("shows requested Fast without considering the effective tier", () => {
  render(
    <RequestModeBadges
      isStream={false}
      requestedSpeedTier="fast"
    />,
  );

  expect(screen.getByLabelText("请求模式：非流式")).toHaveTextContent("非流");
  expect(screen.getByLabelText("Fast 模式")).toHaveTextContent("Fast");
});

test("shows requested Fast while omitting an unknown stream mode", () => {
  render(
    <RequestModeBadges
      isStream={null}
      requestedSpeedTier="fast"
    />,
  );

  expect(screen.queryByText("流")).not.toBeInTheDocument();
  expect(screen.queryByText("非流")).not.toBeInTheDocument();
  expect(screen.getByLabelText("Fast 模式")).toHaveTextContent("Fast");
});

test("omits Fast for standard requests", () => {
  render(<RequestModeBadges isStream requestedSpeedTier="standard" />);

  expect(screen.queryByText("Fast")).not.toBeInTheDocument();
});
