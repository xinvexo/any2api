use any2api_domain::{
    ConfigRevision, OAuthAccount, OAuthAccountDraft, OAuthAccountId, ProviderKind,
    RequestsPerMinute, RoutingCredentialId,
};
use any2api_runtime::api::{PublishedSnapshot, UpstreamCredentialUsageSummary};
use serde::{Deserialize, Serialize};

use super::{
    error::AdminApiError, revision::parse_revision, upstream_usage::UpstreamCredentialUsageResponse,
};

#[derive(Debug, Serialize)]
pub(super) struct OAuthAccountCollectionResponse {
    config_revision: u64,
    items: Vec<OAuthAccountResponse>,
}

impl OAuthAccountCollectionResponse {
    pub(super) fn from_snapshot(
        snapshot: &PublishedSnapshot,
        usage: &[UpstreamCredentialUsageSummary],
    ) -> Self {
        Self {
            config_revision: snapshot.revision().get(),
            items: snapshot
                .oauth_accounts()
                .accounts()
                .iter()
                .map(|account| OAuthAccountResponse::from_snapshot(account, snapshot, usage))
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthAccountResponse {
    id: OAuthAccountId,
    provider_kind: ProviderKind,
    label: String,
    requests_per_minute: Option<u32>,
    enabled: bool,
    safe_account_email: Option<String>,
    expires_at: Option<i64>,
    token_version: u64,
    account_generation: u64,
    config_version: u64,
    selected_model_count: usize,
    /// Models currently selected for public routing.
    models: Vec<String>,
    /// Plan/provider catalog this OAuth account may use (superset of models).
    available_models: Vec<String>,
    /// Official Codex `chatgpt_plan_type` from the ID Token (pass-through).
    plan_type: Option<String>,
    usage: UpstreamCredentialUsageResponse,
}

impl OAuthAccountResponse {
    fn from_snapshot(
        account: &OAuthAccount,
        snapshot: &PublishedSnapshot,
        usage: &[UpstreamCredentialUsageSummary],
    ) -> Self {
        let selected = account
            .models()
            .iter()
            .map(|model| model.as_str().to_owned())
            .collect::<Vec<_>>();
        let available_models = snapshot
            .oauth_available_models(account.id())
            .map(|models| {
                models
                    .iter()
                    .map(|model| model.as_str().to_owned())
                    .collect()
            })
            .unwrap_or_else(|| selected.clone());
        Self {
            id: account.id(),
            provider_kind: account.provider_kind(),
            label: account.label().to_owned(),
            requests_per_minute: account.requests_per_minute().map(RequestsPerMinute::get),
            enabled: account.enabled(),
            safe_account_email: account.safe_account_email().map(str::to_owned),
            expires_at: account.expires_at(),
            token_version: account.token_version(),
            account_generation: account.account_generation(),
            config_version: account.config_version(),
            selected_model_count: selected.len(),
            models: selected,
            available_models,
            plan_type: snapshot.oauth_plan_label(account.id()),
            usage: UpstreamCredentialUsageResponse::for_id(
                RoutingCredentialId::oauth_account(account.id()),
                usage,
            ),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OAuthAccountUpdateRequest {
    expected_revision: u64,
    expected_config_version: u64,
    label: String,
    requests_per_minute: Option<u32>,
    enabled: bool,
}

impl OAuthAccountUpdateRequest {
    pub(super) fn into_domain(
        self,
    ) -> Result<(ConfigRevision, u64, OAuthAccountDraft), AdminApiError> {
        let requests_per_minute = self
            .requests_per_minute
            .map(RequestsPerMinute::new)
            .transpose()
            .map_err(|error| AdminApiError::invalid_oauth_account(error.to_string()))?;
        let draft = OAuthAccountDraft::new(self.label, requests_per_minute, self.enabled)
            .map_err(|error| AdminApiError::invalid_oauth_account(error.to_string()))?;
        Ok((
            parse_revision(self.expected_revision)?,
            parse_version(self.expected_config_version)?,
            draft,
        ))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OAuthAccountModelsRequest {
    expected_revision: u64,
    expected_config_version: u64,
    models: Vec<String>,
}

impl OAuthAccountModelsRequest {
    pub(super) fn into_domain(self) -> Result<(ConfigRevision, u64, Vec<String>), AdminApiError> {
        Ok((
            parse_revision(self.expected_revision)?,
            parse_version(self.expected_config_version)?,
            self.models,
        ))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OAuthAccountDeleteQuery {
    expected_revision: u64,
    expected_config_version: u64,
}

impl OAuthAccountDeleteQuery {
    pub(super) fn into_domain(self) -> Result<(ConfigRevision, u64), AdminApiError> {
        Ok((
            parse_revision(self.expected_revision)?,
            parse_version(self.expected_config_version)?,
        ))
    }
}

fn parse_version(value: u64) -> Result<u64, AdminApiError> {
    (value > 0)
        .then_some(value)
        .ok_or_else(|| AdminApiError::invalid_request("expected_config_version is invalid"))
}
