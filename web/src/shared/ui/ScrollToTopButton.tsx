import { ArrowUp } from "lucide-react";
import { useEffect, useState } from "react";

const MOBILE_VIEWPORT_QUERY = "(max-width: 767px)";
const MOBILE_SCROLL_THRESHOLD = 320;

interface ScrollToTopButtonProps {
  visible: boolean;
  onClick: () => void;
}

/** Fixed control for long management views; it never changes document layout. */
export function ScrollToTopButton({ visible, onClick }: ScrollToTopButtonProps) {
  const [mobileScroll, setMobileScroll] = useState(readMobileScrollState);

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      return;
    }
    const media = window.matchMedia(MOBILE_VIEWPORT_QUERY);
    const update = () => {
      const next = {
        mobile: media.matches,
        pastThreshold: media.matches && window.scrollY >= MOBILE_SCROLL_THRESHOLD,
      };
      setMobileScroll((current) =>
        current.mobile === next.mobile && current.pastThreshold === next.pastThreshold
          ? current
          : next,
      );
    };
    update();
    window.addEventListener("scroll", update, { passive: true });
    media.addEventListener("change", update);
    return () => {
      window.removeEventListener("scroll", update);
      media.removeEventListener("change", update);
    };
  }, []);

  const shouldShow = mobileScroll.mobile ? mobileScroll.pastThreshold : visible;
  if (!shouldShow) {
    return null;
  }

  const handleClick = () => {
    if (mobileScroll.mobile) {
      window.scrollTo({ top: 0, behavior: "smooth" });
    }
    onClick();
  };

  return (
    <button
      type="button"
      aria-label="回到顶部"
      title="回到顶部"
      className="focus-ring fixed bottom-5 right-5 z-30 inline-flex size-10 items-center justify-center rounded-full border border-subtle bg-surface/95 text-secondary shadow-panel backdrop-blur-md transition-colors hover:bg-surface-hover hover:text-primary sm:bottom-6 sm:right-6"
      onClick={handleClick}
    >
      <ArrowUp size={17} aria-hidden="true" />
    </button>
  );
}

function readMobileScrollState() {
  const mobile = typeof window !== "undefined"
    && typeof window.matchMedia === "function"
    && window.matchMedia(MOBILE_VIEWPORT_QUERY).matches;
  return {
    mobile,
    pastThreshold: mobile && window.scrollY >= MOBILE_SCROLL_THRESHOLD,
  };
}
