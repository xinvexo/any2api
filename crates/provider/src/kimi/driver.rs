use any2api_domain::{
    ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind, TransportMode,
};
use any2api_protocol::api::{
    OpenAiChatCachedTokensField, OpenAiChatCompletionsProfile, OpenAiChatCustomToolMode,
    OpenAiChatInstructionRole, OpenAiChatReasoningRequest, OpenAiChatReasoningResponse,
    OpenAiChatRequestField, OpenAiChatRequestFields, OpenAiChatTokenLimitField,
    OpenAiChatToolNamePolicy, ProtocolTargetProfile,
};
use http::HeaderMap;

use super::{headers as kimi_headers, upstream_error as kimi_error};
use crate::{
    ProviderError, ProviderSecret,
    api::{
        CredentialHeaders, CredentialTestPlan, EndpointPlan, ProviderDescriptor, ProviderDriver,
        UpstreamResponseMeta,
    },
    credential::api_key,
};

const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
    ProviderKind::Kimi,
    &[ProtocolOperation::ChatCompletions],
    None,
    &[TransportMode::Json, TransportMode::Sse],
);

#[derive(Debug)]
pub struct KimiDriver;

impl Default for KimiDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl KimiDriver {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ProviderDriver for KimiDriver {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn protocol_target_profile(
        &self,
        dialect: ProtocolDialect,
        upstream_model: &str,
    ) -> Option<ProtocolTargetProfile> {
        (dialect == ProtocolDialect::OpenAiChatCompletions).then_some(
            ProtocolTargetProfile::OpenAiChatCompletions(kimi_chat_profile(upstream_model)),
        )
    }

    fn validate_credential(&self, secret: &ProviderSecret) -> Result<(), ProviderError> {
        api_key::validate_secret(secret)
    }

    fn endpoint_plan(
        &self,
        base_url: &ProviderBaseUrl,
        operation: ProtocolOperation,
    ) -> Result<EndpointPlan, ProviderError> {
        if operation != ProtocolOperation::ChatCompletions {
            return Err(ProviderError::InvalidEndpoint(
                "operation is not supported by Kimi".into(),
            ));
        }
        Ok(EndpointPlan {
            url: api_key::endpoint_url(base_url, operation)?,
        })
    }

    fn credential_test_plan(
        &self,
        base_url: &ProviderBaseUrl,
    ) -> Result<CredentialTestPlan, ProviderError> {
        Ok(CredentialTestPlan {
            url: api_key::credential_test_url(base_url)?,
            headers: HeaderMap::new(),
        })
    }

    fn parse_model_catalog(&self, bounded_body: &[u8]) -> Result<Vec<String>, ProviderError> {
        api_key::parse_model_catalog(bounded_body)
    }

    fn credential_headers(
        &self,
        _base_url: &ProviderBaseUrl,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError> {
        api_key::bearer_credential_headers(secret)
    }

    fn response_headers(&self, _operation: ProtocolOperation, upstream: &HeaderMap) -> HeaderMap {
        kimi_headers::response(upstream)
    }

    fn classify_error(
        &self,
        _operation: ProtocolOperation,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> any2api_domain::UpstreamError {
        kimi_error::classify(meta, bounded_body)
    }
}

fn kimi_chat_profile(upstream_model: &str) -> OpenAiChatCompletionsProfile {
    let request_fields = OpenAiChatRequestFields::NONE
        .with(OpenAiChatRequestField::Logprobs)
        .with(OpenAiChatRequestField::PromptCacheKey)
        .with(OpenAiChatRequestField::ResponseFormat)
        .with(OpenAiChatRequestField::Stop)
        .with(OpenAiChatRequestField::TopLogprobs);
    OpenAiChatCompletionsProfile {
        token_limit_field: OpenAiChatTokenLimitField::MaxCompletionTokens,
        instruction_role: OpenAiChatInstructionRole::System,
        reasoning_request: if upstream_model == "kimi-k3" {
            OpenAiChatReasoningRequest::ReasoningEffort
        } else {
            OpenAiChatReasoningRequest::Unsupported
        },
        reasoning_response: OpenAiChatReasoningResponse::ReasoningContent,
        cached_tokens_field: OpenAiChatCachedTokensField::TopLevel,
        custom_tools: OpenAiChatCustomToolMode::FunctionEnvelope,
        tool_name_policy: OpenAiChatToolNamePolicy::new(64),
        request_fields,
        supports_image_url: true,
        supports_image_detail: false,
        supports_input_audio: false,
        supports_file: false,
    }
}
