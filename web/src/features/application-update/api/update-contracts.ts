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

export type UpdateFailureCode =
  | "update_unsupported"
  | "update_not_available"
  | "update_in_progress"
  | "update_check_failed"
  | "update_download_failed"
  | "update_verification_failed"
  | "update_install_failed";

export type UpdateStatus =
  | { phase: "idle" | "checking" }
  | {
      phase: "downloading";
      targetVersion: string;
      downloadedBytes: number;
      totalBytes: number;
    }
  | { phase: "installing" | "restarting"; targetVersion: string }
  | {
      phase: "failed";
      targetVersion: string | null;
      failureCode: UpdateFailureCode;
    };

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

export function parseUpdateStatus(value: unknown): UpdateStatus {
  const record = readRecord(value);
  const phase = readString(record.phase);
  if (phase === "idle" || phase === "checking") {
    requireNull(record.target_version, record.downloaded_bytes, record.total_bytes, record.failure_code);
    return { phase };
  }
  if (phase === "downloading") {
    const downloadedBytes = readUnsignedInteger(record.downloaded_bytes);
    const totalBytes = readPositiveInteger(record.total_bytes);
    requireNull(record.failure_code);
    if (downloadedBytes > totalBytes) {
      throw invalidResponse();
    }
    return {
      phase,
      targetVersion: readVersion(record.target_version),
      downloadedBytes,
      totalBytes,
    };
  }
  if (phase === "installing" || phase === "restarting") {
    requireNull(record.downloaded_bytes, record.total_bytes, record.failure_code);
    return { phase, targetVersion: readVersion(record.target_version) };
  }
  if (phase === "failed") {
    requireNull(record.downloaded_bytes, record.total_bytes);
    return {
      phase,
      targetVersion: readNullableVersion(record.target_version),
      failureCode: readFailureCode(record.failure_code),
    };
  }
  throw invalidResponse();
}

export function parseApplicationHealthVersion(value: unknown) {
  return readVersion(readRecord(value).application_version);
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

function readUnsignedInteger(value: unknown) {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw invalidResponse();
  }
  return value;
}

function readPositiveInteger(value: unknown) {
  const number = readUnsignedInteger(value);
  if (number === 0) {
    throw invalidResponse();
  }
  return number;
}

function readVersion(value: unknown) {
  const version = readString(value);
  if (!SEMVER.test(version)) {
    throw invalidResponse();
  }
  return version;
}

function readNullableVersion(value: unknown) {
  return value === null ? null : readVersion(value);
}

function readFailureCode(value: unknown): UpdateFailureCode {
  const code = readString(value);
  if (!FAILURE_CODES.has(code as UpdateFailureCode)) {
    throw invalidResponse();
  }
  return code as UpdateFailureCode;
}

function requireNull(...values: unknown[]) {
  if (values.some((value) => value !== null)) {
    throw invalidResponse();
  }
}

const FAILURE_CODES = new Set<UpdateFailureCode>([
  "update_unsupported",
  "update_not_available",
  "update_in_progress",
  "update_check_failed",
  "update_download_failed",
  "update_verification_failed",
  "update_install_failed",
]);
const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;

function invalidResponse() {
  return new Error("invalid application update response");
}
