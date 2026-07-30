import { Moon, Sun } from "lucide-react";

import type { ThemeMode } from "@/app/theme/theme";
import { cn } from "@/shared/lib/cn";
import { SlidingSelectionIndicator } from "@/shared/ui/SlidingSelectionIndicator";

const options = [
  { mode: "light", label: "浅色", icon: Sun },
  { mode: "dark", label: "深色", icon: Moon },
] satisfies Array<{ mode: ThemeMode; label: string; icon: typeof Sun }>;

export function ThemeSelector({
  mode,
  onModeChange,
  compact = false,
}: {
  mode: ThemeMode;
  onModeChange: (mode: ThemeMode) => void;
  compact?: boolean;
}) {
  return (
    <div className={cn("flex items-center gap-3", !compact && "justify-between")}>
      {compact ? null : <span className="text-xs font-medium text-tertiary">外观</span>}
      <div
        className="app-glass-chip relative isolate flex h-9 items-center rounded-[10px] p-1"
        role="group"
        aria-label="外观主题"
      >
        <SlidingSelectionIndicator
          selected={mode}
          className="rounded-full bg-surface shadow-hairline"
        />
        {options.map(({ icon: Icon, label, mode: optionMode }) => (
          <button
            key={optionMode}
            type="button"
            className={cn(
              "focus-ring relative z-10 inline-flex h-7 items-center justify-center rounded-full px-2.5 text-tertiary transition-colors",
              "hover:text-primary",
              mode === optionMode && "text-primary",
            )}
            data-sliding-selection-item={optionMode}
            aria-label={label}
            aria-pressed={mode === optionMode}
            title={label}
            onClick={() => onModeChange(optionMode)}
          >
            <Icon size={15} aria-hidden="true" />
          </button>
        ))}
      </div>
    </div>
  );
}
