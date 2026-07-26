import type { SystemLog, SystemLogOutcome } from "../api/system-log-contracts";

const dateTime = new Intl.DateTimeFormat("zh-CN", {
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
});

export function formatSystemLogTime(value: number) {
  return dateTime.format(new Date(value));
}

export function formatDuration(value: number) {
  if (value < 1_000) {
    return `${value} ms`;
  }
  return `${(value / 1_000).toFixed(value < 10_000 ? 2 : 1)} s`;
}

export function formatBytes(value: number) {
  if (value < 1_024) {
    return `${value} B`;
  }
  if (value < 1_048_576) {
    return `${(value / 1_024).toFixed(1)} KiB`;
  }
  return `${(value / 1_048_576).toFixed(1)} MiB`;
}

export function outcomeLabel(outcome: SystemLogOutcome) {
  switch (outcome) {
    case "completed":
      return "完成";
    case "body_error":
      return "响应错误";
    case "cancelled":
      return "已取消";
  }
}

export function statusTone(log: SystemLog) {
  const status = log.statusCode;
  if (status === null || log.outcome === "cancelled") {
    return "text-secondary";
  }
  if (status >= 500 || log.outcome === "body_error") {
    return "text-danger";
  }
  if (status >= 400) {
    return "text-warning";
  }
  if (status >= 200 && status < 400) {
    return "text-success";
  }
  return "text-secondary";
}
