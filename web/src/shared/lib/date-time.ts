const compactDateTime = new Intl.DateTimeFormat("zh-CN", {
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
});

/** Compact timestamp shared by dense log lists. */
export function formatCompactDateTime(value: number) {
  return compactDateTime.format(value);
}
