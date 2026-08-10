import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import type { ProviderUpstreamProtocolOption } from "../api/provider-contracts";
import { ProtocolFidelityNotice } from "./ProtocolFidelityNotice";

test("explains that Direct does not promise byte transparency", () => {
  render(
    <ProtocolFidelityNotice
      option={{
        protocol: "openai_responses",
        fidelity: "direct",
        operations: ["responses", "responses_compact"],
        bridge: null,
      }}
    />,
  );

  expect(screen.getByText("Direct")).toBeInTheDocument();
  expect(screen.getByText(/不代表逐字节透明/)).toBeInTheDocument();
});

test("renders the selected Translated bridge contract and limitations", () => {
  const option = {
    protocol: "openai_chat_completions",
    fidelity: "translated",
    operations: ["responses"],
    bridge: {
      contractId: "openai-responses-to-chat-completions/v1",
      requestFields: [
        { path: "input", behavior: "translated" },
        { path: "client_metadata", behavior: "validated_only" },
      ],
      toolTypes: ["function"],
      limitations: [
        {
          code: "canonical_request_reconstruction",
          description: "The request is reconstructed.",
        },
      ],
    },
  } satisfies ProviderUpstreamProtocolOption;

  render(<ProtocolFidelityNotice option={option} />);

  expect(screen.getByText("Translated")).toBeInTheDocument();
  expect(
    screen.getByText("openai-responses-to-chat-completions/v1"),
  ).toBeInTheDocument();
  expect(screen.getByText("client_metadata · 仅校验")).toBeInTheDocument();
  expect(screen.getByText("The request is reconstructed.")).toBeInTheDocument();
});
