import type { OverviewUsageResponse } from "@/shared/api/generated/OverviewUsageResponse";

import {
  OVERVIEW_USAGE_RANGE_SPECS,
  type OverviewUsageRange,
} from "./overview-usage-contracts";

export function overviewUsageWire(range: OverviewUsageRange = "24h"): OverviewUsageResponse {
  const spec = OVERVIEW_USAGE_RANGE_SPECS[range];
  const rangeStartedAtMs = 1_700_000_000_000;
  const rangeEndedAtMs = rangeStartedAtMs + spec.bucketCount * spec.bucketWidthMs;
  const buckets = Array.from({ length: spec.bucketCount }, (_, index) => ({
    started_at_ms: rangeStartedAtMs + index * spec.bucketWidthMs,
    ended_at_ms: rangeStartedAtMs + (index + 1) * spec.bucketWidthMs,
    request_count: index === 0 || index === spec.bucketCount - 1 ? 1 : 0,
    successful_request_count: index === 0 ? 1 : 0,
    failed_request_count: index === spec.bucketCount - 1 ? 1 : 0,
  }));
  return {
    generated_at_ms: rangeEndedAtMs - 1,
    range,
    range_started_at_ms: rangeStartedAtMs,
    range_ended_at_ms: rangeEndedAtMs,
    retained_started_at_ms: rangeStartedAtMs - 86_400_000,
    retained: {
      request_count: 3,
      successful_request_count: 2,
      failed_request_count: 1,
      token_usage_request_count: 3,
      input_tokens: "9007199254740993",
      output_tokens: "7",
      total_tokens: "9007199254741000",
    },
    selected: {
      request_count: 2,
      successful_request_count: 1,
      failed_request_count: 1,
      token_usage_request_count: 2,
      input_tokens: "10",
      output_tokens: "5",
      total_tokens: "15",
    },
    time_buckets: buckets,
    models: [
      {
        public_model: "gpt-test",
        is_other: false,
        request_count: 1,
        successful_request_count: 1,
        failed_request_count: 0,
        token_usage_request_count: 1,
        input_tokens: "10",
        output_tokens: "0",
        total_tokens: "10",
      },
      {
        public_model: null,
        is_other: false,
        request_count: 1,
        successful_request_count: 0,
        failed_request_count: 1,
        token_usage_request_count: 1,
        input_tokens: "0",
        output_tokens: "5",
        total_tokens: "5",
      },
    ],
  };
}
