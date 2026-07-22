import type { ReactNode } from "react";

import { cn } from "@/shared/lib/cn";

interface FieldProps {
  label: string;
  htmlFor: string;
  error?: string;
  hint?: ReactNode;
  children: ReactNode;
}

export function Field({ label, htmlFor, error, hint, children }: FieldProps) {
  return (
    <div className="space-y-2">
      <label
        htmlFor={htmlFor}
        className={cn(
          "block text-[13px] font-medium tracking-tight",
          error ? "text-danger" : "text-primary",
        )}
      >
        {label}
      </label>
      {children}
      {error ? (
        <p id={`${htmlFor}-error`} className="text-[12px] leading-4 text-danger" role="alert">
          {error}
        </p>
      ) : hint ? (
        <div className="text-[12px] leading-4 text-tertiary">{hint}</div>
      ) : null}
    </div>
  );
}

export function FormError({ children }: { children: ReactNode }) {
  if (!children) {
    return null;
  }

  return (
    <p className="text-[13px] leading-5 text-danger" role="alert">
      {children}
    </p>
  );
}
