import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { WindowVirtualList } from "./WindowVirtualList";

test("renders only the visible window and follows document scrolling", async () => {
  const items = Array.from({ length: 200 }, (_, index) => ({
    id: `item-${index + 1}`,
    label: `项目 ${index + 1}`,
  }));
  let scrollY = 0;
  const scrollYMock = vi.spyOn(window, "scrollY", "get").mockImplementation(() => scrollY);

  try {
    render(
      <WindowVirtualList
        items={items}
        getItemKey={(item) => item.id}
        renderItem={(item) => <div>{item.label}</div>}
        ariaLabel="移动端虚拟列表"
        estimateItemHeight={72}
        gap={8}
        overscan={2}
      />,
    );

    const list = screen.getByRole("list", { name: "移动端虚拟列表" });
    expect(within(list).getByText("项目 1")).toBeInTheDocument();
    expect(within(list).queryByText("项目 200")).not.toBeInTheDocument();
    expect(within(list).getAllByRole("listitem").length).toBeLessThan(30);

    vi.spyOn(list, "getBoundingClientRect").mockImplementation(
      () => new DOMRect(0, -scrollY, 390, 15_992),
    );
    scrollY = 15_200;
    fireEvent.scroll(window);

    await waitFor(() => expect(within(list).getByText("项目 200")).toBeInTheDocument());
    expect(within(list).queryByText("项目 1")).not.toBeInTheDocument();
    expect(within(list).getAllByRole("listitem").length).toBeLessThan(30);
    expect(within(list).getByText("项目 200").closest("[role='listitem']")).toHaveAttribute(
      "aria-posinset",
      "200",
    );
  } finally {
    scrollYMock.mockRestore();
  }
});
