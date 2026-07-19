use any2api_domain::{API_KEY_SECRET_SCHEMA_VERSION, ProtocolOperation, ProviderBaseUrl};
use url::Url;

use crate::{ProviderError, ProviderSecret};

pub(crate) fn validate_secret(secret: &ProviderSecret) -> Result<(), ProviderError> {
    if secret.schema_version() != API_KEY_SECRET_SCHEMA_VERSION {
        return Err(ProviderError::InvalidCredential(
            "unsupported API Key schema".into(),
        ));
    }
    let value = secret.expose().as_bytes();
    if value.is_empty()
        || value.len() > 8_192
        || !value.iter().all(|byte| (0x21..=0x7e).contains(byte))
    {
        return Err(ProviderError::InvalidCredential(
            "API Key must contain visible ASCII characters".into(),
        ));
    }
    Ok(())
}

pub(crate) fn endpoint_url(
    base_url: &ProviderBaseUrl,
    operation: ProtocolOperation,
) -> Result<Url, ProviderError> {
    let suffix = match operation {
        ProtocolOperation::Responses => "responses",
        ProtocolOperation::ResponsesCompact => "responses/compact",
        ProtocolOperation::Messages => "messages",
        ProtocolOperation::MessagesCountTokens => "messages/count_tokens",
    };
    let value = base_url
        .append_path(suffix)
        .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?;
    Url::parse(&value).map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))
}
