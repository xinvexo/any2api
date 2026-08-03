import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import { expect, test, vi } from "vitest";

import { VirtualGrid } from "./VirtualGrid";

test("commits the measured responsive columns before the first paint", () => {
  const width = vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(900);
  const container = document.createElement("div");
  document.body.append(container);
  const root = createRoot(container);

  try {
    flushSync(() => {
      root.render(
        <VirtualGrid
          items={[1, 2, 3, 4]}
          getItemKey={(item) => item}
          renderItem={(item) => <div>项目 {item}</div>}
          ariaLabel="响应式虚拟网格"
          collectionKey="responsive"
          estimateRowHeight={80}
          minItemWidth={260}
          maxColumns={3}
          gap={12}
        />,
      );
    });

    expect(container.querySelector<HTMLElement>("[data-index='0']")?.style.gridTemplateColumns)
      .toBe("repeat(3, minmax(0, 1fr))");
  } finally {
    flushSync(() => root.unmount());
    container.remove();
    width.mockRestore();
  }
});

test("renders a virtual window and resets scroll for a new collection", async () => {
  const items = Array.from({ length: 40 }, (_, index) => ({
    id: `item-${index + 1}`,
    label: `项目 ${index + 1}`,
  }));
  const { rerender } = render(
    <VirtualGrid
      items={items}
      getItemKey={(item) => item.id}
      renderItem={(item) => <div>{item.label}</div>}
      ariaLabel="测试虚拟网格"
      collectionKey="first"
      estimateRowHeight={80}
      maxColumns={1}
      gap={8}
      overscanRows={1}
    />,
  );

  const viewport = screen.getByRole("region", { name: "测试虚拟网格滚动区域" });
  expect(screen.getByText("项目 1")).toBeInTheDocument();
  expect(screen.queryByText("项目 40")).not.toBeInTheDocument();

  viewport.scrollTop = 2_400;
  fireEvent.scroll(viewport);
  await waitFor(() => expect(screen.getByText("项目 31")).toBeInTheDocument());
  expect(screen.queryByText("项目 1")).not.toBeInTheDocument();

  rerender(
    <VirtualGrid
      items={items}
      getItemKey={(item) => item.id}
      renderItem={(item) => <div>{item.label}</div>}
      ariaLabel="测试虚拟网格"
      collectionKey="second"
      estimateRowHeight={80}
      maxColumns={1}
      gap={8}
      overscanRows={1}
    />,
  );
  await waitFor(() => expect(viewport.scrollTop).toBe(0));
});
