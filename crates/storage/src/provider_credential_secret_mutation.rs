use any2api_domain::{
    CredentialId, CredentialKind, ProviderCredential, ProviderCredentialConfiguration,
    ProviderCredentialDraft, ProviderEndpointConfiguration, ProviderEndpointId, ProxyConfiguration,
};

use crate::{
    error::StorageError,
    provider_api_key::{build_fingerprint, build_oauth2_fingerprint},
    provider_credential_mutation::{
        PreparedProviderCredentialMutation, ProviderCredentialDatabaseChange, map_validation,
        replace, require_config_version,
    },
    vault::{SecretBytes, SecretContext, SecretEnvelope, SecretVault},
};

pub(crate) struct CredentialSecretMutationContext<'a> {
    pub(crate) current: &'a ProviderCredentialConfiguration,
    pub(crate) endpoints: &'a ProviderEndpointConfiguration,
    pub(crate) proxies: &'a ProxyConfiguration,
    pub(crate) vault: &'a SecretVault,
}

impl<'a> CredentialSecretMutationContext<'a> {
    pub(crate) const fn new(
        current: &'a ProviderCredentialConfiguration,
        endpoints: &'a ProviderEndpointConfiguration,
        proxies: &'a ProxyConfiguration,
        vault: &'a SecretVault,
    ) -> Self {
        Self {
            current,
            endpoints,
            proxies,
            vault,
        }
    }
}

pub(crate) fn create(
    context: &CredentialSecretMutationContext<'_>,
    id: CredentialId,
    endpoint_id: ProviderEndpointId,
    draft: ProviderCredentialDraft,
    expected_endpoint_config_version: Option<u64>,
    expected_kind: CredentialKind,
    secret: SecretBytes,
) -> Result<PreparedProviderCredentialMutation, StorageError> {
    require_kind(draft.credential_kind(), expected_kind)?;
    let endpoint = context
        .endpoints
        .get(endpoint_id)
        .ok_or(StorageError::ProviderEndpointNotFound(endpoint_id))?;
    if let Some(expected) = expected_endpoint_config_version
        && endpoint.config_version() != expected
    {
        return Err(StorageError::ProviderEndpointVersionConflict {
            expected,
            actual: endpoint.config_version(),
        });
    }
    let fingerprint = build_secret_fingerprint(
        context.vault,
        endpoint.provider_kind(),
        draft.credential_kind(),
        &secret,
    )?;
    let credential = ProviderCredential::create(id, endpoint_id, draft, fingerprint);
    let envelope = seal(
        context.vault,
        endpoint.provider_kind(),
        &credential,
        &secret,
    )?;
    let mut credentials = context.current.credentials().to_vec();
    credentials.push(credential.clone());
    let configuration =
        ProviderCredentialConfiguration::new(credentials, context.endpoints, context.proxies)
            .map_err(map_validation)?;
    Ok(PreparedProviderCredentialMutation::new(
        configuration,
        ProviderCredentialDatabaseChange::Create {
            credential,
            envelope,
        },
    ))
}

pub(crate) fn rotate_secret(
    context: &CredentialSecretMutationContext<'_>,
    id: CredentialId,
    expected_config_version: Option<u64>,
    expected_secret_version: u64,
    expected_kind: CredentialKind,
    secret: SecretBytes,
) -> Result<PreparedProviderCredentialMutation, StorageError> {
    let existing = context
        .current
        .get(id)
        .ok_or(StorageError::ProviderCredentialNotFound(id))?;
    if let Some(expected_config_version) = expected_config_version {
        require_config_version(existing, expected_config_version)?;
    }
    require_kind(existing.credential_kind(), expected_kind)?;
    if existing.secret_version() != expected_secret_version {
        return Err(StorageError::ProviderCredentialSecretVersionConflict {
            expected: expected_secret_version,
            actual: existing.secret_version(),
        });
    }
    let endpoint = context
        .endpoints
        .get(existing.provider_endpoint_id())
        .ok_or(StorageError::CorruptConfiguration)?;
    let fingerprint = build_secret_fingerprint(
        context.vault,
        endpoint.provider_kind(),
        existing.credential_kind(),
        &secret,
    )?;
    let rotated = match expected_kind {
        CredentialKind::ApiKey => existing.rotated(fingerprint)?,
        CredentialKind::OAuth2 => existing.refreshed(fingerprint)?,
    };
    let envelope = seal(context.vault, endpoint.provider_kind(), &rotated, &secret)?;
    let configuration = replace(
        context.current,
        context.endpoints,
        context.proxies,
        rotated.clone(),
    )?;
    Ok(PreparedProviderCredentialMutation::new(
        configuration,
        ProviderCredentialDatabaseChange::RotateSecret {
            credential: rotated,
            envelope,
        },
    ))
}

fn seal(
    vault: &SecretVault,
    provider_kind: any2api_domain::ProviderKind,
    credential: &ProviderCredential,
    api_key: &SecretBytes,
) -> Result<SecretEnvelope, StorageError> {
    vault
        .seal(
            SecretContext::provider_credential(
                credential.id(),
                provider_kind,
                credential.credential_kind(),
                credential.secret_schema_version(),
                credential.secret_version(),
            ),
            api_key,
        )
        .map_err(StorageError::from)
}

fn build_secret_fingerprint(
    vault: &SecretVault,
    provider_kind: any2api_domain::ProviderKind,
    credential_kind: any2api_domain::CredentialKind,
    secret: &SecretBytes,
) -> Result<any2api_domain::CredentialSecretFingerprint, StorageError> {
    match credential_kind {
        any2api_domain::CredentialKind::ApiKey => {
            build_fingerprint(vault, provider_kind, credential_kind, secret).map_err(Into::into)
        }
        any2api_domain::CredentialKind::OAuth2 => {
            build_oauth2_fingerprint(vault, provider_kind, secret).map_err(Into::into)
        }
    }
}

fn require_kind(actual: CredentialKind, expected: CredentialKind) -> Result<(), StorageError> {
    if actual == expected {
        Ok(())
    } else {
        Err(StorageError::ProviderCredentialKindMismatch)
    }
}
