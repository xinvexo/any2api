export const OFFICIAL_REPOSITORY_URL = "https://github.com/xinvexo/any2api";

export interface ApplicationAbout {
  currentVersion: string;
  repositoryUrl: string;
}

export interface UpdateCheckResult {
  currentVersion: string;
  latestVersion: string;
  updateAvailable: boolean;
  releaseUrl: string;
  publishedAt: string | null;
}

export interface UpdateInstallResult {
  installedVersion: string;
  restartRequested: boolean;
}

export function parseApplicationAbout(value: unknown): ApplicationAbout {
  const record = readRecord(value);
  const repositoryUrl = readString(record.repository_url);
  if (repositoryUrl !== OFFICIAL_REPOSITORY_URL) {
    throw invalidResponse();
  }
  return {
    currentVersion: readVersion(record.current_version),
    repositoryUrl,
  };
}

export function parseUpdateCheckResult(value: unknown): UpdateCheckResult {
  const record = readRecord(value);
  const latestVersion = readVersion(record.latest_version);
  const expectedReleaseUrl = `${OFFICIAL_REPOSITORY_URL}/releases/tag/v${latestVersion}`;
  const releaseUrl = readString(record.release_url);
  if (releaseUrl !== expectedReleaseUrl) {
    throw invalidResponse();
  }
  return {
    currentVersion: readVersion(record.current_version),
    latestVersion,
    updateAvailable: readBoolean(record.update_available),
    releaseUrl,
    publishedAt: readNullableString(record.published_at),
  };
}

export function parseUpdateInstallResult(value: unknown): UpdateInstallResult {
  const record = readRecord(value);
  const restartRequested = readBoolean(record.restart_requested);
  if (!restartRequested) {
    throw invalidResponse();
  }
  return {
    installedVersion: readVersion(record.installed_version),
    restartRequested,
  };
}

function readRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null) {
    throw invalidResponse();
  }
  return value as Record<string, unknown>;
}

function readString(value: unknown) {
  if (typeof value !== "string" || value.length === 0) {
    throw invalidResponse();
  }
  return value;
}

function readNullableString(value: unknown) {
  return value === null ? null : readString(value);
}

function readBoolean(value: unknown) {
  if (typeof value !== "boolean") {
    throw invalidResponse();
  }
  return value;
}

function readVersion(value: unknown) {
  const version = readString(value);
  if (!SEMVER.test(version)) {
    throw invalidResponse();
  }
  return version;
}

const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;

function invalidResponse() {
  return new Error("invalid application update response");
}
