use any2api_domain::{
    CredentialId, OAuthAccountId, ProviderCredentialConfiguration, ProviderEndpointConfiguration,
    ProviderEndpointId, RequestAttempt, RequestAttemptFailureScope, RequestAttemptRetryDecision,
    RequestAttemptStreamTiming, RequestAttemptTransport, RequestRoutingMode,
};
use any2api_runtime::api::PublishedSnapshot;
use serde::Serialize;

use super::dto::RequestLogOutcome;

#[derive(Serialize)]
pub(super) struct RequestAttemptResponse {
    attempt_no: u32,
    route_target_id: Option<String>,
    provider_endpoint_id: Option<String>,
    provider_endpoint_name: Option<String>,
    credential_id: Option<String>,
    credential_label: Option<String>,
    oauth_account_id: Option<String>,
    oauth_account_label: Option<String>,
    proxy_profile_id: Option<String>,
    proxy_profile_label: Option<String>,
    routing_mode: Option<&'static str>,
    failure_scope: Option<&'static str>,
    retry_decision: Option<&'static str>,
    started_at_ms: u64,
    duration_ms: u64,
    error_message: Option<String>,
    status_code: Option<u16>,
    outcome: RequestLogOutcome,
    transport: Option<RequestAttemptTransportResponse>,
    stream_timing: Option<RequestAttemptStreamTimingResponse>,
}

impl RequestAttemptResponse {
    pub(super) fn from_attempt(value: RequestAttempt, snapshot: &PublishedSnapshot) -> Self {
        let outcome = RequestLogOutcome::from_attempt(&value);
        let provider_endpoint = resolve_provider_endpoint(
            value.credential_id,
            value.oauth_account_id,
            snapshot.provider_credentials(),
            snapshot.provider_endpoints(),
        );
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
            attempt_no: value.attempt_no,
            route_target_id: value.route_target_id.map(|id| id.to_string()),
            provider_endpoint_id: provider_endpoint.as_ref().map(|(id, _)| id.to_string()),
            provider_endpoint_name: provider_endpoint.map(|(_, name)| name),
            credential_id: value.credential_id.map(|id| id.to_string()),
            credential_label,
            oauth_account_id: value.oauth_account_id.map(|id| id.to_string()),
            oauth_account_label,
            proxy_profile_id: value.proxy_profile_id.map(|id| id.to_string()),
            proxy_profile_label,
            routing_mode: value.routing_mode.map(RequestRoutingMode::as_str),
            failure_scope: value.failure_scope.map(RequestAttemptFailureScope::as_str),
            retry_decision: value
                .retry_decision
                .map(RequestAttemptRetryDecision::as_str),
            started_at_ms: value.started_at_ms,
            duration_ms: value.duration_ms,
            error_message: value.error_message,
            status_code: value.status_code,
            outcome,
            transport: value.transport.map(Into::into),
            stream_timing: value.stream_timing.map(Into::into),
        }
    }
}

fn resolve_provider_endpoint(
    credential_id: Option<CredentialId>,
    oauth_account_id: Option<OAuthAccountId>,
    credentials: &ProviderCredentialConfiguration,
    endpoints: &ProviderEndpointConfiguration,
) -> Option<(ProviderEndpointId, String)> {
    if oauth_account_id.is_some() {
        return None;
    }
    let credential = credentials.get(credential_id?)?;
    let endpoint_id = credential.provider_endpoint_id();
    let endpoint = endpoints.get(endpoint_id)?;
    Some((endpoint_id, endpoint.name().to_owned()))
}

#[derive(Serialize)]
struct RequestAttemptTransportResponse {
    wire_profile_id: String,
    wire_profile_version: u16,
    timeout_policy_version: u16,
    resolver_mode: &'static str,
    proxy_kind: &'static str,
    connect_timeout_ms: u64,
    read_timeout_ms: u64,
    pool_idle_timeout_ms: u64,
    routing_generation: u64,
    authentication_version: u64,
    traffic_class: &'static str,
}

impl From<RequestAttemptTransport> for RequestAttemptTransportResponse {
    fn from(value: RequestAttemptTransport) -> Self {
        Self {
            wire_profile_id: value.wire_profile_id,
            wire_profile_version: value.wire_profile_version,
            timeout_policy_version: value.timeout_policy_version,
            resolver_mode: value.resolver_mode.as_str(),
            proxy_kind: value.proxy_kind.as_str(),
            connect_timeout_ms: value.connect_timeout_ms,
            read_timeout_ms: value.read_timeout_ms,
            pool_idle_timeout_ms: value.pool_idle_timeout_ms,
            routing_generation: value.routing_generation,
            authentication_version: value.authentication_version,
            traffic_class: value.traffic_class.as_str(),
        }
    }
}

#[derive(Serialize)]
struct RequestAttemptStreamTimingResponse {
    first_upstream_frame_ms: Option<u64>,
    stream_commit_ms: Option<u64>,
    first_downstream_byte_ms: Option<u64>,
    stream_cancel_ms: Option<u64>,
}

impl From<RequestAttemptStreamTiming> for RequestAttemptStreamTimingResponse {
    fn from(value: RequestAttemptStreamTiming) -> Self {
        Self {
            first_upstream_frame_ms: value.first_upstream_frame_ms,
            stream_commit_ms: value.stream_commit_ms,
            first_downstream_byte_ms: value.first_downstream_byte_ms,
            stream_cancel_ms: value.stream_cancel_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{
        CredentialId, CredentialKind, CredentialSecretFingerprint, OAuthAccountId, ProtocolDialect,
        ProviderCredential, ProviderCredentialConfiguration, ProviderCredentialDraft,
        ProviderEndpoint, ProviderEndpointConfiguration, ProviderEndpointDraft, ProviderEndpointId,
        ProviderKind, ProxyConfiguration, ProxyKind, ProxyProfileId, RequestAttemptStreamTiming,
        RequestAttemptTransport, RequestTransportResolverMode, RequestTransportTrafficClass,
    };

    use super::{
        RequestAttemptResponse, RequestAttemptStreamTimingResponse,
        RequestAttemptTransportResponse, resolve_provider_endpoint,
    };
    use crate::admin::request_log::dto::RequestLogOutcome;

    #[test]
    fn api_key_attempt_resolves_endpoint_while_oauth_attempt_does_not() {
        let endpoint_id = ProviderEndpointId::new();
        let credential_id = CredentialId::new();
        let endpoints = ProviderEndpointConfiguration::new(vec![
            ProviderEndpoint::create(
                endpoint_id,
                ProviderEndpointDraft::new(
                    "Codex upstream",
                    ProviderKind::Codex,
                    "https://api.example.com",
                    ProtocolDialect::OpenAiResponses,
                    None,
                    true,
                )
                .expect("endpoint draft"),
            )
            .expect("endpoint"),
        ])
        .expect("endpoint configuration");
        let credentials = ProviderCredentialConfiguration::new(
            vec![ProviderCredential::create(
                credential_id,
                endpoint_id,
                ProviderCredentialDraft::new(
                    "Primary Codex",
                    CredentialKind::ApiKey,
                    ProxyProfileId::DIRECT,
                    None,
                    true,
                )
                .expect("credential draft"),
                CredentialSecretFingerprint::new([0x5a; 32], Some("test".to_owned()))
                    .expect("fingerprint"),
            )],
            &endpoints,
            &ProxyConfiguration::initial(),
        )
        .expect("credential configuration");

        let endpoint =
            resolve_provider_endpoint(Some(credential_id), None, &credentials, &endpoints)
                .expect("API key endpoint");
        assert_eq!(endpoint.0, endpoint_id);
        assert_eq!(endpoint.1, "Codex upstream");

        assert_eq!(
            resolve_provider_endpoint(
                Some(credential_id),
                Some(OAuthAccountId::new()),
                &credentials,
                &endpoints,
            ),
            None
        );

        let response = RequestAttemptResponse {
            attempt_no: 1,
            route_target_id: None,
            provider_endpoint_id: Some(endpoint_id.to_string()),
            provider_endpoint_name: Some(endpoint.1),
            credential_id: Some(credential_id.to_string()),
            credential_label: Some("Primary Codex".to_owned()),
            oauth_account_id: None,
            oauth_account_label: None,
            proxy_profile_id: Some(ProxyProfileId::DIRECT.to_string()),
            proxy_profile_label: Some("DIRECT".to_owned()),
            routing_mode: None,
            failure_scope: None,
            retry_decision: None,
            started_at_ms: 1,
            duration_ms: 2,
            error_message: None,
            status_code: Some(200),
            outcome: RequestLogOutcome::Success,
            transport: None,
            stream_timing: None,
        };
        let json = serde_json::to_value(response).expect("attempt response JSON");
        assert_eq!(json["provider_endpoint_id"], endpoint_id.to_string());
        assert_eq!(json["provider_endpoint_name"], "Codex upstream");
    }

    #[test]
    fn transport_and_stream_diagnostics_use_the_current_nested_wire_contract() {
        let transport = serde_json::to_value(RequestAttemptTransportResponse::from(
            RequestAttemptTransport {
                wire_profile_id: "generic-rustls-hyper-v2".into(),
                wire_profile_version: 2,
                timeout_policy_version: 1,
                resolver_mode: RequestTransportResolverMode::LocalCached,
                proxy_kind: ProxyKind::Socks5,
                connect_timeout_ms: 10_000,
                read_timeout_ms: 300_000,
                pool_idle_timeout_ms: 50_000,
                routing_generation: 4,
                authentication_version: 7,
                traffic_class: RequestTransportTrafficClass::DataPlane,
            },
        ))
        .expect("transport response JSON");
        assert_eq!(transport["wire_profile_id"], "generic-rustls-hyper-v2");
        assert_eq!(transport["resolver_mode"], "local_cached");
        assert_eq!(transport["proxy_kind"], "socks5");
        assert_eq!(transport["routing_generation"], 4);

        let timing = serde_json::to_value(RequestAttemptStreamTimingResponse::from(
            RequestAttemptStreamTiming {
                first_upstream_frame_ms: Some(8),
                stream_commit_ms: Some(9),
                first_downstream_byte_ms: Some(11),
                stream_cancel_ms: None,
            },
        ))
        .expect("stream timing JSON");
        assert_eq!(timing["first_upstream_frame_ms"], 8);
        assert_eq!(timing["stream_commit_ms"], 9);
        assert_eq!(timing["first_downstream_byte_ms"], 11);
        assert!(timing["stream_cancel_ms"].is_null());
    }
}
