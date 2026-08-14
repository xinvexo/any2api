use any2api_runtime::api::CredentialRuntimeObservation;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct CredentialRuntimeResponse {
    resolved_proxy: ResolvedProxyResponse,
    rpm_60s: RpmWindowResponse,
    in_flight: u32,
    status: &'static str,
}

impl From<CredentialRuntimeObservation<'_>> for CredentialRuntimeResponse {
    fn from(value: CredentialRuntimeObservation<'_>) -> Self {
        let proxy = value.resolved_proxy();
        Self {
            resolved_proxy: ResolvedProxyResponse {
                id: proxy.id(),
                name: proxy.name().to_owned(),
                kind: proxy.kind().as_str(),
                enabled: proxy.enabled(),
            },
            rpm_60s: RpmWindowResponse {
                used: value.rpm_window_used(),
                limit: value.rpm_limit(),
            },
            in_flight: value.in_flight(),
            status: value.status().as_str(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ResolvedProxyResponse {
    id: any2api_domain::ProxyProfileId,
    name: String,
    kind: &'static str,
    enabled: bool,
}

#[derive(Debug, Serialize)]
struct RpmWindowResponse {
    used: u32,
    limit: Option<u32>,
}
