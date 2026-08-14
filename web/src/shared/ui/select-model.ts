export type SelectValue = string | number;

export interface SelectOption<T extends SelectValue> {
  value: T;
  label: string;
  disabled?: boolean;
}

export interface MenuLayout {
  left: number;
  top: number;
  width: number;
  maxHeight: number;
}

const EDGE_GAP = 8;
const MENU_GAP = 4;
export const SELECT_MENU_MAX_HEIGHT = 240;
const OPTION_HEIGHT = 32;

export function normalizeActiveIndex<T extends SelectValue>(
  options: readonly SelectOption<T>[],
  selectedIndex: number,
) {
  if (selectedIndex >= 0 && !options[selectedIndex]?.disabled) {
    return selectedIndex;
  }
  return edgeEnabledIndex(options, true);
}

export function edgeEnabledIndex<T extends SelectValue>(
  options: readonly SelectOption<T>[],
  first: boolean,
) {
  const indices = options.map((_, index) => index);
  if (!first) indices.reverse();
  return indices.find((index) => !options[index]?.disabled) ?? -1;
}

export function nextEnabledIndex<T extends SelectValue>(
  options: readonly SelectOption<T>[],
  current: number,
  direction: 1 | -1,
) {
  if (options.length === 0) return -1;
  for (let offset = 1; offset <= options.length; offset += 1) {
    const index = (current + direction * offset + options.length) % options.length;
    if (!options[index]?.disabled) return index;
  }
  return current;
}

export function findTypeaheadMatch<T extends SelectValue>(
  options: readonly SelectOption<T>[],
  current: number,
  query: string,
) {
  const needle = query.toLocaleLowerCase();
  const firstOffset = query.length > 1 ? 0 : 1;
  for (let offset = firstOffset; offset < options.length + firstOffset; offset += 1) {
    const index = (current + offset + options.length) % options.length;
    const option = options[index];
    if (!option?.disabled && option.label.toLocaleLowerCase().startsWith(needle)) return index;
  }
  return -1;
}

export function optionId(listboxId: string, index: number) {
  return `${listboxId}-option-${index}`;
}

export function resolveMenuLayout(
  rect: DOMRect,
  menu: HTMLDivElement,
  optionCount: number,
  viewport: Pick<Window, "innerWidth" | "innerHeight"> = window,
): MenuLayout {
  const viewportWidth = viewport.innerWidth;
  const viewportHeight = viewport.innerHeight;
  const viewportMaxHeight = Math.max(0, viewportHeight - EDGE_GAP * 2);
  const estimatedHeight = Math.max(40, optionCount * OPTION_HEIGHT + 8);
  const borderHeight = Math.max(
    0,
    (menu.offsetHeight ?? 0) - (menu.clientHeight ?? 0),
  );
  const naturalHeight = Math.max(
    40,
    (menu.scrollHeight || estimatedHeight) + borderHeight,
  );
  const desiredHeight = Math.min(naturalHeight, SELECT_MENU_MAX_HEIGHT, viewportMaxHeight);
  const availableWidth = Math.max(0, viewportWidth - EDGE_GAP * 2);
  const width = Math.min(Math.max(rect.width, 96), availableWidth);
  const left = Math.min(Math.max(rect.left, EDGE_GAP), viewportWidth - EDGE_GAP - width);
  const belowSpace = Math.max(0, viewportHeight - rect.bottom - MENU_GAP - EDGE_GAP);
  const aboveSpace = Math.max(0, rect.top - MENU_GAP - EDGE_GAP);
  const openAbove = belowSpace < desiredHeight && aboveSpace > belowSpace;
  const availableHeight = openAbove ? aboveSpace : belowSpace;
  const maxHeight = Math.min(naturalHeight, SELECT_MENU_MAX_HEIGHT, availableHeight);
  const menuHeight = maxHeight;
  const preferredTop = openAbove ? rect.top - MENU_GAP - menuHeight : rect.bottom + MENU_GAP;
  const maxTop = Math.max(EDGE_GAP, viewportHeight - EDGE_GAP - menuHeight);
  const top = Math.min(Math.max(preferredTop, EDGE_GAP), maxTop);
  return { left, top, width, maxHeight };
}
