import { describe, expect, it } from "vitest";

import { parseRequestLogDetail, parseRequestLogList } from "./request-log-contracts";

describe("request log contracts", () => {
  it("parses list metrics and a detail attempt timeline", () => {
    const list = parseRequestLogList({
      items: [request()],
      telemetry: telemetry(),
    });
    expect(list.items[0]?.publicModel).toBe("codex-local");
    expect(list.telemetry.droppedRecords).toBe(2);

    const detail = parseRequestLogDetail({
      request: request(),
      attempts: [
        {
          attempt_no: 1,
          route_target_id: "target-1",
          credential_id: "credential-1",
          proxy_profile_id: "proxy-1",
          started_at_ms: 1_700_000_000_001,
          duration_ms: 25,
          retry_safety: "ambiguous",
          error_class: null,
          status_code: 200,
          outcome: "success",
        },
      ],
      telemetry: telemetry(),
    });
    expect(detail.attempts[0]?.outcome).toBe("success");
    expect(detail.request.firstTokenMs).toBe(18);
    expect(detail.request.inputTokens).toBe(120);
    expect(detail.request.outputTokens).toBe(45);
    expect(detail.request.cacheReadTokens).toBe(30);
    expect(detail.request.cacheWriteTokens).toBe(6);
  });

  it("rejects unknown outcomes and invalid status codes", () => {
    expect(() =>
      parseRequestLogDetail({
        request: { ...request(), status_code: 99 },
        attempts: [],
        telemetry: telemetry(),
      }),
    ).toThrow("invalid request log response");
    expect(() =>
      parseRequestLogDetail({
        request: request(),
        attempts: [
          {
            attempt_no: 1,
            route_target_id: null,
            credential_id: null,
            proxy_profile_id: null,
            started_at_ms: 1,
            duration_ms: 1,
            retry_safety: null,
            error_class: null,
            status_code: null,
            outcome: "guessed",
          },
        ],
        telemetry: telemetry(),
      }),
    ).toThrow("invalid request log response");
  });

  it("accepts the largest lossless token count", () => {
    const list = parseRequestLogList({
      items: [{ ...request(), input_tokens: Number.MAX_SAFE_INTEGER }],
      telemetry: telemetry(),
    });

    expect(list.items[0]?.inputTokens).toBe(Number.MAX_SAFE_INTEGER);
  });

  it("accepts Chat Completions request logs", () => {
    const list = parseRequestLogList({
      items: [
        {
          ...request(),
          ingress_protocol: "openai_chat_completions",
          operation: "chat_completions",
        },
      ],
      telemetry: telemetry(),
    });

    expect(list.items[0]?.ingressProtocol).toBe("openai_chat_completions");
    expect(list.items[0]?.operation).toBe("chat_completions");
  });
});

function request() {
  return {
    request_id: "11111111-1111-4111-8111-111111111111",
    started_at_ms: 1_700_000_000_000,
    config_revision: 9,
    gateway_api_key_id: "22222222-2222-4222-8222-222222222222",
    ingress_protocol: "openai_responses",
    operation: "responses",
    public_model: "codex-local",
    provider_endpoint_id: "33333333-3333-4333-8333-333333333333",
    credential_id: "44444444-4444-4444-8444-444444444444",
    proxy_profile_id: "00000000-0000-0000-0000-000000000000",
    status_code: 200,
    error_class: null,
    attempt_count: 1,
    latency_ms: 30,
    first_token_ms: 18,
    input_tokens: 120,
    output_tokens: 45,
    cache_read_tokens: 30,
    cache_write_tokens: 6,
    is_stream: true,
  };
}

function telemetry() {
  return {
    queued_records: 1,
    dropped_records: 2,
    persisted_records: 3,
  };
}
