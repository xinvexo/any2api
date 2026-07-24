use any2api_domain::{ConfigRevision, MaxConcurrency, OAuthAccount, OAuthAccountId, ProviderKind};

pub struct OAuthStartResult {
    provider: ProviderKind,
    session_id: String,
    authorization_url: String,
    redirect_uri: &'static str,
    expires_in_seconds: u64,
}

impl OAuthStartResult {
    pub(super) fn new(
        provider: ProviderKind,
        session_id: String,
        authorization_url: String,
        redirect_uri: &'static str,
        expires_in_seconds: u64,
    ) -> Self {
        Self {
            provider,
            session_id,
            authorization_url,
            redirect_uri,
            expires_in_seconds,
        }
    }

    #[must_use]
    pub const fn provider(&self) -> ProviderKind {
        self.provider
    }

    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    #[must_use]
    pub fn authorization_url(&self) -> &str {
        &self.authorization_url
    }

    #[must_use]
    pub const fn redirect_uri(&self) -> &'static str {
        self.redirect_uri
    }

    #[must_use]
    pub const fn expires_in_seconds(&self) -> u64 {
        self.expires_in_seconds
    }
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
    pub const fn max_concurrency(&self) -> MaxConcurrency {
        self.account.max_concurrency()
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
