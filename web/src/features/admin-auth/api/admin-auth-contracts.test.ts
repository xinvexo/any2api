import { expect, test } from "vitest";

import { parseAdminSessionState } from "./admin-auth-contracts";

test("parses authenticated and unauthenticated administrator session states", () => {
  expect(parseAdminSessionState(session(false, false, null))).toMatchObject({
    initialized: false,
    authenticated: false,
  });
  expect(parseAdminSessionState(session(true, true, "csrf"))).toMatchObject({
    initialized: true,
    authenticated: true,
    csrfToken: "csrf",
  });
});

test("rejects inconsistent authentication and plaintext warning states", () => {
  expect(() => parseAdminSessionState(session(true, true, null))).toThrow(
    "invalid administrator session response",
  );
  expect(() =>
    parseAdminSessionState({
      ...session(true, false, null),
      secure_transport: true,
      plaintext_http_warning: true,
    }),
  ).toThrow("invalid administrator session response");
});

function session(initialized: boolean, authenticated: boolean, csrfToken: string | null) {
  return {
    initialized,
    authenticated,
    csrf_token: csrfToken,
    remote_access_enabled: false,
    secure_transport: false,
    client_loopback: true,
    through_trusted_proxy: false,
    plaintext_http_warning: false,
  };
}
