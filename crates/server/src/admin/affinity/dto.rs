use any2api_domain::{ProtocolDialect, RouteTargetId, RoutingCredentialId};
use any2api_runtime::api::{
    AffinityBindingSummary, AffinityCredentialCount, AffinityRuntimeSnapshot, PublishedSnapshot,
};
use serde::{Deserialize, Serialize};

use super::error::AdminApiError;

const DEFAULT_BINDING_LIMIT: usize = 100;
const MAX_BINDING_LIMIT: usize = 500;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AffinityQuery {
    #[serde(default = "default_binding_limit")]
    limit: usize,
}

impl AffinityQuery {
    pub(crate) fn limit(self) -> Result<usize, AdminApiError> {
        (self.limit <= MAX_BINDING_LIMIT)
            .then_some(self.limit)
            .ok_or_else(|| AdminApiError::invalid_request("limit must be between 0 and 500"))
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct AffinityRuntimeResponse {
    config_revision: u64,
    soft_binding_count: usize,
    hard_binding_count: usize,
    creating_count: usize,
    credential_counts: Vec<AffinityCredentialCountResponse>,
    bindings: Vec<AffinityBindingResponse>,
}

impl AffinityRuntimeResponse {
    pub(crate) fn new(published: &PublishedSnapshot, snapshot: &AffinityRuntimeSnapshot) -> Self {
        Self {
            config_revision: published.revision().get(),
            soft_binding_count: snapshot.soft_binding_count(),
            hard_binding_count: snapshot.hard_binding_count(),
            creating_count: snapshot.creating_count(),
            credential_counts: snapshot
                .credential_counts()
                .iter()
                .map(|count| AffinityCredentialCountResponse::new(count, published))
                .collect(),
            bindings: snapshot
                .bindings()
                .iter()
                .map(AffinityBindingResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct AffinityClearResponse {
    cleared_count: usize,
}

impl AffinityClearResponse {
    pub(crate) const fn new(cleared_count: usize) -> Self {
        Self { cleared_count }
    }
}

#[derive(Debug, Serialize)]
struct AffinityCredentialCountResponse {
    credential_id: String,
    credential_source: &'static str,
    credential_label: String,
    soft_bindings: usize,
    hard_bindings: usize,
}

impl AffinityCredentialCountResponse {
    fn new(value: &AffinityCredentialCount, published: &PublishedSnapshot) -> Self {
        let credential_id = value.credential_id();
        Self {
            credential_id: routing_credential_token(credential_id),
            credential_source: routing_credential_source(credential_id),
            credential_label: routing_credential_label(published, credential_id),
            soft_bindings: value.soft_bindings(),
            hard_bindings: value.hard_bindings(),
        }
    }
}

#[derive(Debug, Serialize)]
struct AffinityBindingResponse {
    kind: &'static str,
    session_hash_prefix: String,
    credential_id: String,
    credential_source: &'static str,
    route_target_id: RouteTargetId,
    upstream_model: String,
    protocol_dialect: ProtocolDialect,
    expires_in_ms: u64,
}

impl From<&AffinityBindingSummary> for AffinityBindingResponse {
    fn from(value: &AffinityBindingSummary) -> Self {
        Self {
            kind: value.kind().as_str(),
            session_hash_prefix: value.session_hash_prefix().to_owned(),
            credential_id: routing_credential_token(value.credential_id()),
            credential_source: routing_credential_source(value.credential_id()),
            route_target_id: value.route_target_id(),
            upstream_model: value.upstream_model().to_owned(),
            protocol_dialect: value.protocol_dialect(),
            expires_in_ms: value.expires_in_ms(),
        }
    }
}

fn routing_credential_token(id: RoutingCredentialId) -> String {
    match id {
        RoutingCredentialId::ProviderCredential(id) => id.to_string(),
        RoutingCredentialId::OAuthAccount(id) => format!("oauth_account:{id}"),
    }
}

const fn routing_credential_source(id: RoutingCredentialId) -> &'static str {
    match id {
        RoutingCredentialId::ProviderCredential(_) => "provider_credential",
        RoutingCredentialId::OAuthAccount(_) => "oauth_account",
    }
}

fn routing_credential_label(published: &PublishedSnapshot, id: RoutingCredentialId) -> String {
    match id {
        RoutingCredentialId::ProviderCredential(id) => published
            .provider_credentials()
            .get(id)
            .map(|credential| credential.label().to_owned())
            .unwrap_or_else(|| id.to_string()),
        RoutingCredentialId::OAuthAccount(id) => published
            .oauth_accounts()
            .get(id)
            .map(|account| account.label().to_owned())
            .unwrap_or_else(|| id.to_string()),
    }
}

const fn default_binding_limit() -> usize {
    DEFAULT_BINDING_LIMIT
}
