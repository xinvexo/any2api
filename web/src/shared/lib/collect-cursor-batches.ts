interface CursorBatch<T> {
  items: T[];
  nextCursor: string | null;
}

interface CollectedCursorBatches<TBatch> {
  pages: TBatch[];
  pageParams: Array<string | null>;
}

export async function collectCursorBatches<
  TItem,
  TBatch extends CursorBatch<TItem>,
>(
  fetchBatch: (cursor: string | null) => Promise<TBatch>,
  knownIds: ReadonlySet<string>,
  getId: (item: TItem) => string,
  maxBatches: number,
  maxItems = Number.POSITIVE_INFINITY,
): Promise<CollectedCursorBatches<TBatch>> {
  const pages: TBatch[] = [];
  const pageParams: Array<string | null> = [];
  const visitedCursors = new Set<string | null>();
  let itemCount = 0;
  let cursor: string | null = null;

  while (pages.length < maxBatches && itemCount < maxItems) {
    const pageParam = cursor;
    if (visitedCursors.has(pageParam)) {
      break;
    }
    visitedCursors.add(pageParam);
    const batch = await fetchBatch(pageParam);
    pages.push(batch);
    pageParams.push(pageParam);
    itemCount += batch.items.length;
    if (batch.items.some((item) => knownIds.has(getId(item)))) {
      break;
    }
    cursor = batch.nextCursor;
    if (cursor === null) {
      break;
    }
  }

  return { pages, pageParams };
}
