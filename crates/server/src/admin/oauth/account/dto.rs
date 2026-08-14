use any2api_domain::{
    ConfigRevision, OAuthAccount, OAuthAccountDraft, OAuthAccountId, ProviderKind,
    RequestsPerMinute, RoutingCredentialId,
};
use any2api_runtime::api::{OAuthService, PublishedSnapshot, UpstreamCredentialUsageSummary};
use serde::{Deserialize, Serialize};

use crate::admin::{
    credential_runtime::CredentialRuntimeResponse, error::AdminApiError,
    request_usage::RequestUsageResponse, revision::parse_revision, upstream_usage,
};

use super::super::proxy_selection::OAuthProxySelectionDto;
use super::super::refresh_diagnostic::OAuthRefreshFailureResponse;

#[derive(Debug, Serialize)]
pub(super) struct OAuthAccountCollectionResponse {
    config_revision: u64,
    items: Vec<OAuthAccountResponse>,
}

impl OAuthAccountCollectionResponse {
    pub(super) fn from_snapshot(
        snapshot: &PublishedSnapshot,
        usage: &[UpstreamCredentialUsageSummary],
        oauth: Option<&OAuthService>,
    ) -> Self {
        let accounts = accounts_for_management(snapshot.oauth_accounts().accounts());
        Self {
            config_revision: snapshot.revision().get(),
            items: accounts
                .into_iter()
                .map(|account| OAuthAccountResponse::from_snapshot(account, snapshot, usage, oauth))
                .collect(),
        }
    }
}

fn accounts_for_management(accounts: &[OAuthAccount]) -> Vec<&OAuthAccount> {
    let mut accounts = accounts.iter().collect::<Vec<_>>();
    accounts.sort_by(|left, right| {
        left.provider_kind()
            .cmp(&right.provider_kind())
            .then_with(|| left.created_at().cmp(right.created_at()))
            .then_with(|| left.id().cmp(&right.id()))
    });
    accounts
}

#[derive(Debug, Serialize)]
struct OAuthAccountResponse {
    id: OAuthAccountId,
    provider_kind: ProviderKind,
    label: String,
    proxy_selection: OAuthProxySelectionDto,
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
    /// Safe Grok Build `bot_flag_source` derivation; never exposes JWT claims.
    bot_flagged: Option<bool>,
    token_refresh_failure: Option<OAuthRefreshFailureResponse>,
    runtime: CredentialRuntimeResponse,
    usage: RequestUsageResponse,
}

impl OAuthAccountResponse {
    fn from_snapshot(
        account: &OAuthAccount,
        snapshot: &PublishedSnapshot,
        usage: &[UpstreamCredentialUsageSummary],
        oauth: Option<&OAuthService>,
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
        let runtime = snapshot
            .credential_runtime_observation(RoutingCredentialId::oauth_account(account.id()))
            .expect("published OAuth account has runtime observation");
        Self {
            id: account.id(),
            provider_kind: account.provider_kind(),
            label: account.label().to_owned(),
            proxy_selection: account.proxy_selection().into(),
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
            bot_flagged: snapshot.oauth_grok_bot_flag(account.id()),
            token_refresh_failure: oauth
                .and_then(|oauth| oauth.refresh_failure(account.id(), account.token_version()))
                .map(Into::into),
            runtime: runtime.into(),
            usage: upstream_usage::for_id(RoutingCredentialId::oauth_account(account.id()), usage),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OAuthAccountUpdateRequest {
    expected_revision: u64,
    expected_config_version: u64,
    label: String,
    proxy_selection: OAuthProxySelectionDto,
    requests_per_minute: Option<u32>,
    enabled: bool,
}

impl OAuthAccountUpdateRequest {
    pub(super) fn into_domain(
        self,
    ) -> Result<
        (
            ConfigRevision,
            u64,
            OAuthAccountDraft,
            any2api_domain::OAuthProxySelection,
        ),
        AdminApiError,
    > {
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
            self.proxy_selection.into(),
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

#[cfg(test)]
mod tests {
    use any2api_domain::{
        OAuthAccount, OAuthAccountConfiguration, OAuthAccountDraft, OAuthAccountId,
        OAuthProxySelection, ProviderKind, ProxyConfiguration, ProxyProfile, ProxyProfileId,
    };
    use uuid::Uuid;

    use super::accounts_for_management;

    #[test]
    fn management_order_is_oldest_first_without_changing_routing_order() {
        let accounts = vec![
            account(4, "A New", "2026-08-05 00:00:03"),
            account(3, "D Same Time", "2026-08-05 00:00:02"),
            account(2, "C Same Time", "2026-08-05 00:00:02"),
            account(1, "Z Old", "2026-08-05 00:00:01"),
        ];
        let proxies = ProxyConfiguration::new(vec![ProxyProfile::direct()], ProxyProfileId::DIRECT)
            .expect("proxy configuration");
        let configuration =
            OAuthAccountConfiguration::new(accounts, &proxies).expect("OAuth configuration");

        assert_eq!(
            labels(configuration.accounts().iter()),
            ["A New", "C Same Time", "D Same Time", "Z Old"]
        );
        assert_eq!(
            labels(accounts_for_management(configuration.accounts()).into_iter()),
            ["Z Old", "C Same Time", "D Same Time", "A New"]
        );
    }

    fn account(id: u128, label: &str, created_at: &str) -> OAuthAccount {
        OAuthAccount::create(
            OAuthAccountId::from_uuid(Uuid::from_u128(id)),
            ProviderKind::Codex,
            OAuthAccountDraft::new(label, None, true).expect("OAuth account draft"),
            OAuthProxySelection::Global,
            None,
            None,
            created_at,
            vec!["gpt-test".to_owned()],
        )
        .expect("OAuth account")
    }

    fn labels<'a>(accounts: impl Iterator<Item = &'a OAuthAccount>) -> Vec<&'a str> {
        accounts.map(OAuthAccount::label).collect()
    }
}
