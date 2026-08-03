// @vitest-environment node

import { renderToString } from "react-dom/server";
import { expect, test, vi } from "vitest";

import { VirtualGrid } from "./VirtualGrid";

test("renders without DOM access or layout-effect warnings", () => {
  const consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);

  try {
    const html = renderToString(
      <VirtualGrid
        items={[{ id: "account-1", label: "账号 1" }]}
        getItemKey={(item) => item.id}
        renderItem={(item) => <div>{item.label}</div>}
        ariaLabel="OAuth 账号"
        collectionKey="codex"
        estimateRowHeight={240}
      />,
    );

    expect(html).toContain("OAuth 账号滚动区域");
    expect(html).toContain("账号 1");
    expect(consoleError).not.toHaveBeenCalled();
  } finally {
    consoleError.mockRestore();
  }
});
