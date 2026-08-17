import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { ProxyProfile } from "../api/proxy-contracts";
import { ProxyTableRow } from "./ProxyTableRow";

test("uses the shared compact desktop row surface", () => {
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

  expect(screen.getByRole("row")).toHaveClass(
    "compact-row-surface",
    "compact-row-surface-hover",
    "responsive-row-surface",
  );
  expect(screen.getByRole("row")).not.toHaveClass("sm:hover:bg-surface-muted/20");
  expect(screen.getByRole("row")).not.toHaveClass("sm:border-b");
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
