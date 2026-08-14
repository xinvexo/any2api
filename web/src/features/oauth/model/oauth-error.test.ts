import { describe, expect, it } from "vitest";

import { ApiError } from "@/shared/api/http-client";

import { getOAuthErrorMessage } from "./oauth-error";

describe("OAuth error messages", () => {
  it("explains how to resolve an ambiguous login identity", () => {
    expect(
      getOAuthErrorMessage(
        new ApiError(409, "oauth_account_identity_conflict", "conflict"),
      ),
    ).toBe("多个 OAuth 账号对应同一上游身份，请删除重复账号后重试。");
  });

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
    ).toBe("当前账号的 OAuth 出口被上游拒绝，请检查或更换该账号的出口代理。");
  });

  it("identifies an unavailable selected OAuth proxy", () => {
    expect(
      getOAuthErrorMessage(
        new ApiError(400, "oauth_proxy_unavailable", "proxy unavailable"),
      ),
    ).toBe("所选 OAuth 出口不可用，请选择已启用的出口代理。");
  });
});
