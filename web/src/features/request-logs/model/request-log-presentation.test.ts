import { expect, test } from "vitest";

import {
  operationLabel,
  protocolLabel,
  upstreamSource,
} from "./request-log-presentation";

test("labels OpenAI Images request logs", () => {
  expect(protocolLabel("openai_images")).toBe("Images");
  expect(operationLabel("images_generations")).toBe(
    "/v1/images/generations",
  );
  expect(operationLabel("images_edits")).toBe("/v1/images/edits");
});

test("prefixes an API key label with its provider endpoint name", () => {
  const source = upstreamSource({
    oauthAccountId: null,
    credentialId: "credential-1",
    providerEndpointName: "frapi",
    credentialLabel: "key",
  });

  expect(source.kind).toBe("api_key");
  expect(source.displayName).toBe("frapi-key");
});

test("keeps OAuth labels independent from provider endpoint names", () => {
  const source = upstreamSource({
    oauthAccountId: "oauth-1",
    credentialId: null,
    providerEndpointName: "frapi",
    oauthAccountLabel: "work-oauth",
  });

  expect(source.kind).toBe("oauth");
  expect(source.displayName).toBe("work-oauth");
});

test("keeps the credential fallback when an endpoint name is unavailable", () => {
  const source = upstreamSource({
    oauthAccountId: null,
    credentialId: "credential-1",
    providerEndpointName: null,
    credentialLabel: "key",
  });

  expect(source.displayName).toBe("key");
});
