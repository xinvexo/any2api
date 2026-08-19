use any2api_domain::ActiveRequestLog;
use any2api_runtime::api::PublishedSnapshot;
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct ActiveRequestLogResponse {
    state: &'static str,
    request_id: String,
    started_at_ms: u64,
    client_ip: String,
    config_revision: u64,
    gateway_api_key_id: String,
    ingress_protocol: &'static str,
    operation: &'static str,
    public_model: Option<String>,
    thinking_level: Option<String>,
    provider_endpoint_id: Option<String>,
    provider_endpoint_name: Option<String>,
    credential_id: Option<String>,
    credential_label: Option<String>,
    oauth_account_id: Option<String>,
    oauth_account_label: Option<String>,
    proxy_profile_id: Option<String>,
    proxy_profile_label: Option<String>,
    attempt_count: u32,
    is_stream: Option<bool>,
    requested_speed_tier: Option<&'static str>,
    effective_speed_tier: Option<&'static str>,
}

impl ActiveRequestLogResponse {
    pub(super) fn from_log(value: ActiveRequestLog, snapshot: &PublishedSnapshot) -> Self {
        let provider_endpoint_name = value.provider_endpoint_id.and_then(|id| {
            snapshot
                .provider_endpoints()
                .get(id)
                .map(|endpoint| endpoint.name().to_owned())
        });
        let credential_label = value.credential_id.and_then(|id| {
            snapshot
                .provider_credentials()
                .get(id)
                .map(|credential| credential.label().to_owned())
        });
        let oauth_account_label = value.oauth_account_id.and_then(|id| {
            snapshot
                .oauth_accounts()
                .get(id)
                .map(|account| account.label().to_owned())
        });
        let proxy_profile_label = value.proxy_profile_id.and_then(|id| {
            snapshot
                .proxies()
                .get(id)
                .map(|profile| profile.name().to_owned())
        });
        Self {
            state: "processing",
            request_id: value.request_id.to_string(),
            started_at_ms: value.started_at_ms,
            client_ip: value.client_ip.to_string(),
            config_revision: value.config_revision.get(),
            gateway_api_key_id: value.gateway_api_key_id.to_string(),
            ingress_protocol: value.ingress_protocol.as_str(),
            operation: value.operation.as_str(),
            public_model: value.public_model,
            thinking_level: value.thinking_level,
            provider_endpoint_id: value.provider_endpoint_id.map(|id| id.to_string()),
            provider_endpoint_name,
            credential_id: value.credential_id.map(|id| id.to_string()),
            credential_label,
            oauth_account_id: value.oauth_account_id.map(|id| id.to_string()),
            oauth_account_label,
            proxy_profile_id: value.proxy_profile_id.map(|id| id.to_string()),
            proxy_profile_label,
            attempt_count: value.attempt_count,
            is_stream: value.is_stream,
            requested_speed_tier: value.requested_speed_tier.map(|tier| tier.as_str()),
            effective_speed_tier: value.effective_speed_tier.map(|tier| tier.as_str()),
        }
    }
}
