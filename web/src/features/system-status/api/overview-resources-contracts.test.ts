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
  ownership: {
    payload_buffers: {
      heap_current_bytes: 1_048_576,
      heap_peak_bytes: 2_097_152,
      mapped_current_bytes: 8_388_608,
      mapped_peak_bytes: 16_777_216,
      http_body_capture_current_bytes: 524_288,
      http_body_capture_peak_bytes: 1_048_576,
    },
    telemetry: {
      queued_owned_bytes: 262_144,
      in_flight_owned_bytes: 131_072,
      reserved_owned_bytes: 393_216,
    },
    reclamation: { blockers: 1, completed_runs: 12, last_duration_micros: 725 },
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
    ownership: {
      payloadBuffers: {
        heapCurrentBytes: 1_048_576,
        heapPeakBytes: 2_097_152,
        mappedCurrentBytes: 8_388_608,
        mappedPeakBytes: 16_777_216,
        httpBodyCaptureCurrentBytes: 524_288,
        httpBodyCapturePeakBytes: 1_048_576,
      },
      telemetry: {
        queuedOwnedBytes: 262_144,
        inFlightOwnedBytes: 131_072,
        reservedOwnedBytes: 393_216,
      },
      reclamation: { blockers: 1, completedRuns: 12, lastDurationMicros: 725 },
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
