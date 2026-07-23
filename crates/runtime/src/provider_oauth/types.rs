use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, MaxConcurrency, ProviderCredentialDraft,
    ProviderCredentialValidationError, ProviderEndpointId, ProviderKind, ProxyProfileId,
};

pub struct ProviderOAuthStartRequest {
    pub(super) expected_revision: ConfigRevision,
    pub(super) draft: ProviderCredentialDraft,
}

impl ProviderOAuthStartRequest {
    pub fn new(
        expected_revision: ConfigRevision,
        label: impl Into<String>,
        proxy_profile_id: ProxyProfileId,
        max_concurrency: MaxConcurrency,
        enabled: bool,
    ) -> Result<Self, ProviderCredentialValidationError> {
        Ok(Self {
            expected_revision,
            draft: ProviderCredentialDraft::new(
                label,
                CredentialKind::OAuth2,
                proxy_profile_id,
                max_concurrency,
                enabled,
            )?,
        })
    }
}

pub struct ProviderOAuthStartResult {
    session_id: String,
    authorization_url: String,
    redirect_uri: &'static str,
    expires_in_seconds: u64,
}

impl ProviderOAuthStartResult {
    pub(super) fn new(
        session_id: String,
        authorization_url: String,
        redirect_uri: &'static str,
        expires_in_seconds: u64,
    ) -> Self {
        Self {
            session_id,
            authorization_url,
            redirect_uri,
            expires_in_seconds,
        }
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

pub struct ProviderOAuthExchangeResult {
    config_revision: ConfigRevision,
    provider_endpoint_id: ProviderEndpointId,
    credential_id: CredentialId,
    provider_kind: ProviderKind,
    account_id: Option<String>,
    email: Option<String>,
    organization_id: Option<String>,
}

impl ProviderOAuthExchangeResult {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        config_revision: ConfigRevision,
        provider_endpoint_id: ProviderEndpointId,
        credential_id: CredentialId,
        provider_kind: ProviderKind,
        account_id: Option<String>,
        email: Option<String>,
        organization_id: Option<String>,
    ) -> Self {
        Self {
            config_revision,
            provider_endpoint_id,
            credential_id,
            provider_kind,
            account_id,
            email,
            organization_id,
        }
    }

    #[must_use]
    pub const fn config_revision(&self) -> ConfigRevision {
        self.config_revision
    }

    #[must_use]
    pub const fn provider_endpoint_id(&self) -> ProviderEndpointId {
        self.provider_endpoint_id
    }

    #[must_use]
    pub const fn credential_id(&self) -> CredentialId {
        self.credential_id
    }

    #[must_use]
    pub const fn provider_kind(&self) -> ProviderKind {
        self.provider_kind
    }

    #[must_use]
    pub fn account_id(&self) -> Option<&str> {
        self.account_id.as_deref()
    }

    #[must_use]
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    #[must_use]
    pub fn organization_id(&self) -> Option<&str> {
        self.organization_id.as_deref()
    }
}
