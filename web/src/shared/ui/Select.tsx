import { Check, ChevronDown } from "lucide-react";
import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import { createPortal } from "react-dom";

import {
  edgeEnabledIndex,
  findTypeaheadMatch,
  nextEnabledIndex,
  normalizeActiveIndex,
  optionId,
  resolveMenuLayout,
  SELECT_MENU_MAX_HEIGHT,
  type MenuLayout,
  type SelectOption,
  type SelectValue,
} from "./select-model";
import { useSelectTypeahead } from "./use-select-typeahead";
import { cn } from "@/shared/lib/cn";

export type { SelectOption, SelectValue } from "./select-model";

interface SelectProps<T extends SelectValue> {
  value: T;
  options: readonly SelectOption<T>[];
  onValueChange: (value: T) => void;
  id?: string;
  disabled?: boolean;
  invalid?: boolean;
  className?: string;
  placeholder?: string;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
}

export function Select<T extends SelectValue>(props: SelectProps<T>) {
  return <SelectControl key={props.disabled ? "disabled" : "enabled"} {...props} />;
}

function SelectControl<T extends SelectValue>({
  value,
  options,
  onValueChange,
  id,
  disabled = false,
  invalid = false,
  className,
  placeholder = "请选择",
  "aria-label": ariaLabel,
  "aria-labelledby": ariaLabelledBy,
  "aria-describedby": ariaDescribedBy,
}: SelectProps<T>) {
  const generatedId = useId();
  const triggerId = id ?? `select-${generatedId}`;
  const listboxId = `${triggerId}-listbox`;
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const appendTypeahead = useSelectTypeahead();
  const [open, setOpen] = useState(false);
  const [activeValue, setActiveValue] = useState<T | null>(null);
  const [layout, setLayout] = useState<MenuLayout | null>(null);
  const selectedIndex = options.findIndex((option) => Object.is(option.value, value));
  const selectedOption = selectedIndex >= 0 ? options[selectedIndex] : undefined;
  const storedActiveIndex = options.findIndex(
    (option) => activeValue !== null && Object.is(option.value, activeValue) && !option.disabled,
  );
  const activeIndex =
    storedActiveIndex >= 0 ? storedActiveIndex : normalizeActiveIndex(options, selectedIndex);
  const isOpen = open && !disabled;

  useEffect(() => {
    if (!isOpen) {
      return;
    }
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node | null;
      if (
        !target ||
        triggerRef.current?.contains(target) ||
        menuRef.current?.contains(target)
      ) {
        return;
      }
      setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown, true);
    return () => document.removeEventListener("pointerdown", onPointerDown, true);
  }, [isOpen]);

  useLayoutEffect(() => {
    if (!isOpen) {
      return;
    }

    const update = () => {
      const trigger = triggerRef.current;
      const menu = menuRef.current;
      if (!trigger || !menu) {
        return;
      }
      setLayout(resolveMenuLayout(trigger.getBoundingClientRect(), menu, options.length));
    };

    update();
    window.addEventListener("resize", update);
    document.addEventListener("scroll", update, true);
    return () => {
      window.removeEventListener("resize", update);
      document.removeEventListener("scroll", update, true);
    };
  }, [isOpen, options.length]);

  useEffect(() => {
    if (!isOpen || activeIndex < 0) {
      return;
    }
    const active = document.getElementById(optionId(listboxId, activeIndex));
    active?.scrollIntoView?.({ block: "nearest" });
  }, [activeIndex, isOpen, listboxId]);

  function openMenu() {
    if (disabled) {
      return;
    }
    setLayout(null);
    activate(normalizeActiveIndex(options, selectedIndex));
    setOpen(true);
  }

  function activate(index: number) {
    setActiveValue(index >= 0 ? options[index]?.value ?? null : null);
  }

  function choose(index: number) {
    const option = options[index];
    if (!option || option.disabled) {
      return;
    }
    onValueChange(option.value);
    setOpen(false);
    triggerRef.current?.focus({ preventScroll: true });
  }

  function moveActive(direction: 1 | -1) {
    activate(nextEnabledIndex(options, activeIndex, direction));
  }

  function handleKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (event.key === "Escape" && isOpen) {
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      return;
    }
    if (event.key === "Tab") {
      setOpen(false);
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      event.stopPropagation();
      if (!isOpen) {
        openMenu();
      } else {
        moveActive(event.key === "ArrowDown" ? 1 : -1);
      }
      return;
    }
    if (isOpen && (event.key === "Home" || event.key === "End")) {
      event.preventDefault();
      event.stopPropagation();
      activate(edgeEnabledIndex(options, event.key === "Home"));
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      event.stopPropagation();
      if (isOpen) {
        choose(activeIndex);
      } else {
        openMenu();
      }
      return;
    }
    if (event.key.length === 1 && !event.altKey && !event.ctrlKey && !event.metaKey) {
      const query = appendTypeahead(event.key);
      const match = findTypeaheadMatch(options, activeIndex, query);
      if (match >= 0) {
        event.preventDefault();
        if (!isOpen) setLayout(null);
        activate(match);
        setOpen(true);
      }
    }
  }

  return (
    <>
      <button
        ref={triggerRef}
        id={triggerId}
        type="button"
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        aria-controls={listboxId}
        aria-activedescendant={isOpen && activeIndex >= 0 ? optionId(listboxId, activeIndex) : undefined}
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledBy}
        aria-describedby={ariaDescribedBy}
        aria-invalid={invalid}
        disabled={disabled}
        data-value={String(value)}
        className={cn(
          "focus-ring flex h-8 w-full min-w-0 items-center gap-2 rounded-[8px] border-0",
          "bg-surface-muted py-0 pl-2.5 pr-3.5 text-left text-[12px] text-primary",
          "disabled:cursor-not-allowed disabled:opacity-50",
          invalid && "bg-danger/[0.05] ring-1 ring-inset ring-danger/40",
          className,
        )}
        onClick={() => (isOpen ? setOpen(false) : openMenu())}
        onKeyDown={handleKeyDown}
      >
        <span className={cn("min-w-0 flex-1 truncate", !selectedOption && "text-tertiary")}>
          {selectedOption?.label ?? placeholder}
        </span>
        <ChevronDown
          size={14}
          strokeWidth={1.75}
          aria-hidden="true"
          className={cn("shrink-0 text-tertiary", isOpen && "text-primary")}
        />
      </button>
      {isOpen && typeof document !== "undefined"
        ? createPortal(
            <div
              ref={menuRef}
              id={listboxId}
              role="listbox"
              aria-label={ariaLabel}
              aria-labelledby={ariaLabelledBy ?? (ariaLabel ? undefined : triggerId)}
              className="fixed z-[80] overflow-y-auto rounded-[8px] border border-strong bg-surface p-1 shadow-panel"
              style={{
                left: layout?.left ?? 0,
                top: layout?.top ?? 0,
                width: layout?.width ?? 0,
                maxHeight: layout?.maxHeight ?? SELECT_MENU_MAX_HEIGHT,
                visibility: layout ? "visible" : "hidden",
              }}
              onPointerDown={(event) => {
                if (event.pointerType === "mouse") event.preventDefault();
              }}
            >
              {options.map((option, index) => {
                const active = index === activeIndex;
                const selected = index === selectedIndex;
                return (
                  <button
                    key={`${String(option.value)}-${index}`}
                    id={optionId(listboxId, index)}
                    type="button"
                    role="option"
                    tabIndex={-1}
                    aria-selected={selected}
                    aria-disabled={option.disabled || undefined}
                    disabled={option.disabled}
                    title={option.label}
                    className={cn(
                      "flex min-h-8 w-full items-center gap-2 rounded-[6px] px-2 text-left text-[12px]",
                      "disabled:cursor-not-allowed disabled:opacity-40",
                      active ? "bg-accent text-on-accent" : "text-primary",
                    )}
                    onPointerMove={() => !option.disabled && activate(index)}
                    onClick={() => choose(index)}
                  >
                    <span className="min-w-0 flex-1 truncate">{option.label}</span>
                    <Check
                      size={14}
                      strokeWidth={2}
                      aria-hidden="true"
                      className={cn("shrink-0", selected ? "opacity-100" : "opacity-0")}
                    />
                  </button>
                );
              })}
            </div>,
            triggerRef.current?.closest<HTMLElement>("[data-overlay-root]") ?? document.body,
          )
        : null}
    </>
  );
}
