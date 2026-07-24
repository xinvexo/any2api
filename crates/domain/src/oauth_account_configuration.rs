use std::collections::{HashMap, HashSet};

use crate::{
    OAuthAccount, OAuthAccountId, OAuthAccountValidationError, ProviderKind, ProxyConfiguration,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OAuthAccountConfiguration {
    accounts: Vec<OAuthAccount>,
}

impl OAuthAccountConfiguration {
    pub fn new(
        mut accounts: Vec<OAuthAccount>,
        proxies: &ProxyConfiguration,
    ) -> Result<Self, OAuthAccountValidationError> {
        let mut ids = HashSet::new();
        let mut labels = HashMap::new();
        for account in &accounts {
            if !ids.insert(account.id()) {
                return Err(OAuthAccountValidationError::DuplicateId);
            }
            if labels
                .insert((account.provider_kind(), account.label_key()), account.id())
                .is_some()
            {
                return Err(OAuthAccountValidationError::DuplicateLabel);
            }
            if proxies.get(account.proxy_profile_id()).is_none() {
                return Err(OAuthAccountValidationError::MissingProxyProfile);
            }
        }
        accounts.sort_by(|left, right| {
            left.provider_kind()
                .cmp(&right.provider_kind())
                .then_with(|| left.label().cmp(right.label()))
        });
        Ok(Self { accounts })
    }

    #[must_use]
    pub const fn initial() -> Self {
        Self {
            accounts: Vec::new(),
        }
    }

    #[must_use]
    pub fn accounts(&self) -> &[OAuthAccount] {
        &self.accounts
    }

    #[must_use]
    pub fn get(&self, id: OAuthAccountId) -> Option<&OAuthAccount> {
        self.accounts.iter().find(|account| account.id() == id)
    }

    pub fn for_provider(&self, provider: ProviderKind) -> impl Iterator<Item = &OAuthAccount> {
        self.accounts
            .iter()
            .filter(move |account| account.provider_kind() == provider)
    }
}
