import { useEffect, useRef, type RefObject } from "react";

interface ModalEntry {
  root: HTMLElement;
  restoreTarget: HTMLElement | null;
}

const modalStack: ModalEntry[] = [];
let applicationRootState: { root: HTMLElement; wasInert: boolean } | null = null;

export function useModalFocus(
  rootRef: RefObject<HTMLElement | null>,
  active: boolean,
  onEscape?: () => void,
) {
  const onEscapeRef = useRef(onEscape);

  useEffect(() => {
    onEscapeRef.current = onEscape;
  }, [onEscape]);

  useEffect(() => {
    if (!active || typeof document === "undefined") {
      return;
    }
    const root = rootRef.current;
    if (!root) {
      return;
    }
    const entry: ModalEntry = {
      root,
      restoreTarget:
        document.activeElement instanceof HTMLElement ? document.activeElement : null,
    };
    modalStack.push(entry);
    syncInertState();

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented || modalStack.at(-1) !== entry) {
        return;
      }
      if (event.key === "Escape" && onEscapeRef.current) {
        event.preventDefault();
        onEscapeRef.current();
        return;
      }
      if (event.key === "Tab") {
        trapTabKey(event, root);
      }
    };
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      const index = modalStack.indexOf(entry);
      const wasTop = index === modalStack.length - 1;
      if (index >= 0) {
        modalStack.splice(index, 1);
      }
      root.removeAttribute("inert");
      syncInertState();
      if (wasTop) {
        restoreFocus(entry.restoreTarget);
      }
    };
  }, [active, rootRef]);
}

function trapTabKey(event: KeyboardEvent, root: HTMLElement) {
  const focusable = Array.from(
    root.querySelectorAll<HTMLElement>(
      "a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])",
    ),
  ).filter(
    (element) => element.tabIndex >= 0 && !element.hidden && !element.closest("[inert]"),
  );

  if (focusable.length === 0) {
    event.preventDefault();
    const dialog = root.matches("[role='dialog'], [role='alertdialog']")
      ? root
      : root.querySelector<HTMLElement>("[role='dialog'], [role='alertdialog']");
    dialog?.focus({ preventScroll: true });
    return;
  }
  const active = document.activeElement;
  const currentIndex = active instanceof HTMLElement ? focusable.indexOf(active) : -1;
  if (event.shiftKey && currentIndex <= 0) {
    event.preventDefault();
    focusable.at(-1)?.focus({ preventScroll: true });
  } else if (!event.shiftKey && (currentIndex === -1 || currentIndex === focusable.length - 1)) {
    event.preventDefault();
    focusable[0]?.focus({ preventScroll: true });
  }
}

function syncInertState() {
  if (modalStack.length === 0) {
    if (applicationRootState && !applicationRootState.wasInert) {
      applicationRootState.root.removeAttribute("inert");
    }
    applicationRootState = null;
    return;
  }

  if (!applicationRootState) {
    const root = document.getElementById("root");
    if (root) {
      applicationRootState = { root, wasInert: root.hasAttribute("inert") };
    }
  }
  applicationRootState?.root.setAttribute("inert", "");
  const top = modalStack.length - 1;
  modalStack.forEach((entry, index) => {
    entry.root.toggleAttribute("inert", index !== top);
  });
}

function restoreFocus(target: HTMLElement | null) {
  if (target?.isConnected && !target.closest("[inert]")) {
    target.focus({ preventScroll: true });
    return;
  }
  const underlying = modalStack.at(-1)?.root;
  underlying
    ?.querySelector<HTMLElement>("[role='dialog'], [role='alertdialog']")
    ?.focus({ preventScroll: true });
}
