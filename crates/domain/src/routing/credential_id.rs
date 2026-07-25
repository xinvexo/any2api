use std::fmt;

use crate::{CredentialId, OAuthAccountId};

/// Runtime identity for either supported source of upstream authentication.
///
/// The source tag is part of the identity so an OAuth account can never be
/// mistaken for a ProviderCredential, even if their UUID values collide.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RoutingCredentialId {
    ProviderCredential(CredentialId),
    OAuthAccount(OAuthAccountId),
}

impl RoutingCredentialId {
    #[must_use]
    pub const fn provider_credential(id: CredentialId) -> Self {
        Self::ProviderCredential(id)
    }

    #[must_use]
    pub const fn oauth_account(id: OAuthAccountId) -> Self {
        Self::OAuthAccount(id)
    }

    #[must_use]
    pub const fn provider_credential_id(self) -> Option<CredentialId> {
        match self {
            Self::ProviderCredential(id) => Some(id),
            Self::OAuthAccount(_) => None,
        }
    }

    #[must_use]
    pub const fn oauth_account_id(self) -> Option<OAuthAccountId> {
        match self {
            Self::ProviderCredential(_) => None,
            Self::OAuthAccount(id) => Some(id),
        }
    }

    #[must_use]
    pub const fn source_uuid(self) -> uuid::Uuid {
        match self {
            Self::ProviderCredential(id) => *id.as_uuid(),
            Self::OAuthAccount(id) => *id.as_uuid(),
        }
    }
}

impl From<CredentialId> for RoutingCredentialId {
    fn from(value: CredentialId) -> Self {
        Self::provider_credential(value)
    }
}

impl From<OAuthAccountId> for RoutingCredentialId {
    fn from(value: OAuthAccountId) -> Self {
        Self::oauth_account(value)
    }
}

impl fmt::Display for RoutingCredentialId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProviderCredential(id) => write!(formatter, "provider_credential:{id}"),
            Self::OAuthAccount(id) => write!(formatter, "oauth_account:{id}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{CredentialId, OAuthAccountId};

    use super::RoutingCredentialId;

    #[test]
    fn source_tag_is_part_of_the_identity() {
        let uuid = uuid::Uuid::new_v4();
        let provider = RoutingCredentialId::provider_credential(CredentialId::from_uuid(uuid));
        let oauth = RoutingCredentialId::oauth_account(OAuthAccountId::from_uuid(uuid));

        assert_ne!(provider, oauth);
        assert_eq!(
            provider.provider_credential_id(),
            Some(CredentialId::from_uuid(uuid))
        );
        assert_eq!(
            oauth.oauth_account_id(),
            Some(OAuthAccountId::from_uuid(uuid))
        );
    }
}
