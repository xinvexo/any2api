use any2api_domain::{
    CredentialId, ProviderCredential, ProviderCredentialConfiguration, ProviderCredentialDraft,
    ProviderEndpointConfiguration, ProviderEndpointId, ProxyConfiguration,
};

use crate::{error::StorageError, secret::SecretBytes};

use super::{
    api_key::build_fingerprint,
    mutation::{
        PreparedProviderCredentialMutation, ProviderCredentialDatabaseChange, map_validation,
        replace, require_config_version,
    },
};

pub(crate) struct CredentialSecretMutationContext<'a> {
    pub(crate) current: &'a ProviderCredentialConfiguration,
    pub(crate) endpoints: &'a ProviderEndpointConfiguration,
    pub(crate) proxies: &'a ProxyConfiguration,
}

impl<'a> CredentialSecretMutationContext<'a> {
    pub(crate) const fn new(
        current: &'a ProviderCredentialConfiguration,
        endpoints: &'a ProviderEndpointConfiguration,
        proxies: &'a ProxyConfiguration,
    ) -> Self {
        Self {
            current,
            endpoints,
            proxies,
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
    let fingerprint =
        build_fingerprint(endpoint.provider_kind(), draft.credential_kind(), &api_key)?;
    let credential = ProviderCredential::create(id, endpoint_id, draft, fingerprint);
    let mut credentials = context.current.credentials().to_vec();
    credentials.push(credential.clone());
    let configuration =
        ProviderCredentialConfiguration::new(credentials, context.endpoints, context.proxies)
            .map_err(map_validation)?;
    Ok(PreparedProviderCredentialMutation::new(
        configuration,
        ProviderCredentialDatabaseChange::Create {
            credential,
            api_key,
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
        endpoint.provider_kind(),
        existing.credential_kind(),
        &api_key,
    )?;
    let rotated = existing.rotated(fingerprint)?;
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
            api_key,
        },
    ))
}
