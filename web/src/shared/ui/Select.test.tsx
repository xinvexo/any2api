import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { expect, test, vi } from "vitest";

import { Select } from "./Select";

const options = [
  { value: "responses", label: "OpenAI Responses" },
  { value: "chat", label: "Chat Completions" },
] as const;

test("opens a portal listbox and chooses an option", () => {
  function Harness() {
    const [value, setValue] = useState<(typeof options)[number]["value"]>("responses");
    return <Select aria-label="接受协议" value={value} options={options} onValueChange={setValue} />;
  }

  render(<Harness />);
  const trigger = screen.getByRole("combobox", { name: "接受协议" });
  expect(trigger).toHaveTextContent("OpenAI Responses");
  fireEvent.click(trigger);

  expect(screen.getByRole("listbox", { name: "接受协议" })).toBeInTheDocument();
  expect(screen.getByRole("option", { name: "OpenAI Responses" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  fireEvent.click(screen.getByRole("option", { name: "Chat Completions" }));

  expect(trigger).toHaveTextContent("Chat Completions");
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  expect(trigger).toHaveFocus();
});

test("keeps a drawer menu inside its modal overlay root", () => {
  render(
    <div data-testid="overlay" data-overlay-root>
      <Select
        aria-label="接受协议"
        value="responses"
        options={options}
        onValueChange={() => undefined}
      />
    </div>,
  );

  fireEvent.click(screen.getByRole("combobox", { name: "接受协议" }));
  expect(screen.getByTestId("overlay")).toContainElement(
    screen.getByRole("listbox", { name: "接受协议" }),
  );
});

test("supports keyboard selection and keeps Escape inside the open menu", () => {
  const onValueChange = vi.fn();
  const escaped = vi.fn();
  const onWindowKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") escaped();
  };
  window.addEventListener("keydown", onWindowKeyDown);
  try {
    render(
      <Select
        aria-label="类型"
        value="responses"
        options={options}
        onValueChange={onValueChange}
      />,
    );

    const trigger = screen.getByRole("combobox", { name: "类型" });
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.keyDown(trigger, { key: "Enter" });
    expect(onValueChange).toHaveBeenCalledWith("chat");

    fireEvent.keyDown(trigger, { key: " " });
    fireEvent.keyDown(trigger, { key: "Escape" });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(escaped).not.toHaveBeenCalled();
  } finally {
    window.removeEventListener("keydown", onWindowKeyDown);
  }
});

test("matches a multi-character typeahead prefix", () => {
  const onValueChange = vi.fn();
  render(
    <Select
      aria-label="模型"
      value="misc"
      options={[
        { value: "openai", label: "OpenAI Responses" },
        { value: "misc", label: "Miscellaneous" },
        { value: "omega", label: "Omega" },
      ]}
      onValueChange={onValueChange}
    />,
  );

  const trigger = screen.getByRole("combobox", { name: "模型" });
  fireEvent.keyDown(trigger, { key: "o" });
  expect(trigger).toHaveAttribute("aria-activedescendant", expect.stringMatching(/option-2$/));
  fireEvent.keyDown(trigger, { key: "p" });
  expect(trigger).toHaveAttribute("aria-activedescendant", expect.stringMatching(/option-0$/));
  fireEvent.keyDown(trigger, { key: "Enter" });
  expect(onValueChange).toHaveBeenCalledWith("openai");
});

test("closes on an outside pointer and preserves an empty-string option", () => {
  render(
    <Select
      aria-label="内部转换协议"
      value=""
      options={[
        { value: "", label: "不转换" },
        { value: "chat", label: "Chat Completions" },
      ]}
      onValueChange={() => undefined}
    />,
  );

  const trigger = screen.getByRole("combobox", { name: "内部转换协议" });
  expect(trigger).toHaveTextContent("不转换");
  fireEvent.click(trigger);
  fireEvent.pointerDown(document.body);
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
});

test("exposes disabled and invalid states on the focusable trigger", () => {
  render(
    <Select
      aria-label="排队策略"
      value="wait"
      options={[{ value: "wait", label: "等待" }]}
      onValueChange={() => undefined}
      disabled
      invalid
      aria-describedby="strategy-error"
    />,
  );

  const trigger = screen.getByRole("combobox", { name: "排队策略" });
  expect(trigger).toBeDisabled();
  expect(trigger).toHaveAttribute("aria-invalid", "true");
  expect(trigger).toHaveAttribute("aria-describedby", "strategy-error");
  fireEvent.click(trigger);
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
});

test("does not reopen after being disabled and enabled", () => {
  const props = {
    "aria-label": "类型",
    value: "responses" as const,
    options,
    onValueChange: () => undefined,
  };
  const { rerender } = render(<Select {...props} />);

  fireEvent.click(screen.getByRole("combobox", { name: "类型" }));
  expect(screen.getByRole("listbox")).toBeInTheDocument();
  rerender(<Select {...props} disabled />);
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  rerender(<Select {...props} />);
  expect(screen.getByRole("combobox", { name: "类型" })).toHaveAttribute(
    "aria-expanded",
    "false",
  );
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
});

test("normalizes the active option when the option set changes", () => {
  const onValueChange = vi.fn();
  const { rerender } = render(
    <Select
      aria-label="接受协议"
      value="responses"
      options={options}
      onValueChange={onValueChange}
    />,
  );
  const trigger = screen.getByRole("combobox", { name: "接受协议" });
  fireEvent.keyDown(trigger, { key: "ArrowDown" });
  fireEvent.keyDown(trigger, { key: "ArrowDown" });

  rerender(
    <Select
      aria-label="接受协议"
      value="responses"
      options={[options[0]]}
      onValueChange={onValueChange}
    />,
  );
  fireEvent.keyDown(trigger, { key: "Enter" });
  expect(onValueChange).toHaveBeenCalledWith("responses");
});
