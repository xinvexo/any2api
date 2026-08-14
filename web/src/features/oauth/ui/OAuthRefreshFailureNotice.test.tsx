import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { OAuthRefreshFailureNotice } from "./OAuthRefreshFailureNotice";

test("shows the refresh trigger stage reason status and required action separately", () => {
  render(
    <OAuthRefreshFailureNotice
      failure={{
        tokenVersion: 4,
        trigger: "authentication_failure",
        stage: "token_endpoint",
        reason: "refresh_token_reused",
        upstreamStatus: 400,
        failureScope: "egress_path",
        occurredAt: 1_900_000_000,
        reauthorizationRequired: true,
      }}
    />,
  );

  const notice = screen.getByRole("alert", { name: "Token 刷新失败" });
  expect(notice).not.toHaveClass("border-t", "border-danger/20", "rounded-lg", "bg-danger/5");
  expect(notice.querySelector("svg")).toBeNull();
  expect(notice).toHaveTextContent("Access Token 401 后刷新");
  expect(notice).toHaveTextContent("Token Endpoint");
  expect(notice).toHaveTextContent("Refresh Token 已被重复使用");
  expect(notice).toHaveTextContent("上游端点 × 出口路径");
  expect(notice).toHaveTextContent("请重新授权账号");
});
