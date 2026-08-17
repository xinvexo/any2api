import { ArrowUp } from "lucide-react";

interface ScrollToTopButtonProps {
  visible: boolean;
  onClick: () => void;
}

/** Fixed control for long management views; it never changes document layout. */
export function ScrollToTopButton({ visible, onClick }: ScrollToTopButtonProps) {
  if (!visible) {
    return null;
  }

  return (
    <button
      type="button"
      aria-label="回到顶部"
      title="回到顶部"
      className="focus-ring fixed bottom-5 right-5 z-30 inline-flex size-10 items-center justify-center rounded-full border border-subtle bg-surface/95 text-secondary shadow-panel backdrop-blur-md transition-colors hover:bg-surface-hover hover:text-primary sm:bottom-6 sm:right-6"
      onClick={onClick}
    >
      <ArrowUp size={17} aria-hidden="true" />
    </button>
  );
}
