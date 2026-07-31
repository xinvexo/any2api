import { describe, expect, it } from "vitest";

import { ApiError } from "@/shared/api/http-client";

import { getOAuthErrorMessage } from "./oauth-error";

describe("OAuth error messages", () => {
  it("keeps account restrictions separate from provider egress rejection", () => {
    expect(
      getOAuthErrorMessage(
        new ApiError(502, "oauth_account_restricted", "restricted"),
      ),
    ).toBe("上游已明确限制此账号访问。");
    expect(
      getOAuthErrorMessage(
        new ApiError(502, "oauth_provider_egress_restricted", "egress"),
      ),
    ).toBe("当前网络出口被上游拒绝，请检查或更换全局代理。");
  });
});
