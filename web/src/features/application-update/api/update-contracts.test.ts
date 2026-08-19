import { expect, test } from "vitest";

import {
  OFFICIAL_REPOSITORY_URL,
  parseApplicationAbout,
  parseApplicationHealth,
  parseApplicationHealthVersion,
  parseUpdateCheckResult,
  parseUpdateStatus,
} from "./update-contracts";

test("parses the fixed application repository", () => {
  expect(parseApplicationAbout({
    current_version: "1.2.3",
    repository_url: OFFICIAL_REPOSITORY_URL,
  })).toEqual({
    currentVersion: "1.2.3",
    repositoryUrl: OFFICIAL_REPOSITORY_URL,
  });
  expect(() => parseApplicationAbout({
    current_version: "1.2.3",
    repository_url: "https://example.com/other",
  })).toThrow("invalid application update response");
});

test("parses only the release URL derived from the latest version", () => {
  expect(parseUpdateCheckResult({
    current_version: "1.2.3",
    latest_version: "1.3.0",
    update_available: true,
    release_url: `${OFFICIAL_REPOSITORY_URL}/releases/tag/v1.3.0`,
    published_at: "2026-07-29T00:00:00Z",
  }).latestVersion).toBe("1.3.0");
  expect(() => parseUpdateCheckResult({
    current_version: "1.2.3",
    latest_version: "1.3.0",
    update_available: true,
    release_url: `${OFFICIAL_REPOSITORY_URL}/releases/tag/v9.9.9`,
    published_at: null,
  })).toThrow("invalid application update response");
});

test("requires internally consistent update progress", () => {
  expect(parseUpdateStatus(status({
    phase: "downloading",
    target_version: "1.3.0",
    downloaded_bytes: 512,
    total_bytes: 1024,
  }))).toEqual({
    phase: "downloading",
    targetVersion: "1.3.0",
    downloadedBytes: 512,
    totalBytes: 1024,
  });
  expect(parseUpdateStatus(status({
    phase: "failed",
    target_version: "1.3.0",
    failure_code: "update_verification_failed",
  }))).toEqual({
    phase: "failed",
    targetVersion: "1.3.0",
    failureCode: "update_verification_failed",
  });
  expect(() => parseUpdateStatus(status({
    phase: "downloading",
    target_version: "1.3.0",
    downloaded_bytes: 1025,
    total_bytes: 1024,
  }))).toThrow("invalid application update response");
});

test("reads the running build version from health", () => {
  const health = {
    status: "ok",
    application_version: "1.3.0",
    instance_id: "550e8400-e29b-41d4-a716-446655440000",
  };
  expect(parseApplicationHealth(health)).toEqual({
    applicationVersion: "1.3.0",
    instanceId: "550e8400-e29b-41d4-a716-446655440000",
  });
  expect(parseApplicationHealthVersion(health)).toBe("1.3.0");
  expect(() => parseApplicationHealth({ ...health, application_version: "latest" })).toThrow(
    "invalid application update response",
  );
  expect(() => parseApplicationHealth({ ...health, instance_id: "not-a-uuid" })).toThrow(
    "invalid application update response",
  );
});

function status(overrides: Record<string, unknown>) {
  return {
    phase: "idle",
    target_version: null,
    downloaded_bytes: null,
    total_bytes: null,
    failure_code: null,
    ...overrides,
  };
}
