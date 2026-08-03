import type { SettingItem } from "../api/settings-contracts";
import type { SettingDraft } from "../model/setting-draft";
import { validateSettingDraft } from "../model/setting-draft";

const REQUEST_GRACE_KEY = "shutdown.request_grace_period";
const FINALIZE_TIMEOUT_KEY = "shutdown.finalize_timeout";
const FINALIZE_STAGE_COUNT = 6;

interface ShutdownBudgetSummaryProps {
  items: readonly SettingItem[];
  draftFor: (item: SettingItem) => SettingDraft;
}

export function ShutdownBudgetSummary({
  items,
  draftFor,
}: ShutdownBudgetSummaryProps) {
  const requestGrace = items.find((item) => item.key === REQUEST_GRACE_KEY);
  const finalizeTimeout = items.find((item) => item.key === FINALIZE_TIMEOUT_KEY);
  if (!requestGrace || !finalizeTimeout) {
    return null;
  }

  const requestGraceSeconds = numericDraft(requestGrace, draftFor(requestGrace));
  const finalizeTimeoutSeconds = numericDraft(finalizeTimeout, draftFor(finalizeTimeout));
  const valid = requestGraceSeconds !== null && finalizeTimeoutSeconds !== null;

  return (
    <div
      className="mx-1 mt-3 rounded-[8px] bg-surface-muted px-3 py-2.5 text-[12px] leading-5 text-secondary"
      role="status"
      aria-label="优雅停机累计等待预算"
    >
      {valid ? (
        <>
          <p className="font-medium text-primary">
            最长 {formatDuration(requestGraceSeconds + FINALIZE_STAGE_COUNT * finalizeTimeoutSeconds)}
          </p>
          <p className="mt-0.5 tabular-nums">
            {requestGraceSeconds} 秒请求宽限 + {FINALIZE_STAGE_COUNT} × {finalizeTimeoutSeconds} 秒单阶段收尾
          </p>
          <p className="mt-0.5 text-tertiary">
            每个收尾阶段分别计时；这是预算上限，正常停机通常更快。
          </p>
        </>
      ) : (
        <p>修正停机时间后即可计算累计等待预算。</p>
      )}
    </div>
  );
}

function numericDraft(item: SettingItem, draft: SettingDraft) {
  const validation = validateSettingDraft(item, draft);
  return typeof validation.value === "number" ? validation.value : null;
}

function formatDuration(seconds: number) {
  if (seconds < 60) {
    return `${seconds} 秒`;
  }
  const minutes = Math.floor(seconds / 60);
  const remainder = seconds % 60;
  return remainder === 0 ? `${minutes} 分钟` : `${minutes} 分 ${remainder} 秒`;
}
