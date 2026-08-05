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
        failureScope: null,
        occurredAt: 1_900_000_000,
        reauthorizationRequired: true,
      }}
    />,
  );

  const notice = screen.getByRole("alert", { name: "Token 刷新失败" });
  expect(notice).toHaveTextContent("Access Token 401 后刷新");
  expect(notice).toHaveTextContent("Token Endpoint");
  expect(notice).toHaveTextContent("Refresh Token 已被重复使用（HTTP 400）");
  expect(notice).toHaveTextContent("请重新授权账号");
});
