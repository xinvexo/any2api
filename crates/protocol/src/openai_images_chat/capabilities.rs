use any2api_domain::ProtocolOperation;

use crate::api::{
    BridgeLimitation, BridgeRequestFieldBehavior, BridgeRequestFieldCapability,
    ProtocolBridgeCapabilities,
};

use BridgeRequestFieldBehavior::{Forwarded, Translated, ValidatedOnly};

const REQUEST_FIELDS: &[BridgeRequestFieldCapability] = &[
    BridgeRequestFieldCapability::new("background", Forwarded),
    BridgeRequestFieldCapability::new("model", Translated),
    BridgeRequestFieldCapability::new("moderation", Forwarded),
    BridgeRequestFieldCapability::new("n", Forwarded),
    BridgeRequestFieldCapability::new("output_compression", Forwarded),
    BridgeRequestFieldCapability::new("output_format", Forwarded),
    BridgeRequestFieldCapability::new("partial_images", ValidatedOnly),
    BridgeRequestFieldCapability::new("prompt", Translated),
    BridgeRequestFieldCapability::new("quality", Forwarded),
    BridgeRequestFieldCapability::new("response_format", ValidatedOnly),
    BridgeRequestFieldCapability::new("size", Forwarded),
    BridgeRequestFieldCapability::new("stream", Translated),
    BridgeRequestFieldCapability::new("style", Forwarded),
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
        "generations_only",
        "Only images_generations is supported; images_edits cannot be represented.",
    ),
    BridgeLimitation::new(
        "non_streaming",
        "Streaming and partial image delivery are not supported.",
    ),
    BridgeLimitation::new(
        "url_responses_only",
        "The upstream Chat response must contain one unique HTTP(S) image URL per choice.",
    ),
];

pub(super) static CAPABILITIES: ProtocolBridgeCapabilities = ProtocolBridgeCapabilities {
    contract_id: "openai-images-to-chat-completions/v1",
    operations: &[ProtocolOperation::ImagesGenerations],
    request_fields: REQUEST_FIELDS,
    tool_types: &[],
    limitations: LIMITATIONS,
};
