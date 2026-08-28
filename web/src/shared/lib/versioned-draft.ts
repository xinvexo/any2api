export interface DraftSourceVersion {
  configRevision: number;
  entityVersion?: number;
}

export interface VersionedDraft<T> {
  source: DraftSourceVersion;
  value: T;
}

export function draftSourceChanged(
  source: DraftSourceVersion,
  current: DraftSourceVersion,
) {
  return source.configRevision !== current.configRevision
    || source.entityVersion !== current.entityVersion;
}
