use any2api_domain::{MaxConcurrency, ProviderEndpointId, ProviderKind, ProxyProfileId};
use any2api_runtime::api::{
    ProviderOAuthExchangeResult, ProviderOAuthStartRequest, ProviderOAuthStartResult,
};
use serde::{Deserialize, Serialize};

use super::{error::AdminApiError, revision::parse_revision};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderOAuthStartBody {
    expected_revision: u64,
    label: String,
    proxy_profile_id: ProxyProfileId,
    max_concurrency: u32,
    enabled: bool,
}

impl ProviderOAuthStartBody {
    pub(crate) fn into_domain(self) -> Result<ProviderOAuthStartRequest, AdminApiError> {
        let max_concurrency = MaxConcurrency::new(self.max_concurrency)
            .map_err(|error| AdminApiError::invalid_provider_credential(error.to_string()))?;
        ProviderOAuthStartRequest::new(
            parse_revision(self.expected_revision)?,
            self.label,
            self.proxy_profile_id,
            max_concurrency,
            self.enabled,
        )
        .map_err(|error| AdminApiError::invalid_provider_credential(error.to_string()))
    }
}

#[derive(Serialize)]
pub(crate) struct ProviderOAuthStartResponse {
    session_id: String,
    authorization_url: String,
    redirect_uri: String,
    expires_in_seconds: u64,
}

impl From<ProviderOAuthStartResult> for ProviderOAuthStartResponse {
    fn from(result: ProviderOAuthStartResult) -> Self {
        Self {
            session_id: result.session_id().to_owned(),
            authorization_url: result.authorization_url().to_owned(),
            redirect_uri: result.redirect_uri().to_owned(),
            expires_in_seconds: result.expires_in_seconds(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderOAuthExchangeBody {
    session_id: String,
    callback_url: String,
}

impl ProviderOAuthExchangeBody {
    pub(crate) fn into_parts(self) -> (String, String) {
        (self.session_id, self.callback_url)
    }
}

#[derive(Serialize)]
pub(crate) struct ProviderOAuthExchangeResponse {
    config_revision: u64,
    provider_endpoint_id: ProviderEndpointId,
    credential_id: any2api_domain::CredentialId,
    provider_kind: ProviderKind,
    account_id: Option<String>,
    email: Option<String>,
    organization_id: Option<String>,
}

impl From<ProviderOAuthExchangeResult> for ProviderOAuthExchangeResponse {
    fn from(result: ProviderOAuthExchangeResult) -> Self {
        Self {
            config_revision: result.config_revision().get(),
            provider_endpoint_id: result.provider_endpoint_id(),
            credential_id: result.credential_id(),
            provider_kind: result.provider_kind(),
            account_id: result.account_id().map(str::to_owned),
            email: result.email().map(str::to_owned),
            organization_id: result.organization_id().map(str::to_owned),
        }
    }
}
