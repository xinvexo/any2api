const BYTE_UNITS = ["B", "KiB", "MiB", "GiB", "TiB"] as const;

export function formatResourceBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  const unitIndex = Math.min(Math.floor(Math.log(value) / Math.log(1024)), BYTE_UNITS.length - 1);
  const scaled = value / 1024 ** unitIndex;
  return `${new Intl.NumberFormat("zh-CN", { maximumFractionDigits: scaled >= 100 ? 0 : 1 }).format(scaled)} ${BYTE_UNITS[unitIndex]}`;
}

export function formatResourcePercent(value: number): string {
  return `${new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 1 }).format(value)}%`;
}

export function formatSystemMemory(usedBytes: number, totalBytes: number) {
  const ratio = totalBytes > 0 ? (usedBytes / totalBytes) * 100 : 0;
  return {
    value: formatResourcePercent(ratio),
    note: `已用 ${formatResourceBytes(usedBytes)} / 共 ${formatResourceBytes(totalBytes)}`,
  };
}
