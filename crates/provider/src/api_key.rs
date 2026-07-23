use std::collections::BTreeSet;

use any2api_domain::{
    API_KEY_SECRET_SCHEMA_VERSION, ProtocolOperation, ProviderBaseUrl, UpstreamModelName,
};
use serde::Deserialize;
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
        ProtocolOperation::ChatCompletions => "chat/completions",
        ProtocolOperation::Messages => "messages",
        ProtocolOperation::MessagesCountTokens => "messages/count_tokens",
    };
    let value = base_url
        .append_path(suffix)
        .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?;
    Url::parse(&value).map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))
}

pub(crate) fn credential_test_url(base_url: &ProviderBaseUrl) -> Result<Url, ProviderError> {
    let value = base_url
        .append_path("models")
        .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?;
    Url::parse(&value).map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))
}

pub(crate) fn parse_model_catalog(body: &[u8]) -> Result<Vec<String>, ProviderError> {
    let catalog = serde_json::from_slice::<ModelCatalog>(body)
        .map_err(|_| ProviderError::InvalidResponse("model catalog is not valid JSON".into()))?;
    let mut models = BTreeSet::new();
    for item in catalog.data {
        let model = UpstreamModelName::new(item.id)
            .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?;
        models.insert(model.as_str().to_owned());
    }
    Ok(models.into_iter().collect())
}

#[derive(Deserialize)]
struct ModelCatalog {
    data: Vec<ModelCatalogItem>,
}

#[derive(Deserialize)]
struct ModelCatalogItem {
    id: String,
}

#[cfg(test)]
mod tests {
    use super::parse_model_catalog;
    use crate::ProviderError;

    #[test]
    fn model_catalog_is_sorted_and_deduplicated() {
        let models =
            parse_model_catalog(br#"{"data":[{"id":"gpt-z"},{"id":"gpt-a"},{"id":"gpt-z"}]}"#)
                .expect("model catalog");

        assert_eq!(models, ["gpt-a", "gpt-z"]);
    }

    #[test]
    fn malformed_or_invalid_model_catalog_is_rejected() {
        assert!(matches!(
            parse_model_catalog(br#"{"data":not-json}"#),
            Err(ProviderError::InvalidResponse(_))
        ));
        assert!(matches!(
            parse_model_catalog(br#"{"data":[{"id":" invalid "}]}"#),
            Err(ProviderError::InvalidResponse(_))
        ));
    }
}
