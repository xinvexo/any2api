/**
 * Global feedback notifications.
 *
 * - Mount `<NotificationHost />` once at the app root (outside the page router).
 * - Call `notify.success/warning/danger/info` from any feature for transient feedback.
 * - Do not render inline success banners that shift page layout.
 * - Menu/route switches must not clear active notifications.
 */
export { notify } from "./notify";
export type { NotificationTone, ShowNotificationInput } from "./notify";
export { NotificationHost } from "./NotificationHost";
export {
  clearNotifications,
  dismissNotification,
  getNotifications,
} from "./notification-store";
