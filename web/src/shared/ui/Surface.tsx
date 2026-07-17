import type { HTMLAttributes } from "react";

import { cn } from "@/shared/lib/cn";

export function Surface({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("rounded-panel border border-subtle bg-surface shadow-hairline", className)}
      {...props}
    />
  );
}
