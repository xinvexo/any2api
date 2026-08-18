import { cn } from "@/shared/lib/cn";

export function AppBrandIcon({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 32 32"
      className={cn("text-control-strong", className)}
      aria-hidden="true"
      focusable="false"
    >
      <rect x="1" y="1" width="30" height="30" rx="8" fill="currentColor" />
      <g
        fill="none"
        stroke="var(--on-control-strong)"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="2.1"
      >
        <path d="M8 16h7" />
        <path d="m15 16 7-6" />
        <path d="m15 16 7 6" />
      </g>
      <g fill="var(--on-control-strong)">
        <circle cx="8" cy="16" r="1.55" />
        <circle cx="15" cy="16" r="1.55" />
        <circle cx="23" cy="9" r="1.55" />
        <circle cx="23" cy="23" r="1.55" />
      </g>
    </svg>
  );
}
