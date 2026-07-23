import { describe, expect, it } from "vitest";

import { parseOAuthStartResult } from "./oauth-contracts";

describe("parseOAuthStartResult", () => {
  it("parses a valid OAuth2 start response", () => {
    expect(
      parseOAuthStartResult({
        provider: "codex",
        session_id: "session",
        authorization_url: "https://auth.example.com/authorize",
        redirect_uri: "http://localhost:1455/auth/callback",
        expires_in_seconds: 600,
      }),
    ).toEqual({
      provider: "codex",
      sessionId: "session",
      authorizationUrl: "https://auth.example.com/authorize",
      redirectUri: "http://localhost:1455/auth/callback",
      expiresInSeconds: 600,
    });
  });

  it("rejects an invalid provider or redirect URI", () => {
    expect(() =>
      parseOAuthStartResult({
        provider: "other",
        session_id: "session",
        authorization_url: "https://auth.example.com/authorize",
        redirect_uri: "file:///tmp/callback",
        expires_in_seconds: 600,
      }),
    ).toThrow("invalid OAuth2 login response");
  });
});
