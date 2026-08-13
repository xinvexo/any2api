import { describe, expect, it } from "vitest";

import { parseRequestLogDetail, parseRequestLogList } from "./request-log-contracts";

describe("request log contracts", () => {
  it("parses list metrics and a detail attempt timeline", () => {
    const list = parseRequestLogList(requestLogPage([request()]));
    expect(list.items[0]?.publicModel).toBe("codex-local");
    expect(list.items[0]?.clientIp).toBe("203.0.113.8");
    expect(list.items[0]?.providerEndpointName).toBe("frapi");
    expect(list.telemetry.inFlightRecords).toBe(4);
    expect(list.telemetry.droppedRecords).toBe(2);

    const detail = parseRequestLogDetail({
      request: request(),
      attempts: [
        {
          attempt_no: 1,
          route_target_id: "target-1",
          credential_id: "credential-1",
          credential_label: "Primary credential",
          oauth_account_id: null,
          oauth_account_label: null,
          proxy_profile_id: "proxy-1",
          proxy_profile_label: "Primary proxy",
          routing_mode: "balanced",
          failure_scope: "authentication",
          retry_decision: "reselect",
          started_at_ms: 1_700_000_000_001,
          duration_ms: 25,
          error_message: null,
          status_code: 200,
          outcome: "success",
          transport: {
            wire_profile_id: "generic-rustls-hyper-v2",
            wire_profile_version: 2,
            timeout_policy_version: 1,
            resolver_mode: "system",
            proxy_kind: "direct",
            connect_timeout_ms: 10_000,
            read_timeout_ms: 300_000,
            pool_idle_timeout_ms: 50_000,
            routing_generation: 4,
            authentication_version: 7,
            traffic_class: "data_plane",
          },
          stream_timing: {
            first_upstream_frame_ms: 8,
            stream_commit_ms: 9,
            first_downstream_byte_ms: 11,
            stream_cancel_ms: null,
          },
        },
      ],
      telemetry: telemetry(),
    });
    expect(detail.attempts[0]?.statusCode).toBe(200);
    expect(detail.attempts[0]).toMatchObject({
      routingMode: "balanced",
      failureScope: "authentication",
      retryDecision: "reselect",
    });
    expect(detail.attempts[0]?.transport).toMatchObject({
      wireProfileId: "generic-rustls-hyper-v2",
      resolverMode: "system",
      routingGeneration: 4,
    });
    expect(detail.attempts[0]?.streamTiming?.firstDownstreamByteMs).toBe(11);
    expect(detail.request.firstTokenMs).toBe(18);
    expect(detail.request.inputTokens).toBe(120);
    expect(detail.request.outputTokens).toBe(45);
    expect(detail.request.cacheReadTokens).toBe(30);
  });

  it("rejects invalid request and attempt status codes", () => {
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
            credential_label: null,
            oauth_account_id: "oauth-account-1",
            oauth_account_label: "Primary OAuth",
            proxy_profile_id: null,
            proxy_profile_label: null,
            routing_mode: "bound",
            failure_scope: null,
            retry_decision: null,
            started_at_ms: 1,
            duration_ms: 1,
            error_message: null,
            status_code: 600,
            outcome: "failed",
            transport: null,
            stream_timing: null,
          },
        ],
        telemetry: telemetry(),
      }),
    ).toThrow("invalid request log response");
  });

  it("accepts the largest lossless token count", () => {
    const list = parseRequestLogList(
      requestLogPage([{ ...request(), input_tokens: Number.MAX_SAFE_INTEGER }]),
    );

    expect(list.items[0]?.inputTokens).toBe(Number.MAX_SAFE_INTEGER);
  });

  it("accepts Chat Completions request logs", () => {
    const list = parseRequestLogList(
      requestLogPage([
        {
          ...request(),
          ingress_protocol: "openai_chat_completions",
          operation: "chat_completions",
        },
      ]),
    );

    expect(list.items[0]?.ingressProtocol).toBe("openai_chat_completions");
    expect(list.items[0]?.operation).toBe("chat_completions");
  });

  it("accepts Images generation and edit request logs", () => {
    for (const operation of ["images_generations", "images_edits"]) {
      const list = parseRequestLogList(
        requestLogPage([
          {
            ...request(),
            ingress_protocol: "openai_images",
            operation,
          },
        ]),
      );

      expect(list.items[0]?.ingressProtocol).toBe("openai_images");
      expect(list.items[0]?.operation).toBe(operation);
    }
  });

  it("parses request and attempt error messages", () => {
    const detail = parseRequestLogDetail({
      request: {
        ...request(),
        status_code: 401,
        outcome: "failed",
        error_message: "Incorrect API key provided",
      },
      attempts: [
        {
          attempt_no: 1,
          route_target_id: null,
          credential_id: "credential-1",
          credential_label: "Primary credential",
          oauth_account_id: null,
          oauth_account_label: null,
          proxy_profile_id: "proxy-1",
          proxy_profile_label: "Primary proxy",
          routing_mode: "balanced",
          failure_scope: "authentication",
          retry_decision: "terminal",
          started_at_ms: 1,
          duration_ms: 12,
          error_message: "Incorrect API key provided",
          status_code: 401,
          outcome: "failed",
          transport: null,
          stream_timing: null,
        },
      ],
      telemetry: telemetry(),
    });

    expect(detail.request.errorMessage).toBe("Incorrect API key provided");
    expect(detail.attempts[0]?.errorMessage).toBe("Incorrect API key provided");
  });

  it("keeps a 200 stream failure distinct from HTTP success", () => {
    const detail = parseRequestLogDetail({
      request: {
        ...request(),
        outcome: "failed",
        error_message: "upstream response stream reported a failure event",
      },
      attempts: [
        {
          attempt_no: 1,
          route_target_id: null,
          credential_id: "credential-1",
          credential_label: "Primary credential",
          oauth_account_id: null,
          oauth_account_label: null,
          proxy_profile_id: "proxy-1",
          proxy_profile_label: "Primary proxy",
          routing_mode: "bound",
          failure_scope: "exact_candidate",
          retry_decision: "terminal",
          started_at_ms: 1,
          duration_ms: 12,
          error_message: "upstream response stream reported a failure event",
          status_code: 200,
          outcome: "failed",
          transport: null,
          stream_timing: null,
        },
      ],
      telemetry: telemetry(),
    });

    expect(detail.request.statusCode).toBe(200);
    expect(detail.request.outcome).toBe("failed");
    expect(detail.attempts[0]?.outcome).toBe("failed");
  });

  it("rejects inconsistent page metadata", () => {
    expect(() =>
      parseRequestLogList({ ...requestLogPage([request()]), total: 0 }),
    ).toThrow("invalid request log response");
    expect(() =>
      parseRequestLogList({ ...requestLogPage([]), page_size: 101 }),
    ).toThrow("invalid request log response");
    expect(() =>
      parseRequestLogList({ ...requestLogPage([request()]), cursor: null }),
    ).toThrow("invalid request log response");
    expect(() =>
      parseRequestLogList({ ...requestLogPage([]), cursor: null, next_cursor: "r1.next" }),
    ).toThrow("invalid request log response");
  });

  it("rejects omitted fields from the current nullable contract", () => {
    const omittedRequestField = request() as Record<string, unknown>;
    delete omittedRequestField.credential_label;
    expect(() => parseRequestLogList(requestLogPage([omittedRequestField]))).toThrow(
      "invalid request log response",
    );

    const omittedAttemptField = attempt() as Record<string, unknown>;
    delete omittedAttemptField.oauth_account_label;
    expect(() =>
      parseRequestLogDetail({
        request: request(),
        attempts: [omittedAttemptField],
        telemetry: telemetry(),
      })
    ).toThrow("invalid request log response");
  });

  it("allows a cursor to advance across a page containing only corrupt persisted rows", () => {
    const page = parseRequestLogList({
      ...requestLogPage([]),
      total: 2,
      cursor: "r1.current",
      next_cursor: "r1.next",
    });

    expect(page.items).toEqual([]);
    expect(page.nextCursor).toBe("r1.next");
  });
});

function requestLogPage(items: unknown[]) {
  return {
    items,
    total: items.length,
    page_size: 20,
    cursor: items.length > 0 ? "r1.current" : null,
    next_cursor: null,
    telemetry: telemetry(),
  };
}

function request() {
  return {
    request_id: "11111111-1111-4111-8111-111111111111",
    started_at_ms: 1_700_000_000_000,
    client_ip: "203.0.113.8",
    config_revision: 9,
    gateway_api_key_id: "22222222-2222-4222-8222-222222222222",
    ingress_protocol: "openai_responses",
    operation: "responses",
    public_model: "codex-local",
    thinking_level: null,
    provider_endpoint_id: "33333333-3333-4333-8333-333333333333",
    provider_endpoint_name: "frapi",
    credential_id: "44444444-4444-4444-8444-444444444444",
    credential_label: "Primary credential",
    oauth_account_id: null,
    oauth_account_label: null,
    proxy_profile_id: "00000000-0000-0000-0000-000000000000",
    proxy_profile_label: "DIRECT",
    status_code: 200,
    outcome: "success",
    error_message: null,
    attempt_count: 1,
    latency_ms: 30,
    first_token_ms: 18,
    input_tokens: 120,
    output_tokens: 45,
    cache_read_tokens: 30,
    is_stream: true,
  };
}

function attempt() {
  return {
    attempt_no: 1,
    route_target_id: "target-1",
    credential_id: "credential-1",
    credential_label: "Primary credential",
    oauth_account_id: null,
    oauth_account_label: null,
    proxy_profile_id: "proxy-1",
    proxy_profile_label: "Primary proxy",
    routing_mode: "balanced",
    failure_scope: null,
    retry_decision: null,
    started_at_ms: 1,
    duration_ms: 1,
    error_message: null,
    status_code: 200,
    outcome: "success",
    transport: null,
    stream_timing: null,
  };
}

function telemetry() {
  return {
    queued_records: 1,
    in_flight_records: 4,
    dropped_records: 2,
    persisted_records: 3,
  };
}
