use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderKind, TransportMode,
};

use super::OAuthLoginFlow;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OAuthCapabilities {
    operations: &'static [ProtocolOperation],
    login_flow: OAuthLoginFlow,
    quota: bool,
    quota_supplement: bool,
    quota_reset: bool,
}

impl OAuthCapabilities {
    #[must_use]
    pub const fn new(operations: &'static [ProtocolOperation], login_flow: OAuthLoginFlow) -> Self {
        Self {
            operations,
            login_flow,
            quota: false,
            quota_supplement: false,
            quota_reset: false,
        }
    }

    #[must_use]
    pub const fn with_quota(mut self) -> Self {
        self.quota = true;
        self
    }

    #[must_use]
    pub const fn with_quota_supplement(mut self) -> Self {
        self.quota = true;
        self.quota_supplement = true;
        self
    }

    #[must_use]
    pub const fn with_quota_reset(mut self) -> Self {
        self.quota = true;
        self.quota_reset = true;
        self
    }

    #[must_use]
    pub const fn operations(self) -> &'static [ProtocolOperation] {
        self.operations
    }

    #[must_use]
    pub const fn login_flow(self) -> OAuthLoginFlow {
        self.login_flow
    }

    #[must_use]
    pub const fn supports_quota(self) -> bool {
        self.quota
    }

    #[must_use]
    pub const fn supports_quota_supplement(self) -> bool {
        self.quota_supplement
    }

    #[must_use]
    pub const fn supports_quota_reset(self) -> bool {
        self.quota_reset
    }

    #[must_use]
    pub fn supports_operation(self, operation: ProtocolOperation) -> bool {
        self.operations.contains(&operation)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderDescriptor {
    kind: ProviderKind,
    api_key_operations: &'static [ProtocolOperation],
    oauth: Option<OAuthCapabilities>,
    transport_modes: &'static [TransportMode],
}

impl ProviderDescriptor {
    #[must_use]
    pub const fn new(
        kind: ProviderKind,
        api_key_operations: &'static [ProtocolOperation],
        oauth: Option<OAuthCapabilities>,
        transport_modes: &'static [TransportMode],
    ) -> Self {
        Self {
            kind,
            api_key_operations,
            oauth,
            transport_modes,
        }
    }

    #[must_use]
    pub const fn kind(self) -> ProviderKind {
        self.kind
    }

    #[must_use]
    pub const fn id(self) -> &'static str {
        self.kind.as_str()
    }

    #[must_use]
    pub const fn api_key_operations(self) -> &'static [ProtocolOperation] {
        self.api_key_operations
    }

    #[must_use]
    pub const fn oauth(self) -> Option<OAuthCapabilities> {
        self.oauth
    }

    #[must_use]
    pub const fn transport_modes(self) -> &'static [TransportMode] {
        self.transport_modes
    }

    pub fn protocols(self) -> impl Iterator<Item = ProtocolDialect> {
        ProtocolDialect::ALL
            .into_iter()
            .filter(move |dialect| self.supports_protocol(*dialect))
    }

    #[must_use]
    pub fn supports_protocol(self, dialect: ProtocolDialect) -> bool {
        self.api_key_operations
            .iter()
            .chain(
                self.oauth
                    .into_iter()
                    .flat_map(|oauth| oauth.operations().iter()),
            )
            .any(|operation| operation.dialect() == dialect)
    }

    #[must_use]
    pub fn supports_api_key_operation(self, operation: ProtocolOperation) -> bool {
        self.api_key_operations.contains(&operation)
    }

    #[must_use]
    pub fn supports_oauth_operation(self, operation: ProtocolOperation) -> bool {
        self.oauth
            .is_some_and(|oauth| oauth.supports_operation(operation))
    }

    #[must_use]
    pub const fn supports_oauth(self) -> bool {
        self.oauth.is_some()
    }

    #[must_use]
    pub fn supports_transport_mode(self, mode: TransportMode) -> bool {
        self.transport_modes.contains(&mode)
    }

    #[must_use]
    pub const fn supports_credential_kind(self, kind: CredentialKind) -> bool {
        match kind {
            CredentialKind::ApiKey => !self.api_key_operations.is_empty(),
        }
    }

    pub(crate) fn validate(self) -> Result<(), &'static str> {
        if self.api_key_operations.is_empty() && self.oauth.is_none() {
            return Err("provider has no credential mode");
        }
        if has_duplicates(self.api_key_operations) {
            return Err("API-key operation is duplicated");
        }
        if has_duplicates(self.transport_modes) || self.transport_modes.is_empty() {
            return Err("transport modes are empty or duplicated");
        }
        if let Some(oauth) = self.oauth
            && (oauth.operations().is_empty() || has_duplicates(oauth.operations()))
        {
            return Err("OAuth operations are empty or duplicated");
        }
        Ok(())
    }
}

fn has_duplicates<T: PartialEq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].contains(value))
}
