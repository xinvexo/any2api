export function chartColorWithAlpha(color: string, alpha: number) {
  const normalized = color.trim().replace("#", "");
  if (!/^[0-9a-f]{6}$/i.test(normalized)) return color;
  const red = Number.parseInt(normalized.slice(0, 2), 16);
  const green = Number.parseInt(normalized.slice(2, 4), 16);
  const blue = Number.parseInt(normalized.slice(4, 6), 16);
  return `rgb(${red} ${green} ${blue} / ${alpha})`;
}
