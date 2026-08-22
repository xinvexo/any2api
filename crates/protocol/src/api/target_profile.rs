#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolTargetProfile {
    OpenAiChatCompletions(OpenAiChatCompletionsProfile),
}

impl ProtocolTargetProfile {
    #[must_use]
    pub const fn openai_chat_completions(self) -> Option<OpenAiChatCompletionsProfile> {
        match self {
            Self::OpenAiChatCompletions(profile) => Some(profile),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenAiChatCompletionsProfile {
    pub token_limit_field: OpenAiChatTokenLimitField,
    pub instruction_role: OpenAiChatInstructionRole,
    pub reasoning_request: OpenAiChatReasoningRequest,
    pub reasoning_response: OpenAiChatReasoningResponse,
    pub cached_tokens_field: OpenAiChatCachedTokensField,
    pub custom_tools: OpenAiChatCustomToolMode,
    pub tool_name_policy: OpenAiChatToolNamePolicy,
    pub request_fields: OpenAiChatRequestFields,
    pub supports_image_url: bool,
    pub supports_image_detail: bool,
    pub supports_input_audio: bool,
    pub supports_file: bool,
}

impl OpenAiChatCompletionsProfile {
    pub const CURRENT_OPENAI: Self = Self {
        token_limit_field: OpenAiChatTokenLimitField::MaxCompletionTokens,
        instruction_role: OpenAiChatInstructionRole::Developer,
        reasoning_request: OpenAiChatReasoningRequest::ReasoningEffort,
        reasoning_response: OpenAiChatReasoningResponse::ReasoningContentOrReasoning,
        cached_tokens_field: OpenAiChatCachedTokensField::PromptTokensDetails,
        custom_tools: OpenAiChatCustomToolMode::Native,
        tool_name_policy: OpenAiChatToolNamePolicy::new(128),
        request_fields: OpenAiChatRequestFields::ALL,
        supports_image_url: true,
        supports_image_detail: true,
        supports_input_audio: true,
        supports_file: true,
    };

    pub const COMPATIBLE_BASELINE: Self = Self {
        token_limit_field: OpenAiChatTokenLimitField::MaxTokens,
        instruction_role: OpenAiChatInstructionRole::System,
        reasoning_request: OpenAiChatReasoningRequest::ReasoningEffort,
        reasoning_response: OpenAiChatReasoningResponse::ReasoningContentOrReasoning,
        cached_tokens_field: OpenAiChatCachedTokensField::PromptTokensDetailsOrTopLevel,
        custom_tools: OpenAiChatCustomToolMode::FunctionEnvelope,
        tool_name_policy: OpenAiChatToolNamePolicy::new(64),
        request_fields: OpenAiChatRequestFields::ALL,
        supports_image_url: true,
        supports_image_detail: true,
        supports_input_audio: false,
        supports_file: false,
    };

    #[must_use]
    pub const fn supports_request_field(self, field: OpenAiChatRequestField) -> bool {
        self.request_fields.contains(field)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenAiChatCachedTokensField {
    PromptTokensDetails,
    TopLevel,
    PromptTokensDetailsOrTopLevel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenAiChatTokenLimitField {
    MaxTokens,
    MaxCompletionTokens,
}

impl OpenAiChatTokenLimitField {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MaxTokens => "max_tokens",
            Self::MaxCompletionTokens => "max_completion_tokens",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenAiChatInstructionRole {
    System,
    Developer,
}

impl OpenAiChatInstructionRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Developer => "developer",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenAiChatReasoningRequest {
    Unsupported,
    ReasoningEffort,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenAiChatReasoningResponse {
    Unsupported,
    ReasoningContent,
    Reasoning,
    ReasoningContentOrReasoning,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenAiChatCustomToolMode {
    Native,
    FunctionEnvelope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenAiChatToolNamePolicy {
    max_chars: u16,
}

impl OpenAiChatToolNamePolicy {
    #[must_use]
    pub const fn new(max_chars: u16) -> Self {
        Self { max_chars }
    }

    #[must_use]
    pub const fn max_chars(self) -> usize {
        self.max_chars as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OpenAiChatRequestField {
    FrequencyPenalty,
    LogitBias,
    Logprobs,
    Metadata,
    ParallelToolCalls,
    PresencePenalty,
    PromptCacheKey,
    ResponseFormat,
    Seed,
    ServiceTier,
    Stop,
    Store,
    Temperature,
    TopLogprobs,
    TopP,
    User,
    Verbosity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenAiChatRequestFields(u32);

impl OpenAiChatRequestFields {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self((1_u32 << 17) - 1);

    #[must_use]
    pub const fn with(self, field: OpenAiChatRequestField) -> Self {
        Self(self.0 | (1_u32 << field as u8))
    }

    #[must_use]
    pub const fn without(self, field: OpenAiChatRequestField) -> Self {
        Self(self.0 & !(1_u32 << field as u8))
    }

    #[must_use]
    pub const fn contains(self, field: OpenAiChatRequestField) -> bool {
        self.0 & (1_u32 << field as u8) != 0
    }
}
