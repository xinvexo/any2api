use any2api_domain::{OAuthAccountId, ProviderKind, RequestsPerMinute};
use any2api_runtime::api::{
    OAuthActivationResult, OAuthDevicePollResult, OAuthStartFlow, OAuthStartResult,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OAuthStartRequest {
    provider: ProviderKind,
}

impl OAuthStartRequest {
    pub(super) const fn provider(&self) -> ProviderKind {
        self.provider
    }
}

#[derive(Serialize)]
#[serde(tag = "flow", rename_all = "snake_case")]
pub(super) enum OAuthStartResponse {
    AuthorizationCode {
        provider: ProviderKind,
        session_id: String,
        authorization_url: String,
        redirect_uri: &'static str,
        expires_in_seconds: u64,
    },
    DeviceCode {
        provider: ProviderKind,
        session_id: String,
        user_code: String,
        verification_uri: String,
        verification_uri_complete: Option<String>,
        expires_in_seconds: u64,
        poll_interval_seconds: u64,
    },
}

impl From<OAuthStartResult> for OAuthStartResponse {
    fn from(result: OAuthStartResult) -> Self {
        let (provider, session_id, expires_in_seconds, flow) = result.into_parts();
        match flow {
            OAuthStartFlow::AuthorizationCode {
                authorization_url,
                redirect_uri,
            } => Self::AuthorizationCode {
                provider,
                session_id,
                authorization_url,
                redirect_uri,
                expires_in_seconds,
            },
            OAuthStartFlow::DeviceCode {
                user_code,
                verification_uri,
                verification_uri_complete,
                poll_interval_seconds,
            } => Self::DeviceCode {
                provider,
                session_id,
                user_code,
                verification_uri,
                verification_uri_complete,
                expires_in_seconds,
                poll_interval_seconds,
            },
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OAuthDevicePollRequest {
    session_id: String,
}

impl OAuthDevicePollRequest {
    pub(super) fn into_session_id(self) -> String {
        self.session_id
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OAuthExchangeRequest {
    session_id: String,
    callback_url: String,
}

impl OAuthExchangeRequest {
    pub(super) fn into_parts(self) -> (String, String) {
        (self.session_id, self.callback_url)
    }
}

#[derive(Serialize)]
pub(super) struct OAuthExchangeResponse {
    provider: ProviderKind,
    account_id: OAuthAccountId,
    label: String,
    requests_per_minute: Option<u32>,
    enabled: bool,
    safe_account_email: Option<String>,
    expires_at: Option<i64>,
    selected_model_count: usize,
    config_version: u64,
    config_revision: u64,
}

impl From<OAuthActivationResult> for OAuthExchangeResponse {
    fn from(result: OAuthActivationResult) -> Self {
        Self {
            provider: result.provider(),
            account_id: result.account_id(),
            label: result.label().to_owned(),
            requests_per_minute: result.requests_per_minute().map(RequestsPerMinute::get),
            enabled: result.enabled(),
            safe_account_email: result.safe_account_email().map(str::to_owned),
            expires_at: result.expires_at(),
            selected_model_count: result.selected_model_count(),
            config_version: result.config_version(),
            config_revision: result.config_revision().get(),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum OAuthDevicePollResponse {
    Pending { retry_after_seconds: u64 },
    Complete { account: OAuthExchangeResponse },
}

impl From<OAuthDevicePollResult> for OAuthDevicePollResponse {
    fn from(result: OAuthDevicePollResult) -> Self {
        match result {
            OAuthDevicePollResult::Pending {
                retry_after_seconds,
            } => Self::Pending {
                retry_after_seconds,
            },
            OAuthDevicePollResult::Complete(account) => Self::Complete {
                account: account.into(),
            },
        }
    }
}
