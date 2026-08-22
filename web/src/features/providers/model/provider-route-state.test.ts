import { describe, expect, it } from "vitest";

import {
  decodeProviderRouteState,
  transitionProviderRoute,
} from "./provider-route-state";

describe("provider route state", () => {
  it("decodes a credential-model deep link", () => {
    const state = decodeProviderRouteState(new URLSearchParams(
      "kind=openai&keys=endpoint-1&credential=credential-1&action=models",
    ));

    expect(state).toEqual({
      selectedKind: "openai",
      endpointEditorId: null,
      credentialEndpointId: "endpoint-1",
      credentialId: "credential-1",
      credentialAction: "models",
    });
  });

  it("falls back to the first provider for an unknown kind", () => {
    const state = decodeProviderRouteState(new URLSearchParams("kind=unknown"));

    expect(state.selectedKind).toBe("codex");
  });

  it("clears incompatible drawers when switching provider kind", () => {
    const next = transitionProviderRoute(
      new URLSearchParams(
        "kind=codex&editor=endpoint-1&keys=endpoint-1&credential=credential-1&action=models&range=24h",
      ),
      { type: "select-kind", kind: "claude" },
    );

    expect(next.toString()).toBe("kind=claude&range=24h");
  });

  it("moves a newly created credential from its editor to model selection", () => {
    const creating = transitionProviderRoute(
      new URLSearchParams("kind=openai&editor=new"),
      {
        type: "open-credential-editor",
        endpointId: "endpoint-1",
        credentialId: "new",
        kind: "openai",
      },
    );
    const selectingModels = transitionProviderRoute(creating, {
      type: "open-credential-models",
      endpointId: "endpoint-1",
      credentialId: "credential-2",
    });

    expect(decodeProviderRouteState(selectingModels)).toEqual({
      selectedKind: "openai",
      endpointEditorId: null,
      credentialEndpointId: "endpoint-1",
      credentialId: "credential-2",
      credentialAction: "models",
    });
  });

  it("does not let a stale drawer close clear a newer credential route", () => {
    const current = new URLSearchParams(
      "kind=codex&keys=endpoint-1&credential=credential-2&action=models",
    );

    expect(transitionProviderRoute(current, {
      type: "close-credential",
      endpointId: "endpoint-1",
      expectedCredentialId: "credential-1",
    })).toBe(current);
  });
});
