use any2api_domain::{
    ConfigRevision, GatewayApiKey, GatewayApiKeyDraft, GatewayApiKeyId, RequestsPerMinute,
};
use any2api_runtime::api::{GatewayApiKeyUsageSummary, PublishedSnapshot, RequestTelemetry};
use serde::{Deserialize, Serialize};

use crate::admin::request_usage::RequestUsageResponse;

use super::{error::AdminApiError, revision::parse_revision};

#[derive(Serialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "GatewayApiKeyCollectionResponse.ts")
)]
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
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "GatewayApiKeySecretResponse.ts")
)]
pub(crate) struct GatewayApiKeySecretResponse {
    config_revision: u64,
    items: Vec<GatewayApiKeyResponse>,
    token: String,
}

impl GatewayApiKeySecretResponse {
    pub(crate) fn new(configuration: GatewayApiKeyCollectionResponse, token: &str) -> Self {
        Self {
            config_revision: configuration.config_revision,
            items: configuration.items,
            token: token.to_owned(),
        }
    }
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export_to = "GatewayApiKeyResponse.ts"))]
struct GatewayApiKeyResponse {
    #[cfg_attr(test, ts(as = "String"))]
    id: GatewayApiKeyId,
    name: String,
    token_prefix: String,
    token_version: u64,
    config_version: u64,
    requests_per_minute: Option<u32>,
    enabled: bool,
    created_at: String,
    last_used_at: Option<String>,
    usage: RequestUsageResponse,
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
            token_prefix: key.token_prefix().to_owned(),
            token_version: key.token_version(),
            config_version: key.config_version(),
            requests_per_minute: key.requests_per_minute().map(RequestsPerMinute::get),
            enabled: key.enabled(),
            created_at: key.created_at().to_owned(),
            last_used_at,
            usage: usage.map_or_else(RequestUsageResponse::empty, |summary| {
                RequestUsageResponse::new(
                    summary.total_requests,
                    summary.successful_requests,
                    &summary.window_slots,
                )
            }),
        }
    }
}

fn newest_timestamp(stored: Option<&str>, live: Option<&str>) -> Option<String> {
    stored.into_iter().chain(live).max().map(str::to_owned)
}

#[derive(Deserialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "GatewayApiKeyCreateRequest.ts")
)]
#[serde(deny_unknown_fields)]
pub(crate) struct GatewayApiKeyCreateRequest {
    expected_revision: u64,
    name: String,
    requests_per_minute: Option<u32>,
    enabled: bool,
}

impl GatewayApiKeyCreateRequest {
    pub(crate) fn into_domain(self) -> Result<(ConfigRevision, GatewayApiKeyDraft), AdminApiError> {
        Ok((
            parse_revision(self.expected_revision)?,
            build_draft(self.name, self.requests_per_minute, self.enabled)?,
        ))
    }
}

#[derive(Deserialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "GatewayApiKeyUpdateRequest.ts")
)]
#[serde(deny_unknown_fields)]
pub(crate) struct GatewayApiKeyUpdateRequest {
    expected_revision: u64,
    expected_config_version: u64,
    name: String,
    requests_per_minute: Option<u32>,
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
            build_draft(self.name, self.requests_per_minute, self.enabled)?,
        ))
    }
}

#[derive(Deserialize)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "GatewayApiKeyRotateRequest.ts")
)]
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
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "GatewayApiKeyDeleteRequest.ts")
)]
#[serde(deny_unknown_fields)]
pub(crate) struct GatewayApiKeyDeleteRequest {
    expected_revision: u64,
    expected_config_version: u64,
}

impl GatewayApiKeyDeleteRequest {
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

fn build_draft(
    name: String,
    requests_per_minute: Option<u32>,
    enabled: bool,
) -> Result<GatewayApiKeyDraft, AdminApiError> {
    let requests_per_minute = requests_per_minute
        .map(RequestsPerMinute::new)
        .transpose()
        .map_err(|error| AdminApiError::invalid_gateway_api_key(error.to_string()))?;
    GatewayApiKeyDraft::new(name, enabled)
        .map(|draft| draft.with_requests_per_minute(requests_per_minute))
        .map_err(|error| AdminApiError::invalid_gateway_api_key(error.to_string()))
}

fn parse_version(value: u64, message: &'static str) -> Result<u64, AdminApiError> {
    (value > 0)
        .then_some(value)
        .ok_or_else(|| AdminApiError::invalid_request(message))
}

#[cfg(test)]
pub(in crate::admin) fn export_bindings(config: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    use ts_rs::TS as _;

    GatewayApiKeyCollectionResponse::export_all(config)?;
    GatewayApiKeySecretResponse::export_all(config)?;
    GatewayApiKeyCreateRequest::export_all(config)?;
    GatewayApiKeyUpdateRequest::export_all(config)?;
    GatewayApiKeyRotateRequest::export_all(config)?;
    GatewayApiKeyDeleteRequest::export_all(config)
}

#[cfg(test)]
mod tests {
    use super::build_draft;

    #[test]
    fn optional_gateway_rate_limit_is_bounded() {
        let unlimited = build_draft("Unlimited".to_owned(), None, true).expect("unlimited draft");
        assert_eq!(unlimited.requests_per_minute(), None);

        let limited =
            build_draft("Limited".to_owned(), Some(100_000), true).expect("limited draft");
        assert_eq!(
            limited.requests_per_minute().map(|value| value.get()),
            Some(100_000)
        );
        assert!(build_draft("Invalid".to_owned(), Some(0), true).is_err());
        assert!(build_draft("Invalid".to_owned(), Some(100_001), true).is_err());
    }
}
