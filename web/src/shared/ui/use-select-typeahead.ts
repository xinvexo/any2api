import { useEffect, useRef } from "react";

const RESET_DELAY_MS = 700;

export function useSelectTypeahead() {
  const bufferRef = useRef("");
  const resetTimerRef = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (resetTimerRef.current !== null) window.clearTimeout(resetTimerRef.current);
    },
    [],
  );

  return (key: string) => {
    const next = `${bufferRef.current}${key}`.toLocaleLowerCase();
    const repeated = [...next].every((character) => character === next[0]);
    bufferRef.current = repeated ? key.toLocaleLowerCase() : next;
    if (resetTimerRef.current !== null) window.clearTimeout(resetTimerRef.current);
    resetTimerRef.current = window.setTimeout(() => {
      bufferRef.current = "";
      resetTimerRef.current = null;
    }, RESET_DELAY_MS);
    return bufferRef.current;
  };
}
