use std::collections::HashSet;

use any2api_domain::ProviderKind;
use any2api_provider::api::OAuthTokenMaterial;
use sha2::{Digest, Sha256};

#[derive(Clone, Eq, Hash, PartialEq)]
pub(crate) struct OAuthAccountIdentity {
    provider: ProviderKind,
    key: OAuthAccountIdentityKey,
}

#[derive(Clone, Eq, Hash, PartialEq)]
enum OAuthAccountIdentityKey {
    AccountId(String),
    Email(String),
}

pub(crate) struct OAuthImportIdentity {
    keys: Vec<OAuthImportIdentityKey>,
}

#[derive(Clone, Eq, Hash, PartialEq)]
enum OAuthImportIdentityKey {
    Stable(OAuthAccountIdentity),
    Secret([u8; 32]),
}

#[derive(Default)]
pub(crate) struct OAuthImportIdentityIndex {
    keys: HashSet<OAuthImportIdentityKey>,
}

impl OAuthAccountIdentity {
    pub(crate) fn from_token(token: &OAuthTokenMaterial) -> Option<Self> {
        let key = token
            .account_id()
            .and_then(normalize_account_id)
            .map(OAuthAccountIdentityKey::AccountId)
            .or_else(|| {
                token
                    .email()
                    .and_then(normalize_email)
                    .map(OAuthAccountIdentityKey::Email)
            })?;
        Some(Self {
            provider: token.provider(),
            key,
        })
    }

    pub(crate) fn matches(&self, token: &OAuthTokenMaterial) -> bool {
        if token.provider() != self.provider {
            return false;
        }
        match &self.key {
            OAuthAccountIdentityKey::AccountId(expected) => token
                .account_id()
                .and_then(normalize_account_id)
                .is_some_and(|actual| actual == *expected),
            OAuthAccountIdentityKey::Email(expected) => {
                token.account_id().and_then(normalize_account_id).is_none()
                    && token
                        .email()
                        .and_then(normalize_email)
                        .is_some_and(|actual| actual == *expected)
            }
        }
    }
}

impl OAuthImportIdentity {
    pub(crate) fn from_token(token: &OAuthTokenMaterial) -> Self {
        let mut keys = Vec::with_capacity(4);
        if let Some(identity) = OAuthAccountIdentity::from_token(token) {
            keys.push(OAuthImportIdentityKey::Stable(identity));
        }
        keys.push(OAuthImportIdentityKey::Secret(secret_digest(
            token.provider(),
            b"access",
            token.access_token(),
        )));
        if let Some(refresh_token) = token.refresh_token() {
            keys.push(OAuthImportIdentityKey::Secret(secret_digest(
                token.provider(),
                b"refresh",
                refresh_token,
            )));
        }
        if let Some(id_token) = token.id_token() {
            keys.push(OAuthImportIdentityKey::Secret(secret_digest(
                token.provider(),
                b"id",
                id_token,
            )));
        }
        Self { keys }
    }
}

impl OAuthImportIdentityIndex {
    pub(crate) fn include_existing(&mut self, identity: &OAuthImportIdentity) {
        self.keys.extend(identity.keys.iter().cloned());
    }

    pub(crate) fn insert_new(&mut self, identity: &OAuthImportIdentity) -> bool {
        if identity.keys.iter().any(|key| self.keys.contains(key)) {
            return false;
        }
        self.include_existing(identity);
        true
    }
}

fn secret_digest(provider: ProviderKind, kind: &[u8], secret: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"any2api.oauth-import-identity.v1\0");
    digest.update(provider.as_str().as_bytes());
    digest.update(b"\0");
    digest.update(kind);
    digest.update(b"\0");
    digest.update(
        u64::try_from(secret.len())
            .expect("OAuth token length fits u64")
            .to_be_bytes(),
    );
    digest.update(secret.as_bytes());
    digest.finalize().into()
}

fn normalize_account_id(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn normalize_email(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_lowercase())
}

#[cfg(test)]
mod tests {
    use any2api_domain::ProviderKind;
    use any2api_provider::api::OAuthTokenMaterial;

    use super::{OAuthAccountIdentity, OAuthImportIdentity, OAuthImportIdentityIndex};

    fn token(
        provider: ProviderKind,
        account_id: Option<&str>,
        email: Option<&str>,
    ) -> OAuthTokenMaterial {
        token_with_access(provider, "access-token", account_id, email)
    }

    fn token_with_access(
        provider: ProviderKind,
        access_token: &str,
        account_id: Option<&str>,
        email: Option<&str>,
    ) -> OAuthTokenMaterial {
        OAuthTokenMaterial::new(
            provider,
            access_token.into(),
            None,
            None,
            None,
            account_id.map(str::to_owned),
            email.map(str::to_owned),
        )
        .expect("token")
    }

    #[test]
    fn account_id_takes_priority_over_email_and_provider_isolated() {
        let identity = OAuthAccountIdentity::from_token(&token(
            ProviderKind::Codex,
            Some("account-a"),
            Some("person@example.com"),
        ))
        .expect("identity");

        assert!(identity.matches(&token(
            ProviderKind::Codex,
            Some("account-a"),
            Some("renamed@example.com"),
        )));
        assert!(!identity.matches(&token(
            ProviderKind::Codex,
            Some("account-b"),
            Some("person@example.com"),
        )));
        assert!(!identity.matches(&token(
            ProviderKind::Grok,
            Some("account-a"),
            Some("person@example.com"),
        )));
    }

    #[test]
    fn email_only_matches_another_token_without_account_id() {
        let identity = OAuthAccountIdentity::from_token(&token(
            ProviderKind::Claude,
            None,
            Some(" Person@Example.COM "),
        ))
        .expect("identity");

        assert!(identity.matches(&token(
            ProviderKind::Claude,
            None,
            Some("person@example.com"),
        )));
        assert!(!identity.matches(&token(
            ProviderKind::Claude,
            Some("stable-id"),
            Some("person@example.com"),
        )));
        assert!(
            OAuthAccountIdentity::from_token(&token(ProviderKind::Claude, None, None)).is_none()
        );
    }

    #[test]
    fn import_identity_rejects_stable_or_exact_secret_duplicates() {
        let stable = token(
            ProviderKind::Codex,
            Some("account-a"),
            Some("person@example.com"),
        );
        let same_stable = token(
            ProviderKind::Codex,
            Some("account-a"),
            Some("renamed@example.com"),
        );
        let mut index = OAuthImportIdentityIndex::default();
        assert!(index.insert_new(&OAuthImportIdentity::from_token(&stable)));
        assert!(!index.insert_new(&OAuthImportIdentity::from_token(&same_stable)));

        let first = OAuthTokenMaterial::new(
            ProviderKind::Claude,
            "same-access".into(),
            Some("same-refresh".into()),
            None,
            None,
            None,
            None,
        )
        .expect("first token");
        let same_refresh = OAuthTokenMaterial::new(
            ProviderKind::Claude,
            "other-access".into(),
            Some("same-refresh".into()),
            None,
            None,
            None,
            None,
        )
        .expect("second token");
        assert!(index.insert_new(&OAuthImportIdentity::from_token(&first)));
        assert!(!index.insert_new(&OAuthImportIdentity::from_token(&same_refresh)));
    }

    #[test]
    fn import_identity_keeps_unproven_accounts_distinct() {
        let first = token_with_access(
            ProviderKind::Claude,
            "access-a",
            Some("account-a"),
            Some("shared@example.com"),
        );
        let second = token_with_access(
            ProviderKind::Claude,
            "access-b",
            Some("account-b"),
            Some("shared@example.com"),
        );
        let third = token_with_access(
            ProviderKind::Claude,
            "access-c",
            None,
            Some("shared@example.com"),
        );
        let mut index = OAuthImportIdentityIndex::default();

        assert!(index.insert_new(&OAuthImportIdentity::from_token(&first)));
        assert!(index.insert_new(&OAuthImportIdentity::from_token(&second)));
        assert!(index.insert_new(&OAuthImportIdentity::from_token(&third)));
    }
}
