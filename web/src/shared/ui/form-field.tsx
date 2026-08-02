import { AlertTriangle, XCircle } from "lucide-react";
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

  return <FormNotice tone="danger">{children}</FormNotice>;
}

export function FormNotice({
  tone,
  children,
}: {
  tone: "danger" | "warning";
  children: ReactNode;
}) {
  const Icon = tone === "danger" ? XCircle : AlertTriangle;

  return (
    <div
      className={cn(
        "flex items-start gap-2 rounded-[10px] border px-3 py-2.5 text-[13px] leading-5",
        tone === "danger" && "border-danger/20 bg-danger/8 text-danger",
        tone === "warning" && "border-warning/25 bg-warning/10 text-warning",
      )}
      role={tone === "danger" ? "alert" : "status"}
    >
      <Icon size={15} className="mt-0.5 shrink-0" aria-hidden="true" />
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  );
}
