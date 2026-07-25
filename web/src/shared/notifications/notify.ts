import {
  showNotification,
  type NotificationTone,
  type ShowNotificationInput,
} from "./notification-store";

export type { NotificationTone, ShowNotificationInput };

/**
 * Imperative global feedback API.
 * Prefer this over inline page banners for transient success/warning/error copy.
 *
 * Usage: `notify.success("已保存")` — rendered by root `<NotificationHost />`.
 */
export const notify = {
  show(input: ShowNotificationInput | string) {
    return showNotification(input);
  },
  success(message: string, durationMs?: number) {
    return showNotification({ message, tone: "success", durationMs });
  },
  warning(message: string, durationMs?: number) {
    return showNotification({ message, tone: "warning", durationMs });
  },
  danger(message: string, durationMs?: number) {
    return showNotification({ message, tone: "danger", durationMs });
  },
  info(message: string, durationMs?: number) {
    return showNotification({ message, tone: "info", durationMs });
  },
};
