import type { LucideIcon } from "lucide-react";

export type OverviewMetricTone = "blue" | "violet" | "green" | "orange";
export type OverviewMetricValueTone = "neutral" | "accent" | "danger";

const toneColors: Record<OverviewMetricTone, string> = {
  blue: "var(--chart-1)",
  violet: "var(--chart-2)",
  green: "var(--chart-6)",
  orange: "var(--chart-5)",
};

export interface OverviewMetricTileProps {
  label: string;
  value: string;
  note: string;
  progress?: number | null;
  progressLabel?: string;
  tone: OverviewMetricTone;
  valueTone?: OverviewMetricValueTone;
  icon: LucideIcon;
}

export function OverviewMetricTile({
  label,
  value,
  note,
  progress = null,
  progressLabel,
  tone,
  valueTone = "neutral",
  icon: Icon,
}: OverviewMetricTileProps) {
  const color = toneColors[tone];
  const valueClass = {
    neutral: "text-primary",
    accent: "text-accent",
    danger: "text-danger",
  }[valueTone];

  return (
    <div className="flex min-h-[7.5rem] min-w-0 flex-col rounded-[8px] border border-subtle bg-surface/70 px-3 py-3 transition-colors hover:border-strong sm:px-3.5">
      <div className="flex min-w-0 items-center gap-2">
        <span
          className="grid size-6 shrink-0 place-items-center rounded-[6px]"
          style={{
            backgroundColor: `color-mix(in srgb, ${color} 12%, transparent)`,
            color,
          }}
          aria-hidden="true"
        >
          <Icon size={14} strokeWidth={2.1} />
        </span>
        <span className="truncate text-xs font-medium text-secondary">{label}</span>
      </div>
      <strong
        className={`mt-2.5 min-w-0 truncate text-[1.45rem] font-semibold leading-none tracking-tight tabular-nums ${valueClass}`}
      >
        {value}
      </strong>
      <ProgressBar value={progress} color={color} label={progressLabel ?? `${label} 使用率`} />
      <p className="mt-1.5 truncate text-[11px] leading-4 text-tertiary" title={note}>
        {note}
      </p>
    </div>
  );
}

export function ProgressBar({
  value,
  color = "var(--accent)",
  label,
}: {
  value: number | null;
  color?: string;
  label: string;
}) {
  const bounded = value === null ? 0 : Math.min(100, Math.max(0, value));
  return (
    <div
      className="mt-2.5 h-1 overflow-hidden rounded-full bg-surface-muted"
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={value === null ? undefined : bounded}
    >
      <span
        className="block h-full rounded-full transition-[width] duration-500"
        style={{ backgroundColor: color, width: `${bounded}%` }}
      />
    </div>
  );
}
