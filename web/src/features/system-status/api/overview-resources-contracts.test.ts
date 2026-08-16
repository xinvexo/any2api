import { expect, test } from "vitest";

import { parseOverviewResources } from "./overview-resources-contracts";

const resources = () => ({
  sampled_at_ms: 1_726_800_000_000,
  process: { resident_memory_bytes: 134_217_728, cpu_usage_percent: 2.4 },
  system: {
    used_memory_bytes: 8_589_934_592,
    total_memory_bytes: 17_179_869_184,
    cpu_usage_percent: 31.7,
  },
});

test("parses process and system resource snapshots", () => {
  expect(parseOverviewResources(resources())).toEqual({
    sampledAtMs: 1_726_800_000_000,
    process: { residentMemoryBytes: 134_217_728, cpuUsagePercent: 2.4 },
    system: {
      usedMemoryBytes: 8_589_934_592,
      totalMemoryBytes: 17_179_869_184,
      cpuUsagePercent: 31.7,
    },
  });
});

test("rejects unsafe, inconsistent, and out-of-range resource values", () => {
  const invalid = resources();
  invalid.system.used_memory_bytes = invalid.system.total_memory_bytes + 1;
  expect(() => parseOverviewResources(invalid)).toThrow("invalid overview resources response");

  const unsafe = resources();
  unsafe.process.resident_memory_bytes = Number.MAX_SAFE_INTEGER + 1;
  expect(() => parseOverviewResources(unsafe)).toThrow("invalid overview resources response");

  const invalidCpu = resources();
  invalidCpu.system.cpu_usage_percent = 101;
  expect(() => parseOverviewResources(invalidCpu)).toThrow("invalid overview resources response");
});
