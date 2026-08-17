import { useEffect, useLayoutEffect, useRef, useState } from "react";

export type ListEntryAnimation = "arrive" | "complete";

const ANIMATION_LIFETIME_MS = 900;
const EMPTY_ANIMATIONS: ReadonlyMap<string, ListEntryAnimation> = new Map();

/**
 * Marks only newly prepended entries and real state transitions. Appending older
 * cursor pages is intentionally ignored so virtual scrolling never replays effects.
 */
export function useListEntryAnimations<T>(
  items: readonly T[],
  getId: (item: T) => string,
  getState: (item: T) => string,
  collectionKey: string,
) {
  const collectionKeyRef = useRef(collectionKey);
  const previousRef = useRef<Map<string, string> | null>(null);
  const timersRef = useRef(new Map<string, number>());
  const [animations, setAnimations] = useState(EMPTY_ANIMATIONS);

  useLayoutEffect(() => {
    const previous = previousRef.current;
    const next = new Map(items.map((item) => [getId(item), getState(item)]));
    previousRef.current = next;
    if (previous === null || collectionKeyRef.current !== collectionKey) {
      collectionKeyRef.current = collectionKey;
      timersRef.current.forEach((timer) => window.clearTimeout(timer));
      timersRef.current.clear();
      setAnimations(EMPTY_ANIMATIONS);
      return;
    }

    const firstKnownIndex = items.findIndex((item) => previous.has(getId(item)));
    const prependBoundary =
      firstKnownIndex >= 0
        ? firstKnownIndex
        : previous.size === 0
          ? items.length
          : 0;
    const detected = new Map<string, ListEntryAnimation>();
    items.forEach((item, index) => {
      const id = getId(item);
      const previousState = previous.get(id);
      if (previousState !== undefined && previousState !== getState(item)) {
        detected.set(id, "complete");
      } else if (
        previousState === undefined &&
        index < prependBoundary
      ) {
        detected.set(id, "arrive");
      }
    });

    if (detected.size === 0) return;

    setAnimations((current) => {
      const merged = new Map(current);
      detected.forEach((animation, id) => merged.set(id, animation));
      return merged;
    });
    detected.forEach((_, id) => {
      const activeTimer = timersRef.current.get(id);
      if (activeTimer !== undefined) window.clearTimeout(activeTimer);
      const timer = window.setTimeout(() => {
        timersRef.current.delete(id);
        setAnimations((current) => {
          if (!current.has(id)) return current;
          const remaining = new Map(current);
          remaining.delete(id);
          return remaining.size === 0 ? EMPTY_ANIMATIONS : remaining;
        });
      }, ANIMATION_LIFETIME_MS);
      timersRef.current.set(id, timer);
    });
  }, [collectionKey, getId, getState, items]);

  useEffect(() => {
    const timers = timersRef.current;
    return () => {
      timers.forEach((timer) => window.clearTimeout(timer));
      timers.clear();
    };
  }, []);

  return animations;
}

export function listEntryAnimationClass(animation: ListEntryAnimation | undefined) {
  if (animation === "arrive") return "log-entry-arrive";
  if (animation === "complete") return "log-entry-complete";
  return undefined;
}
