use any2api_domain::{
    ConfigRevision, OAuthAccount, OAuthAccountId, ProviderKind, RequestsPerMinute,
};

pub struct OAuthStartResult {
    provider: ProviderKind,
    session_id: String,
    expires_in_seconds: u64,
    flow: OAuthStartFlow,
}

pub enum OAuthStartFlow {
    AuthorizationCode {
        authorization_url: String,
        redirect_uri: &'static str,
    },
    DeviceCode {
        user_code: String,
        verification_uri: String,
        verification_uri_complete: Option<String>,
        poll_interval_seconds: u64,
    },
}

impl OAuthStartResult {
    pub(super) fn authorization_code(
        provider: ProviderKind,
        session_id: String,
        authorization_url: String,
        redirect_uri: &'static str,
        expires_in_seconds: u64,
    ) -> Self {
        Self {
            provider,
            session_id,
            expires_in_seconds,
            flow: OAuthStartFlow::AuthorizationCode {
                authorization_url,
                redirect_uri,
            },
        }
    }

    pub(super) fn device_code(
        provider: ProviderKind,
        session_id: String,
        user_code: String,
        verification_uri: String,
        verification_uri_complete: Option<String>,
        expires_in_seconds: u64,
        poll_interval_seconds: u64,
    ) -> Self {
        Self {
            provider,
            session_id,
            expires_in_seconds,
            flow: OAuthStartFlow::DeviceCode {
                user_code,
                verification_uri,
                verification_uri_complete,
                poll_interval_seconds,
            },
        }
    }

    #[must_use]
    pub fn into_parts(self) -> (ProviderKind, String, u64, OAuthStartFlow) {
        (
            self.provider,
            self.session_id,
            self.expires_in_seconds,
            self.flow,
        )
    }
}

pub enum OAuthDevicePollResult {
    Pending { retry_after_seconds: u64 },
    Complete(OAuthActivationResult),
}

pub struct OAuthActivationResult {
    config_revision: ConfigRevision,
    account: OAuthAccount,
}

impl OAuthActivationResult {
    pub(super) const fn new(config_revision: ConfigRevision, account: OAuthAccount) -> Self {
        Self {
            config_revision,
            account,
        }
    }

    #[must_use]
    pub const fn provider(&self) -> ProviderKind {
        self.account.provider_kind()
    }

    #[must_use]
    pub const fn account_id(&self) -> OAuthAccountId {
        self.account.id()
    }

    #[must_use]
    pub fn label(&self) -> &str {
        self.account.label()
    }

    #[must_use]
    pub const fn requests_per_minute(&self) -> Option<RequestsPerMinute> {
        self.account.requests_per_minute()
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.account.enabled()
    }

    #[must_use]
    pub fn safe_account_email(&self) -> Option<&str> {
        self.account.safe_account_email()
    }

    #[must_use]
    pub const fn expires_at(&self) -> Option<i64> {
        self.account.expires_at()
    }

    #[must_use]
    pub fn selected_model_count(&self) -> usize {
        self.account.models().len()
    }

    #[must_use]
    pub const fn config_version(&self) -> u64 {
        self.account.config_version()
    }

    #[must_use]
    pub const fn config_revision(&self) -> ConfigRevision {
        self.config_revision
    }
}
