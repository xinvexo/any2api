import { expect, test } from "vitest";

import {
  OFFICIAL_REPOSITORY_URL,
  parseApplicationAbout,
  parseUpdateCheckResult,
  parseUpdateInstallResult,
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

test("requires installation to request a restart", () => {
  expect(parseUpdateInstallResult({
    installed_version: "1.3.0",
    restart_requested: true,
  })).toEqual({ installedVersion: "1.3.0", restartRequested: true });
  expect(() => parseUpdateInstallResult({
    installed_version: "1.3.0",
    restart_requested: false,
  })).toThrow("invalid application update response");
});
