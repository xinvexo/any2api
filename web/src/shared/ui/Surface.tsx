import type { HTMLAttributes } from "react";

import { cn } from "@/shared/lib/cn";

export function Surface({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("rounded-[14px] border border-subtle bg-surface", className)}
      {...props}
    />
  );
}
