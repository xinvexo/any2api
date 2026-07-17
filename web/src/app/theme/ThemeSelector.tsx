import { Laptop, Moon, Sun } from "lucide-react";

import type { ThemeMode } from "@/app/theme/theme";
import { cn } from "@/shared/lib/cn";

const options = [
  { mode: "light", label: "浅色", icon: Sun },
  { mode: "system", label: "跟随系统", icon: Laptop },
  { mode: "dark", label: "深色", icon: Moon },
] satisfies Array<{ mode: ThemeMode; label: string; icon: typeof Sun }>;

export function ThemeSelector({
  mode,
  onModeChange,
}: {
  mode: ThemeMode;
  onModeChange: (mode: ThemeMode) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-xs font-medium text-tertiary">外观</span>
      <div className="flex h-9 items-center rounded-control border border-subtle bg-surface-muted p-1" role="group" aria-label="外观主题">
        {options.map(({ icon: Icon, label, mode: optionMode }) => (
          <button
            key={optionMode}
            type="button"
            className={cn(
              "focus-ring grid size-7 place-items-center rounded-[5px] text-tertiary transition-colors",
              "hover:text-primary",
              mode === optionMode && "bg-surface text-primary shadow-hairline",
            )}
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
