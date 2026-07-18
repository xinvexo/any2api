use any2api_domain::{ConfigRevision, GatewayApiKey, GatewayApiKeyDraft, GatewayApiKeyId};
use any2api_runtime::api::{GatewayApiKeyPublishResult, PublishedSnapshot};
use serde::{Deserialize, Serialize};

use super::{error::AdminApiError, revision::parse_revision};

#[derive(Serialize)]
pub(crate) struct GatewayApiKeyCollectionResponse {
    config_revision: u64,
    items: Vec<GatewayApiKeyResponse>,
}

impl GatewayApiKeyCollectionResponse {
    pub(crate) fn from_snapshot(snapshot: &PublishedSnapshot) -> Self {
        Self {
            config_revision: snapshot.revision().get(),
            items: snapshot
                .gateway_api_keys()
                .keys()
                .iter()
                .map(GatewayApiKeyResponse::from)
                .collect(),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct GatewayApiKeySecretResponse {
    config_revision: u64,
    items: Vec<GatewayApiKeyResponse>,
    token: String,
}

impl GatewayApiKeySecretResponse {
    pub(crate) fn from_publish(result: &GatewayApiKeyPublishResult) -> Self {
        let configuration = GatewayApiKeyCollectionResponse::from_snapshot(result.snapshot());
        Self {
            config_revision: configuration.config_revision,
            items: configuration.items,
            token: result.token().as_str().to_owned(),
        }
    }
}

#[derive(Serialize)]
struct GatewayApiKeyResponse {
    id: GatewayApiKeyId,
    name: String,
    token_prefix: String,
    token_version: u64,
    config_version: u64,
    enabled: bool,
    revoked_at: Option<String>,
    created_at: String,
    last_used_at: Option<String>,
}

impl From<&GatewayApiKey> for GatewayApiKeyResponse {
    fn from(key: &GatewayApiKey) -> Self {
        Self {
            id: key.id(),
            name: key.name().to_owned(),
            token_prefix: key.token_prefix().to_owned(),
            token_version: key.token_version(),
            config_version: key.config_version(),
            enabled: key.enabled(),
            revoked_at: key.revoked_at().map(str::to_owned),
            created_at: key.created_at().to_owned(),
            last_used_at: key.last_used_at().map(str::to_owned),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GatewayApiKeyCreateRequest {
    expected_revision: u64,
    name: String,
    enabled: bool,
}

impl GatewayApiKeyCreateRequest {
    pub(crate) fn into_domain(self) -> Result<(ConfigRevision, GatewayApiKeyDraft), AdminApiError> {
        Ok((
            parse_revision(self.expected_revision)?,
            build_draft(self.name, self.enabled)?,
        ))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GatewayApiKeyUpdateRequest {
    expected_revision: u64,
    expected_config_version: u64,
    name: String,
    enabled: bool,
}

impl GatewayApiKeyUpdateRequest {
    pub(crate) fn into_domain(
        self,
    ) -> Result<(ConfigRevision, u64, GatewayApiKeyDraft), AdminApiError> {
        Ok((
            parse_revision(self.expected_revision)?,
            parse_version(
                self.expected_config_version,
                "expected_config_version is invalid",
            )?,
            build_draft(self.name, self.enabled)?,
        ))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GatewayApiKeyRotateRequest {
    expected_revision: u64,
    expected_config_version: u64,
    expected_token_version: u64,
}

impl GatewayApiKeyRotateRequest {
    pub(crate) fn into_domain(self) -> Result<(ConfigRevision, u64, u64), AdminApiError> {
        Ok((
            parse_revision(self.expected_revision)?,
            parse_version(
                self.expected_config_version,
                "expected_config_version is invalid",
            )?,
            parse_version(
                self.expected_token_version,
                "expected_token_version is invalid",
            )?,
        ))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GatewayApiKeyRevokeRequest {
    expected_revision: u64,
    expected_config_version: u64,
}

impl GatewayApiKeyRevokeRequest {
    pub(crate) fn into_domain(self) -> Result<(ConfigRevision, u64), AdminApiError> {
        Ok((
            parse_revision(self.expected_revision)?,
            parse_version(
                self.expected_config_version,
                "expected_config_version is invalid",
            )?,
        ))
    }
}

fn build_draft(name: String, enabled: bool) -> Result<GatewayApiKeyDraft, AdminApiError> {
    GatewayApiKeyDraft::new(name, enabled)
        .map_err(|error| AdminApiError::invalid_gateway_api_key(error.to_string()))
}

fn parse_version(value: u64, message: &'static str) -> Result<u64, AdminApiError> {
    (value > 0)
        .then_some(value)
        .ok_or_else(|| AdminApiError::invalid_request(message))
}
