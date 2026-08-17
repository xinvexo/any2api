import { render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type {
  ProxyConfiguration,
  ProxyProfile,
} from "../api/proxy-contracts";
import { ProxyList } from "./ProxyList";
import { ProxyTableRow } from "./ProxyTableRow";

test("uses a cell-backed desktop card without adding a table column", () => {
  render(
    <table>
      <tbody>
        <ProxyTableRow
          proxy={proxy()}
          isGlobal={false}
          pending={false}
          testing={false}
          testPending={false}
          testError={null}
          onTest={vi.fn()}
          onEdit={vi.fn()}
          onSetGlobal={vi.fn()}
          onDelete={vi.fn()}
        />
      </tbody>
    </table>,
  );

  const row = screen.getByRole("row");
  const cells = within(row).getAllByRole("cell");
  expect(row).toHaveClass("desktop-table-card-row");
  expect(row).not.toHaveClass("compact-row-surface");
  expect(row).not.toHaveClass("sm:hover:bg-surface-muted/20", "sm:border-b");
  expect(cells).toHaveLength(7);
  expect(cells[0]).toHaveTextContent("测试代理");
  expect(cells[1]).toHaveTextContent("HTTP");
  expect(cells[2]).toHaveTextContent("127.0.0.1:8080");
  expect(cells[6]).toHaveClass("sm:min-w-72");
});

test("shares one fixed seven-column model between header and body", () => {
  render(
    <ProxyList
      configuration={configuration()}
      pending={false}
      refreshing={false}
      actionError={null}
      testingProxyId={null}
      testResults={{}}
      testError={null}
      testErrorProxyId={null}
      onCreate={vi.fn()}
      onRefresh={vi.fn()}
      onTest={vi.fn()}
      onEdit={vi.fn()}
      onSetGlobal={vi.fn()}
      onDelete={vi.fn()}
    />,
  );

  const table = screen.getByRole("table", { name: "出口代理列表" });
  expect(table).toHaveClass("sm:table-fixed", "sm:border-separate");
  expect(table.querySelectorAll("colgroup > col")).toHaveLength(7);
  expect(table.querySelectorAll("thead th")).toHaveLength(7);
  expect(table.querySelectorAll("tbody td")).toHaveLength(7);
  expect(countLabel()).not.toHaveClass("border-t");
});

function proxy(): ProxyProfile {
  return {
    id: "proxy-1",
    name: "测试代理",
    kind: "http",
    host: "127.0.0.1",
    port: 8080,
    username: null,
    passwordConfigured: false,
    authenticationVersion: 0,
    enabled: true,
    builtIn: false,
    configVersion: 1,
  };
}

function configuration(): ProxyConfiguration {
  return {
    configRevision: 1,
    globalProxyId: "direct",
    items: [proxy()],
  };
}

function countLabel() {
  return screen.getByText(
    (_, element) => element?.tagName === "P" && element.textContent === "共 1 条",
  ).parentElement;
}
