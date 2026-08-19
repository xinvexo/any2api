export interface OverviewResources {
  sampledAtMs: number;
  process: {
    residentMemoryBytes: number;
    cpuUsagePercent: number;
  };
  system: {
    usedMemoryBytes: number;
    totalMemoryBytes: number;
    cpuUsagePercent: number;
  };
  ownership: {
    payloadBuffers: {
      heapCurrentBytes: number;
      heapPeakBytes: number;
      mappedCurrentBytes: number;
      mappedPeakBytes: number;
      httpBodyCaptureCurrentBytes: number;
      httpBodyCapturePeakBytes: number;
    };
  };
}

export function parseOverviewResources(value: unknown): OverviewResources {
  const root = record(value);
  const process = record(root.process);
  const system = record(root.system);
  const ownership = record(root.ownership);
  const payloadBuffers = record(ownership.payload_buffers);
  const usedMemoryBytes = integer(system.used_memory_bytes);
  const totalMemoryBytes = positive(system.total_memory_bytes);
  if (usedMemoryBytes > totalMemoryBytes) throw invalid();
  return {
    sampledAtMs: integer(root.sampled_at_ms),
    process: {
      residentMemoryBytes: integer(process.resident_memory_bytes),
      cpuUsagePercent: percent(process.cpu_usage_percent),
    },
    system: {
      usedMemoryBytes,
      totalMemoryBytes,
      cpuUsagePercent: percent(system.cpu_usage_percent),
    },
    ownership: {
      payloadBuffers: {
        heapCurrentBytes: integer(payloadBuffers.heap_current_bytes),
        heapPeakBytes: integer(payloadBuffers.heap_peak_bytes),
        mappedCurrentBytes: integer(payloadBuffers.mapped_current_bytes),
        mappedPeakBytes: integer(payloadBuffers.mapped_peak_bytes),
        httpBodyCaptureCurrentBytes: integer(
          payloadBuffers.http_body_capture_current_bytes,
        ),
        httpBodyCapturePeakBytes: integer(payloadBuffers.http_body_capture_peak_bytes),
      },
    },
  };
}

function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw invalid();
  return value as Record<string, unknown>;
}

function integer(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) throw invalid();
  return value;
}

function positive(value: unknown): number {
  const result = integer(value);
  if (result === 0) throw invalid();
  return result;
}

function percent(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 100) {
    throw invalid();
  }
  return value;
}

function invalid() {
  return new Error("invalid overview resources response");
}
