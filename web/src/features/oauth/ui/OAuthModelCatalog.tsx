import type { OAuthAccountPresentation } from "../model/oauth-account-presentation";
import { Button } from "@/shared/ui/Button";
import { cn } from "@/shared/lib/cn";

/** Read-only model catalog drawer body — same presentation model as the list card. */
export function OAuthModelCatalog({
  presentation,
  onClose,
}: {
  presentation: OAuthAccountPresentation;
  onClose: () => void;
}) {
  const catalog = presentation.modelCatalog;

  return (
    <div className="flex min-h-0 flex-col gap-5">
      <header className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="truncate text-[15px] font-semibold tracking-tight text-primary">
            {presentation.title}
          </p>
          <p className="mt-1 truncate text-[12px] text-secondary">{presentation.subtitle}</p>
        </div>
        <div className="flex shrink-0 flex-col items-end gap-1.5">
          {presentation.badges
            .filter((badge) => badge.tone === "neutral")
            .map((badge) => (
              <span
                key={badge.key}
                className={cn(
                  "rounded-full bg-surface-muted px-2 py-0.5 text-[11px] font-medium text-secondary",
                )}
              >
                {badge.label}
              </span>
            ))}
          <span className="tabular-nums text-[11px] text-tertiary">{catalog.length} 个模型</span>
        </div>
      </header>

      {catalog.length === 0 ? (
        <p className="py-10 text-center text-[13px] text-secondary">该账号模型目录为空</p>
      ) : (
        <ul className="space-y-1.5" aria-label="可用模型">
          {catalog.map((model, index) => (
            <li
              key={model}
              className="flex items-center gap-2.5 rounded-[10px] bg-surface-muted/70 px-3 py-2.5"
            >
              <span
                className="w-5 shrink-0 text-right font-mono text-[10px] tabular-nums text-tertiary"
                aria-hidden="true"
              >
                {String(index + 1).padStart(2, "0")}
              </span>
              <span className="min-w-0 flex-1 break-all font-mono text-[12.5px] leading-5 tracking-tight text-primary">
                {model}
              </span>
            </li>
          ))}
        </ul>
      )}

      <div className="flex justify-end border-t border-subtle pt-4">
        <Button type="button" variant="secondary" onClick={onClose}>
          关闭
        </Button>
      </div>
    </div>
  );
}
