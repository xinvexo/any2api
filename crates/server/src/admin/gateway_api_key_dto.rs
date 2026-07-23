use any2api_domain::{ConfigRevision, GatewayApiKey, GatewayApiKeyDraft, GatewayApiKeyId};
use any2api_runtime::api::{
    GatewayApiKeyPublishResult, GatewayApiKeyUsageSummary, PublishedSnapshot, RequestTelemetry,
};
use serde::{Deserialize, Serialize};

use super::{error::AdminApiError, revision::parse_revision};

#[derive(Serialize)]
pub(crate) struct GatewayApiKeyCollectionResponse {
    config_revision: u64,
    items: Vec<GatewayApiKeyResponse>,
}

impl GatewayApiKeyCollectionResponse {
    pub(crate) fn from_snapshot(
        snapshot: &PublishedSnapshot,
        telemetry: &RequestTelemetry,
        usage: &[GatewayApiKeyUsageSummary],
    ) -> Self {
        Self {
            config_revision: snapshot.revision().get(),
            items: snapshot
                .gateway_api_keys()
                .keys()
                .iter()
                .map(|key| {
                    GatewayApiKeyResponse::new(
                        key,
                        telemetry,
                        usage.iter().find(|summary| summary.id == key.id()),
                    )
                })
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
    pub(crate) fn from_publish(
        result: &GatewayApiKeyPublishResult,
        telemetry: &RequestTelemetry,
        usage: &[GatewayApiKeyUsageSummary],
    ) -> Self {
        let configuration =
            GatewayApiKeyCollectionResponse::from_snapshot(result.snapshot(), telemetry, usage);
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
    token: String,
    token_prefix: String,
    token_version: u64,
    config_version: u64,
    enabled: bool,
    revoked_at: Option<String>,
    created_at: String,
    last_used_at: Option<String>,
    usage: GatewayApiKeyUsageResponse,
}

impl GatewayApiKeyResponse {
    fn new(
        key: &GatewayApiKey,
        telemetry: &RequestTelemetry,
        usage: Option<&GatewayApiKeyUsageSummary>,
    ) -> Self {
        let live_last_used_at = telemetry.gateway_key_last_used_at(key.id());
        let last_used_at = newest_timestamp(key.last_used_at(), live_last_used_at.as_deref());
        Self {
            id: key.id(),
            name: key.name().to_owned(),
            token: key.token().to_owned(),
            token_prefix: key.token_prefix().to_owned(),
            token_version: key.token_version(),
            config_version: key.config_version(),
            enabled: key.enabled(),
            revoked_at: key.revoked_at().map(str::to_owned),
            created_at: key.created_at().to_owned(),
            last_used_at,
            usage: GatewayApiKeyUsageResponse::new(usage),
        }
    }
}

#[derive(Serialize)]
struct GatewayApiKeyUsageResponse {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    recent_outcomes: Vec<GatewayApiKeyRequestOutcomeResponse>,
}

impl GatewayApiKeyUsageResponse {
    fn new(summary: Option<&GatewayApiKeyUsageSummary>) -> Self {
        let Some(summary) = summary else {
            return Self {
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                recent_outcomes: Vec::new(),
            };
        };
        Self {
            total_requests: summary.total_requests,
            successful_requests: summary.successful_requests,
            failed_requests: summary.failed_requests(),
            recent_outcomes: summary
                .recent_outcomes
                .iter()
                .map(|outcome| GatewayApiKeyRequestOutcomeResponse {
                    status_code: outcome.status_code,
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct GatewayApiKeyRequestOutcomeResponse {
    status_code: u16,
}

fn newest_timestamp(stored: Option<&str>, live: Option<&str>) -> Option<String> {
    stored.into_iter().chain(live).max().map(str::to_owned)
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
