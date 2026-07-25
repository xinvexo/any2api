import { AlertTriangle, CheckCircle2, Info, X, XCircle } from "lucide-react";
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";

import {
  dismissNotification,
  subscribeNotifications,
  type NotificationItem,
  type NotificationTone,
} from "./notification-store";
import { cn } from "@/shared/lib/cn";

const toneIcon: Record<NotificationTone, typeof CheckCircle2> = {
  success: CheckCircle2,
  warning: AlertTriangle,
  danger: XCircle,
  info: Info,
};

const toneClass: Record<NotificationTone, string> = {
  success: "text-success",
  warning: "text-warning",
  danger: "text-danger",
  info: "text-accent-copy",
};

const timerBarClass: Record<NotificationTone, string> = {
  success: "bg-success",
  warning: "bg-warning",
  danger: "bg-danger",
  info: "bg-accent",
};

/**
 * App-root notification viewport.
 * Mount once outside the page router so menu/route switches never unmount
 * active notifications. Feature code must only call `notify.*`.
 */
export function NotificationHost() {
  const [items, setItems] = useState<readonly NotificationItem[]>(() => []);

  useEffect(() => subscribeNotifications(setItems), []);

  if (typeof document === "undefined") {
    return null;
  }

  // Always keep the fixed region mounted so route remounts do not tear down
  // the portal shell; cards stack independently and dismiss on their own timers.
  return createPortal(
    <div
      className="pointer-events-none fixed inset-x-0 top-0 z-[70] flex justify-center p-3 sm:p-4"
      data-notification-host=""
      aria-live="polite"
      aria-relevant="additions text"
      aria-atomic="false"
    >
      <ol
        className="flex max-h-[min(70dvh,32rem)] w-full max-w-[24rem] flex-col gap-2 overflow-y-auto overscroll-contain"
        aria-label="全局通知"
      >
        {items.map((item) => (
          <NotificationCard key={item.id} item={item} />
        ))}
      </ol>
    </div>,
    document.body,
  );
}

function NotificationCard({ item }: { item: NotificationItem }) {
  const Icon = toneIcon[item.tone];

  return (
    <li
      className={cn(
        "notification-card pointer-events-auto overflow-hidden rounded-[12px] border border-subtle bg-surface shadow-panel",
      )}
      role={item.tone === "danger" || item.tone === "warning" ? "alert" : "status"}
    >
      <div className="flex items-start gap-2.5 px-3.5 py-3">
        <Icon
          size={16}
          className={cn("mt-0.5 shrink-0", toneClass[item.tone])}
          aria-hidden="true"
        />
        <p className="min-w-0 flex-1 text-[13px] leading-5 text-primary">{item.message}</p>
        <button
          type="button"
          className="focus-ring -mr-1 -mt-0.5 grid size-7 shrink-0 place-items-center rounded-full text-tertiary transition-colors hover:bg-surface-hover hover:text-primary"
          aria-label="关闭通知"
          onClick={() => dismissNotification(item.id)}
        >
          <X size={14} aria-hidden="true" />
        </button>
      </div>
      <div
        className="notification-card__timer"
        aria-hidden="true"
        data-notification-timer=""
      >
        <div
          className={cn("notification-card__timer-bar", timerBarClass[item.tone])}
          style={{ animationDuration: `${item.durationMs}ms` }}
          data-notification-timer-bar=""
        />
      </div>
    </li>
  );
}
