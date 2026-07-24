import { Edit3, ListChecks, Trash2 } from "lucide-react";
import type { ReactNode } from "react";

import type { OAuthAccountPresentation } from "../model/oauth-account-presentation";
import { Button } from "@/shared/ui/Button";
import { Switch } from "@/shared/ui/Switch";
import { Surface } from "@/shared/ui/Surface";
import { cn } from "@/shared/lib/cn";

interface OAuthAccountCardProps {
  presentation: OAuthAccountPresentation;
  pending: boolean;
  onToggleEnabled: (enabled: boolean) => void;
  onViewModels: () => void;
  onEdit: () => void;
  onDelete: () => void;
  details?: ReactNode;
}

/** Shared account row with presentation data and optional provider details. */
export function OAuthAccountCard({
  presentation,
  pending,
  onToggleEnabled,
  onViewModels,
  onEdit,
  onDelete,
  details,
}: OAuthAccountCardProps) {
  return (
    <Surface className="px-3 py-2.5">
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-0.5">
            <h3 className="truncate text-[13px] font-semibold tracking-tight" title={presentation.title}>
              {presentation.title}
            </h3>
            {presentation.badges.map((badge) => (
              <span
                key={badge.key}
                className={cn(
                  "rounded-full px-1.5 py-0.5 text-[11px] font-medium",
                  badge.tone === "warning"
                    ? "bg-warning/15 text-warning-copy"
                    : "bg-surface-muted text-secondary",
                )}
              >
                {badge.label}
              </span>
            ))}
          </div>
          <p className="mt-0.5 truncate text-[12px] text-secondary">{presentation.subtitle}</p>
          {presentation.metrics.length > 0 ? (
            <p className="mt-1 flex flex-wrap gap-x-2.5 gap-y-0.5 text-[11px] text-tertiary">
              {presentation.metrics.map((metric) => (
                <span key={metric.key} className="min-w-0 truncate" title={metric.title}>
                  {metric.label}{" "}
                  <span className="tabular-nums text-secondary">{metric.value}</span>
                </span>
              ))}
            </p>
          ) : null}
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <Switch
            checked={presentation.enabled}
            disabled={pending}
            aria-label={presentation.enabled ? `停用 ${presentation.title}` : `启用 ${presentation.title}`}
            onCheckedChange={onToggleEnabled}
          />
          <Button
            variant="ghost"
            size="sm"
            disabled={pending}
            onClick={onViewModels}
            aria-label={`查看 ${presentation.title} 的可用模型`}
          >
            <ListChecks size={13} aria-hidden="true" />
            模型
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={pending}
            onClick={onEdit}
            aria-label={`编辑 ${presentation.title}`}
          >
            <Edit3 size={13} aria-hidden="true" />
            编辑
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={pending}
            onClick={onDelete}
            aria-label={`删除 ${presentation.title}`}
          >
            <Trash2 size={13} aria-hidden="true" />
          </Button>
        </div>
      </div>
      {details}
    </Surface>
  );
}
