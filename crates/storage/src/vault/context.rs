use any2api_domain::{CredentialId, CredentialKind, ProviderKind, ProxyProfileId};

pub(crate) const AAD_VERSION: u16 = 1;
const AAD_DOMAIN: &[u8] = b"any2api-secret-aad";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretContext {
    ProviderCredential {
        credential_id: CredentialId,
        provider_kind: ProviderKind,
        credential_kind: CredentialKind,
    },
    ProxyPassword {
        proxy_profile_id: ProxyProfileId,
    },
}

impl SecretContext {
    #[must_use]
    pub const fn provider_credential(
        credential_id: CredentialId,
        provider_kind: ProviderKind,
        credential_kind: CredentialKind,
    ) -> Self {
        Self::ProviderCredential {
            credential_id,
            provider_kind,
            credential_kind,
        }
    }

    #[must_use]
    pub const fn proxy_password(proxy_profile_id: ProxyProfileId) -> Self {
        Self::ProxyPassword { proxy_profile_id }
    }

    pub(crate) fn encode_aad(self) -> Vec<u8> {
        let mut aad = Vec::with_capacity(AAD_DOMAIN.len() + 24);
        aad.extend_from_slice(AAD_DOMAIN);
        aad.extend_from_slice(&AAD_VERSION.to_be_bytes());
        match self {
            Self::ProviderCredential {
                credential_id,
                provider_kind,
                credential_kind,
            } => {
                aad.push(1);
                aad.extend_from_slice(credential_id.as_uuid().as_bytes());
                aad.push(provider_kind_code(provider_kind));
                aad.push(credential_kind_code(credential_kind));
            }
            Self::ProxyPassword { proxy_profile_id } => {
                aad.push(2);
                aad.extend_from_slice(proxy_profile_id.as_uuid().as_bytes());
            }
        }
        aad
    }
}

const fn provider_kind_code(kind: ProviderKind) -> u8 {
    match kind {
        ProviderKind::Codex => 1,
        ProviderKind::Claude => 2,
    }
}

const fn credential_kind_code(kind: CredentialKind) -> u8 {
    match kind {
        CredentialKind::ApiKey => 1,
    }
}
