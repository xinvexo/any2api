import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { GatewayApiKey } from "../api/gateway-api-key-contracts";
import { GatewayApiKeyTableRow } from "./GatewayApiKeyTableRow";

test("uses the shared compact desktop row surface", () => {
  render(
    <table>
      <tbody>
        <GatewayApiKeyTableRow
          apiKey={apiKey()}
          pending={false}
          onEdit={vi.fn()}
          onToggleEnabled={vi.fn()}
          onRotate={vi.fn()}
          onDelete={vi.fn()}
        />
      </tbody>
    </table>,
  );

  expect(screen.getByRole("row")).toHaveClass(
    "compact-row-surface",
    "compact-row-surface-hover",
    "responsive-row-surface",
  );
  expect(screen.getByRole("row")).not.toHaveClass("sm:hover:bg-surface-muted/20");
  expect(screen.getByRole("row")).not.toHaveClass("sm:border-b");
});

function apiKey(): GatewayApiKey {
  return {
    id: "gateway-key-1",
    name: "本地开发",
    token: `sk-${"a".repeat(43)}`,
    tokenPrefix: "sk-aaaa",
    tokenVersion: 1,
    configVersion: 1,
    enabled: true,
    createdAt: "2026-08-17 20:00:00",
    lastUsedAt: null,
    usage: {
      totalRequests: 0,
      successfulRequests: 0,
      failedRequests: 0,
      windowMinutes: 2,
      windowSlots: [],
    },
  };
}
