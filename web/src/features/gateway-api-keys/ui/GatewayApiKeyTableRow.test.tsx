import { render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type {
  GatewayApiKey,
  GatewayApiKeyConfiguration,
} from "../api/gateway-api-key-contracts";
import { GatewayApiKeyList } from "./GatewayApiKeyList";
import { GatewayApiKeyTableRow } from "./GatewayApiKeyTableRow";

test("uses a cell-backed desktop card without adding a table column", () => {
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

  const row = screen.getByRole("row");
  expect(row).toHaveClass("desktop-table-card-row");
  expect(row).not.toHaveClass("compact-row-surface");
  expect(screen.getByRole("row")).not.toHaveClass("sm:hover:bg-surface-muted/20");
  expect(screen.getByRole("row")).not.toHaveClass("sm:border-b");
  expect(within(row).getAllByRole("cell")).toHaveLength(5);
  expect(within(row).getAllByRole("cell")[2]).toHaveTextContent("2026/08/17");
  expect(within(row).getAllByRole("cell")[3]).toHaveTextContent("2026/08/16");
  expect(within(row).getAllByRole("cell")[4]).toHaveClass("sm:min-w-80");
});

test("shares one fixed five-column model between header and body", () => {
  render(
    <GatewayApiKeyList
      configuration={configuration()}
      pending={false}
      refreshing={false}
      actionError={null}
      onCreate={vi.fn()}
      onRefresh={vi.fn()}
      onEdit={vi.fn()}
      onToggleEnabled={vi.fn()}
      onRotate={vi.fn()}
      onDelete={vi.fn()}
    />,
  );

  const table = screen.getByRole("table", { name: "网关密钥列表" });
  expect(table).toHaveClass("sm:table-fixed", "sm:border-separate");
  expect(table.querySelectorAll("colgroup > col")).toHaveLength(5);
  expect(table.querySelectorAll("thead th")).toHaveLength(5);
  expect(table.querySelectorAll("tbody td")).toHaveLength(5);
  expect(countLabel()).not.toHaveClass("border-t");
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
    createdAt: "2026/08/16 20:00:00",
    lastUsedAt: "2026/08/17 20:00:00",
    usage: {
      totalRequests: 0,
      successfulRequests: 0,
      failedRequests: 0,
      windowMinutes: 2,
      windowSlots: [],
    },
  };
}

function configuration(): GatewayApiKeyConfiguration {
  return { configRevision: 1, items: [apiKey()] };
}

function countLabel() {
  return screen.getByText(
    (_, element) => element?.tagName === "P" && element.textContent === "共 1 条",
  ).parentElement;
}
