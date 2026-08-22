use std::collections::HashSet;

use any2api_domain::ProviderKind;
use any2api_provider::api::{OAuthPrincipalIdentity, OAuthTokenMaterial, ProviderDriver};
use sha2::{Digest, Sha256};

pub(crate) struct OAuthImportIdentity {
    keys: Vec<OAuthImportIdentityKey>,
}

#[derive(Clone, Eq, Hash, PartialEq)]
enum OAuthImportIdentityKey {
    Stable(OAuthPrincipalIdentity),
    Secret([u8; 32]),
}

#[derive(Default)]
pub(crate) struct OAuthImportIdentityIndex {
    keys: HashSet<OAuthImportIdentityKey>,
}

impl OAuthImportIdentity {
    pub(crate) fn from_token(driver: &dyn ProviderDriver, token: &OAuthTokenMaterial) -> Self {
        let mut keys = Vec::with_capacity(4);
        if let Some(identity) = driver
            .oauth_token()
            .and_then(|provider| provider.oauth_principal_identity(token))
        {
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

    pub(crate) fn stable_matches(&self, candidate: &Self) -> bool {
        self.keys.iter().any(|key| {
            matches!(key, OAuthImportIdentityKey::Stable(_))
                && candidate.keys.iter().any(|candidate| candidate == key)
        })
    }

    pub(crate) fn exact_token_matches_identity(&self, candidate: &Self) -> bool {
        self.keys.iter().any(|key| {
            matches!(key, OAuthImportIdentityKey::Secret(_))
                && candidate.keys.iter().any(|candidate| candidate == key)
        })
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
    digest.update(b"any2api.oauth-credential-identity.v1\0");
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

#[cfg(test)]
mod tests;
