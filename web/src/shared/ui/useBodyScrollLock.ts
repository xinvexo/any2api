import { useEffect } from "react";

interface BodyStyleSnapshot {
  body: HTMLElement;
  overflow: string;
  paddingRight: string;
}

let activeLocks = 0;
let snapshot: BodyStyleSnapshot | null = null;

export function useBodyScrollLock(locked: boolean) {
  useEffect(() => {
    if (!locked) {
      return;
    }
    return acquireBodyScrollLock();
  }, [locked]);
}

function acquireBodyScrollLock() {
  if (typeof document === "undefined") {
    return undefined;
  }
  const body = document.body;
  if (!body) {
    return undefined;
  }

  if (activeLocks === 0) {
    snapshot = {
      body,
      overflow: body.style.overflow,
      paddingRight: body.style.paddingRight,
    };
    body.style.overflow = "hidden";
    const scrollbarGap = Math.max(
      0,
      window.innerWidth - document.documentElement.clientWidth,
    );
    if (scrollbarGap > 0) {
      body.style.paddingRight = `${scrollbarGap}px`;
    }
  }
  activeLocks += 1;
  let released = false;
  return () => {
    if (released) {
      return;
    }
    released = true;
    activeLocks -= 1;
    if (activeLocks !== 0 || !snapshot) {
      return;
    }
    snapshot.body.style.overflow = snapshot.overflow;
    snapshot.body.style.paddingRight = snapshot.paddingRight;
    snapshot = null;
  };
}
