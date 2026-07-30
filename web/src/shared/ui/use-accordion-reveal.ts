import { useEffect, useState } from "react";

/** Keep content swaps outside the 150 ms accordion indicator transition. */
const ACCORDION_OPENING_MS = 180;

export function useAccordionReveal(active: boolean, contentReady: boolean) {
  const [openingComplete, setOpeningComplete] = useState(contentReady);

  useEffect(() => {
    if (!active || openingComplete) {
      return;
    }

    const timeout = window.setTimeout(() => setOpeningComplete(true), ACCORDION_OPENING_MS);
    return () => window.clearTimeout(timeout);
  }, [active, openingComplete]);

  return active && contentReady && openingComplete;
}
