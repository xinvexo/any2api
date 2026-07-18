use any2api_domain::{
    CredentialId, ProviderCredential, ProviderCredentialConfiguration, ProviderCredentialDraft,
    ProviderEndpointConfiguration, ProviderEndpointId, ProxyConfiguration,
};

use crate::{
    error::StorageError,
    provider_api_key::build_fingerprint,
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
    api_key: SecretBytes,
) -> Result<PreparedProviderCredentialMutation, StorageError> {
    let endpoint = context
        .endpoints
        .get(endpoint_id)
        .ok_or(StorageError::ProviderEndpointNotFound(endpoint_id))?;
    let fingerprint = build_fingerprint(
        context.vault,
        endpoint.provider_kind(),
        draft.credential_kind(),
        &api_key,
    )?;
    let credential = ProviderCredential::create(id, endpoint_id, draft, fingerprint);
    let envelope = seal(
        context.vault,
        endpoint.provider_kind(),
        &credential,
        &api_key,
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
    expected_config_version: u64,
    expected_secret_version: u64,
    api_key: SecretBytes,
) -> Result<PreparedProviderCredentialMutation, StorageError> {
    let existing = context
        .current
        .get(id)
        .ok_or(StorageError::ProviderCredentialNotFound(id))?;
    require_config_version(existing, expected_config_version)?;
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
    let fingerprint = build_fingerprint(
        context.vault,
        endpoint.provider_kind(),
        existing.credential_kind(),
        &api_key,
    )?;
    let rotated = existing.rotated(fingerprint)?;
    let envelope = seal(context.vault, endpoint.provider_kind(), &rotated, &api_key)?;
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
