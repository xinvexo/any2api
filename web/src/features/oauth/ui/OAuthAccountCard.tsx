import { Bot, Edit3, ListChecks, Network, Trash2 } from "lucide-react";
import type { ReactNode } from "react";

import type { OAuthAccountPresentation } from "../model/oauth-account-presentation";
import { cn } from "@/shared/lib/cn";
import { RowActionButton } from "@/shared/ui/RowActionButton";
import { Surface } from "@/shared/ui/Surface";
import { Switch } from "@/shared/ui/Switch";

interface OAuthAccountCardProps {
  presentation: OAuthAccountPresentation;
  proxyLabel: string;
  pending: boolean;
  onToggleEnabled: (enabled: boolean) => void;
  onViewModels: () => void;
  onEdit: () => void;
  onDelete: () => void;
  details?: ReactNode;
  lastUpdatedAt?: number | null;
}

/**
 * Compact tile for the OAuth account grid.
 * Sized for 1 / 2 / 3 columns — dense layout, plain surface chrome.
 */
export function OAuthAccountCard({
  presentation,
  proxyLabel,
  pending,
  onToggleEnabled,
  onViewModels,
  onEdit,
  onDelete,
  details,
  lastUpdatedAt = null,
}: OAuthAccountCardProps) {
  const planBadge = presentation.badges.find((badge) => badge.key === "plan");
  const statusBadges = presentation.badges.filter((badge) => badge.key !== "plan");
  const hasProblem = statusBadges.some((badge) => badge.key === "token-refresh-failed");

  return (
    <Surface
      data-floating-bounds
      className={cn(
        "flex h-full min-w-0 flex-col overflow-hidden p-0 shadow-hairline",
        "transition-opacity duration-150",
        hasProblem && [
          "border-danger/20 bg-linear-to-b",
          "from-danger/10 via-danger/[0.035] to-surface",
        ],
        !presentation.enabled && "opacity-[0.72]",
      )}
    >
      <div className="flex items-start gap-2 px-3 pt-2.5 pb-0">
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-1.5">
            <h3
              className="min-w-0 truncate text-[13px] font-semibold tracking-tight text-primary"
              title={presentation.title}
            >
              {presentation.title}
            </h3>
            {statusBadges.map((badge) => badge.key === "bot-flagged" ? (
              <Bot
                key={badge.key}
                size={14}
                className="shrink-0 text-warning"
                role="img"
                aria-label={badge.label}
              />
            ) : (
              <span
                key={badge.key}
                aria-label={`账号状态：${badge.label}`}
                className={cn(
                  "shrink-0 rounded-full px-1.5 py-px text-[10px] font-medium leading-4",
                  badge.tone === "success" && "bg-success/10 text-success",
                  badge.tone === "warning" && "bg-warning/12 text-warning",
                  badge.tone === "danger" && "bg-danger/10 text-danger",
                )}
              >
                {badge.label}
              </span>
            ))}
          </div>
          <p className="mt-0.5 truncate text-[12px] text-secondary" title={presentation.subtitle}>
            {presentation.subtitle}
          </p>
          <p className="mt-1 flex min-w-0 items-center gap-1 text-[11px] text-secondary">
            <Network size={11} className="shrink-0" aria-hidden="true" />
            <span className="truncate" title={proxyLabel}>{proxyLabel}</span>
          </p>
        </div>
        {/* Plan + switch stay pinned on the right; title truncation won't shove them. */}
        <div className="flex shrink-0 items-center gap-1.5 pt-0.5">
          {planBadge ? (
            <span className="rounded-full bg-surface-muted px-1.5 py-px text-[10px] font-medium leading-4 text-secondary">
              {planBadge.label}
            </span>
          ) : null}
          <Switch
            checked={presentation.enabled}
            disabled={pending}
            aria-label={
              presentation.enabled
                ? `停用 ${presentation.title}`
                : `启用 ${presentation.title}`
            }
            onCheckedChange={onToggleEnabled}
          />
        </div>
      </div>

      {/* Inset hairlines — stop short of card edges to avoid table-like full rules. */}
      <div className="px-3 py-2">
        <div className="border-t border-subtle/50 pt-2">
          {presentation.metrics.length > 0 ? (
            <div className="flex flex-wrap items-baseline gap-x-3 gap-y-0.5 text-[11px]">
              {presentation.metrics.map((metric) => (
                <span key={metric.key} className="inline-flex min-w-0 items-baseline gap-1">
                  <span className="shrink-0 text-secondary">{metric.label}</span>
                  <span
                    className={cn(
                      "truncate font-medium tabular-nums text-primary",
                      metric.tone === "success" && "text-success",
                      metric.tone === "warning" && "text-warning",
                    )}
                    title={metric.title ?? metric.value}
                  >
                    {metric.value}
                  </span>
                </span>
              ))}
            </div>
          ) : null}
          {details ? (
            <div className={presentation.metrics.length > 0 ? "mt-1.5" : undefined}>{details}</div>
          ) : null}
        </div>
      </div>

      <div className="mt-auto px-3">
        <div className="flex min-w-0 items-center justify-between gap-1 border-t border-subtle/50 px-0 py-1">
          {lastUpdatedAt === null ? null : (
            <span
              className="min-w-0 truncate text-[10px] tabular-nums text-tertiary"
              title={`最后更新 ${formatUpdatedAt(lastUpdatedAt)}`}
            >
              最后更新 {formatUpdatedAt(lastUpdatedAt)}
            </span>
          )}
          <div className="ml-auto flex shrink-0 items-center">
            <RowActionButton
              quiet
              label={`查看 ${presentation.title} 的可用模型`}
              disabled={pending}
              onClick={onViewModels}
            >
              <ListChecks size={12} aria-hidden="true" />
              模型
            </RowActionButton>
            <RowActionButton
              quiet
              label={`编辑 ${presentation.title}`}
              disabled={pending}
              onClick={onEdit}
            >
              <Edit3 size={12} aria-hidden="true" />
              编辑
            </RowActionButton>
            <RowActionButton
              quiet
              tone="danger"
              label={`删除 ${presentation.title}`}
              disabled={pending}
              onClick={onDelete}
            >
              <Trash2 size={12} aria-hidden="true" />
            </RowActionButton>
          </div>
        </div>
      </div>
    </Surface>
  );
}

function formatUpdatedAt(value: number) {
  const date = new Date(value * 1_000);
  if (Number.isNaN(date.getTime())) return String(value);
  const part = (number: number) => String(number).padStart(2, "0");
  return `${part(date.getMonth() + 1)}/${part(date.getDate())} ${part(date.getHours())}:${part(date.getMinutes())}:${part(date.getSeconds())}`;
}
