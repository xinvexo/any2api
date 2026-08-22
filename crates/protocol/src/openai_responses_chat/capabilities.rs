use any2api_domain::ProtocolOperation;

use crate::api::{
    BridgeLimitation, BridgeRequestFieldBehavior, BridgeRequestFieldCapability,
    ProtocolBridgeCapabilities,
};

use BridgeRequestFieldBehavior::{Forwarded, LocalState, Translated, ValidatedOnly};

const REQUEST_FIELDS: &[BridgeRequestFieldCapability] = &[
    BridgeRequestFieldCapability::new("model", Translated),
    BridgeRequestFieldCapability::new("input", Translated),
    BridgeRequestFieldCapability::new("instructions", Translated),
    BridgeRequestFieldCapability::new("previous_response_id", LocalState),
    BridgeRequestFieldCapability::new("include", ValidatedOnly),
    BridgeRequestFieldCapability::new("stream", Translated),
    BridgeRequestFieldCapability::new("max_output_tokens", Translated),
    BridgeRequestFieldCapability::new("reasoning", Translated),
    BridgeRequestFieldCapability::new("text", Translated),
    BridgeRequestFieldCapability::new("tools", Translated),
    BridgeRequestFieldCapability::new("tool_choice", Translated),
    BridgeRequestFieldCapability::new("client_metadata", ValidatedOnly),
    BridgeRequestFieldCapability::new("n", ValidatedOnly),
    BridgeRequestFieldCapability::new("frequency_penalty", Forwarded),
    BridgeRequestFieldCapability::new("logit_bias", Forwarded),
    BridgeRequestFieldCapability::new("logprobs", Forwarded),
    BridgeRequestFieldCapability::new("metadata", Forwarded),
    BridgeRequestFieldCapability::new("parallel_tool_calls", Forwarded),
    BridgeRequestFieldCapability::new("presence_penalty", Forwarded),
    BridgeRequestFieldCapability::new("prompt_cache_key", Forwarded),
    BridgeRequestFieldCapability::new("seed", Forwarded),
    BridgeRequestFieldCapability::new("service_tier", Forwarded),
    BridgeRequestFieldCapability::new("stop", Forwarded),
    BridgeRequestFieldCapability::new("store", Forwarded),
    BridgeRequestFieldCapability::new("temperature", Forwarded),
    BridgeRequestFieldCapability::new("top_logprobs", Forwarded),
    BridgeRequestFieldCapability::new("top_p", Forwarded),
    BridgeRequestFieldCapability::new("user", Forwarded),
];

const LIMITATIONS: &[BridgeLimitation] = &[
    BridgeLimitation::new(
        "canonical_request_reconstruction",
        "The Chat Completions request is reconstructed; source JSON ordering and whitespace are not preserved.",
    ),
    BridgeLimitation::new(
        "unknown_fields_rejected",
        "Top-level request fields not listed by this contract are rejected before upstream I/O.",
    ),
    BridgeLimitation::new(
        "single_choice",
        "The request supports exactly one response choice; n must be absent or equal to 1.",
    ),
    BridgeLimitation::new(
        "client_executed_tools_only",
        "Function, custom, namespace, and tool_search are projected only when execution remains client-side; hosted tools have no generic Chat Completions equivalent.",
    ),
    BridgeLimitation::new(
        "target_profile_dependent",
        "Token fields, instruction roles, optional request fields, multimodal parts, reasoning wire fields, custom tools, and tool names follow the selected Provider's declared Chat target profile.",
    ),
    BridgeLimitation::new(
        "validated_client_metadata_not_forwarded",
        "client_metadata is validated as string values but has no Chat Completions wire equivalent.",
    ),
    BridgeLimitation::new(
        "local_continuation",
        "previous_response_id is backed by process-local continuation state and is not sent upstream.",
    ),
    BridgeLimitation::new(
        "synthetic_responses_identity",
        "Response IDs, item IDs, timestamps, and Responses SSE lifecycle events are synthesized locally.",
    ),
];

pub(super) static CAPABILITIES: ProtocolBridgeCapabilities = ProtocolBridgeCapabilities {
    contract_id: "openai-responses-to-chat-completions/v2",
    operations: &[ProtocolOperation::Responses],
    request_fields: REQUEST_FIELDS,
    tool_types: &["function", "custom", "namespace", "tool_search"],
    limitations: LIMITATIONS,
};
