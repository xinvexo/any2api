use std::fmt;

use any2api_domain::ProviderKind;
use sha2::{Digest, Sha256};

use super::token::OAuthTokenMaterial;

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct OAuthPrincipalIdentity {
    provider: ProviderKind,
    kind: OAuthPrincipalIdentityKind,
    digest: [u8; 32],
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum OAuthPrincipalIdentityKind {
    AccountId,
    Email,
    WorkspaceMember,
}

impl fmt::Debug for OAuthPrincipalIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            OAuthPrincipalIdentityKind::AccountId => "account_id",
            OAuthPrincipalIdentityKind::Email => "email",
            OAuthPrincipalIdentityKind::WorkspaceMember => "workspace_member",
        };
        formatter
            .debug_struct("OAuthPrincipalIdentity")
            .field("provider", &self.provider)
            .field("kind", &kind)
            .finish()
    }
}

impl OAuthPrincipalIdentity {
    #[must_use]
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

pub(crate) fn default_principal_identity(
    token: &OAuthTokenMaterial,
) -> Option<OAuthPrincipalIdentity> {
    token
        .account_id()
        .and_then(normalize_exact)
        .map(|account_id| {
            principal_identity(
                token.provider(),
                OAuthPrincipalIdentityKind::AccountId,
                &[Some(account_id.as_str())],
            )
        })
        .or_else(|| email_principal_identity(token.provider(), token.email()))
}

pub(crate) fn email_principal_identity(
    provider: ProviderKind,
    email: Option<&str>,
) -> Option<OAuthPrincipalIdentity> {
    normalize_email(email?).map(|email| {
        principal_identity(
            provider,
            OAuthPrincipalIdentityKind::Email,
            &[Some(email.as_str())],
        )
    })
}

pub(crate) fn workspace_member_principal_identity(
    provider: ProviderKind,
    workspace_id: Option<&str>,
    member_id: &str,
) -> Option<OAuthPrincipalIdentity> {
    let workspace_id = workspace_id.and_then(normalize_exact);
    let member_id = normalize_exact(member_id)?;
    Some(principal_identity(
        provider,
        OAuthPrincipalIdentityKind::WorkspaceMember,
        &[workspace_id.as_deref(), Some(member_id.as_str())],
    ))
}

fn principal_identity(
    provider: ProviderKind,
    kind: OAuthPrincipalIdentityKind,
    parts: &[Option<&str>],
) -> OAuthPrincipalIdentity {
    let kind_label: &[u8] = match kind {
        OAuthPrincipalIdentityKind::AccountId => b"account_id",
        OAuthPrincipalIdentityKind::Email => b"email",
        OAuthPrincipalIdentityKind::WorkspaceMember => b"workspace_member",
    };
    let mut digest = Sha256::new();
    digest.update(b"any2api.oauth-principal-identity.v1\0");
    update(&mut digest, Some(provider.as_str().as_bytes()));
    update(&mut digest, Some(kind_label));
    for part in parts {
        update(&mut digest, (*part).map(str::as_bytes));
    }
    OAuthPrincipalIdentity {
        provider,
        kind,
        digest: digest.finalize().into(),
    }
}

fn update(digest: &mut Sha256, value: Option<&[u8]>) {
    match value {
        Some(value) => {
            digest.update([1]);
            digest.update(
                u64::try_from(value.len())
                    .expect("OAuth identity length fits u64")
                    .to_be_bytes(),
            );
            digest.update(value);
        }
        None => digest.update([0]),
    }
}

fn normalize_exact(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn normalize_email(value: &str) -> Option<String> {
    normalize_exact(value).map(|value| value.to_lowercase())
}
