import { useEffect, useRef } from "react";

export function IntersectionSentinel({
  enabled = true,
  onVisibilityChange,
  rootMargin = "0px",
}: {
  enabled?: boolean;
  onVisibilityChange: (visible: boolean) => void;
  rootMargin?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const element = ref.current;
    if (!enabled || !element || typeof IntersectionObserver === "undefined") {
      return;
    }
    const observer = new IntersectionObserver(
      ([entry]) => onVisibilityChange(entry?.isIntersecting === true),
      { rootMargin },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [enabled, onVisibilityChange, rootMargin]);

  return <div ref={ref} className="h-px w-full" aria-hidden="true" />;
}
