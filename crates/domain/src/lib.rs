mod credential;
mod error;
mod gateway_api_key;
mod id;
mod kind;
mod oauth_account;
mod provider;
mod proxy;
mod revision;
mod routing;
mod settings;
mod telemetry;
mod upstream_error;

pub use credential::{
    CREDENTIAL_FINGERPRINT_LENGTH, CREDENTIAL_FINGERPRINT_VERSION, CredentialFingerprintError,
    CredentialSecretFingerprint, MAX_REQUESTS_PER_MINUTE, RequestsPerMinute,
    RequestsPerMinuteError,
};
pub use error::{ANY2API_UPSTREAM_TIMEOUT_MESSAGE, ErrorClass, PublicError, PublicErrorCode};
pub use gateway_api_key::{
    GATEWAY_TOKEN_BODY_LEN, GATEWAY_TOKEN_HASH_VERSION, GATEWAY_TOKEN_PREFIX,
    GATEWAY_TOKEN_VERSION, GatewayApiKey, GatewayApiKeyConfiguration, GatewayApiKeyDraft,
    GatewayApiKeyValidationError, validate_gateway_token,
};
pub use id::{
    CredentialId, GatewayApiKeyId, ModelRouteId, OAuthAccountId, ProviderEndpointId,
    ProxyProfileId, RequestId, RouteTargetId,
};
pub use kind::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderKind, RequestBodyEncoding,
    TransportMode,
};
pub use oauth_account::{
    OAuthAccount, OAuthAccountConfiguration, OAuthAccountDraft, OAuthAccountValidationError,
};
pub use provider::{
    API_KEY_SECRET_SCHEMA_VERSION, ProviderBaseUrl, ProviderCredential,
    ProviderCredentialConfiguration, ProviderCredentialDraft, ProviderCredentialValidationError,
    ProviderEndpoint, ProviderEndpointConfiguration, ProviderEndpointDraft,
    ProviderEndpointValidationError, ProviderUrlValidationError,
};
pub use proxy::{
    MAX_PROXY_USERNAME_BYTES, ProxyAddress, ProxyAuthentication,
    ProxyAuthenticationValidationError, ProxyConfiguration, ProxyDraft, ProxyKind, ProxyProfile,
    ProxyValidationError,
};
pub use revision::{ConfigRevision, ConfigRevisionError};
pub use routing::{
    FallbackTier, MAX_MODEL_NAME_CHARS, ModelNameValidationError, ModelRoute,
    ModelRouteConfiguration, ModelRouteDraft, ModelRouteValidationError, PublicModelName,
    RetrySafety, RouteTarget, RouteTargetDraft, RoutingCredentialId, UpstreamModelName,
};
pub use settings::{
    AdminSettings, AffinitySettings, FileLogLevel, LoggingSettings, MAX_FILE_LOG_RETENTION_SECS,
    MAX_FILE_LOG_TOTAL_SIZE, MAX_REQUEST_LOG_RETENTION_SECS, MAX_REQUEST_LOG_ROWS,
    MAX_STREAM_PRECOMMIT_BYTES, MAX_TELEMETRY_QUEUE_CAPACITY, ModelSettings, OAuthSettings,
    RateLimitMode, ReliabilitySettings, SchedulerSettings, SettingApplyMode, SettingDefinition,
    SettingKey, SettingOverrides, SettingValue, SettingValueType, SettingsConfiguration,
    SettingsValidationError, ShutdownSettings, StreamSettings, UpstreamSettings,
};
pub use telemetry::{
    CompletedRequestLog, HttpAccessLog, HttpAccessLogOutcome, HttpProtocolVersion,
    MAX_HTTP_ACCESS_LOG_METHOD_CHARS, MAX_HTTP_ACCESS_LOG_PATH_CHARS,
    MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS, MAX_REQUEST_LOG_THINKING_LEVEL_CHARS, MAX_TOKEN_COUNT,
    RequestAttempt, RequestAttemptOutcome, RequestLog, TokenUsage, bound_error_message,
    bound_thinking_level, bounded_log_text,
};
pub use upstream_error::{
    MAX_RETRY_AFTER_SECONDS, MAX_UPSTREAM_ERROR_MESSAGE_BYTES, RetryAfterHint, UpstreamError,
    UpstreamErrorClassification, UpstreamErrorKind, UpstreamQuotaExhaustion,
};
