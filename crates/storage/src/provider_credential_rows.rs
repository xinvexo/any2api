use std::str::FromStr;

use any2api_domain::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, MaxConcurrency, ProviderCredential,
    ProviderCredentialConfiguration, ProviderCredentialDraft, ProviderEndpointConfiguration,
    ProviderEndpointId, ProxyConfiguration, ProxyProfileId,
};
use sqlx::{FromRow, SqliteConnection};
use subtle::ConstantTimeEq;

use crate::{
    error::StorageError,
    provider_api_key::build_fingerprint,
    vault::{SecretContext, SecretEnvelope, SecretVault},
};

#[derive(Debug, FromRow)]
struct ProviderCredentialRow {
    id: String,
    provider_endpoint_id: String,
    label: String,
    credential_kind: String,
    secret_schema_version: i64,
    secret_version: i64,
    credential_generation: i64,
    config_version: i64,
    envelope_version: i64,
    key_id: String,
    algorithm: String,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    aad_version: i64,
    fingerprint_version: i64,
    secret_fingerprint: Vec<u8>,
    secret_tail: Option<String>,
    proxy_profile_id: String,
    max_concurrency: i64,
    enabled: i64,
}

pub(crate) async fn load_provider_credentials_from(
    connection: &mut SqliteConnection,
    vault: &SecretVault,
    endpoints: &ProviderEndpointConfiguration,
    proxies: &ProxyConfiguration,
) -> Result<ProviderCredentialConfiguration, StorageError> {
    let rows = sqlx::query_as::<_, ProviderCredentialRow>(
        "SELECT id, provider_endpoint_id, label, credential_kind, secret_schema_version, \
         secret_version, credential_generation, config_version, envelope_version, key_id, \
         algorithm, nonce, ciphertext, aad_version, fingerprint_version, secret_fingerprint, \
         secret_tail, proxy_profile_id, max_concurrency, enabled \
         FROM provider_credentials ORDER BY provider_endpoint_id ASC, label ASC",
    )
    .fetch_all(connection)
    .await?;
    let credentials = rows
        .into_iter()
        .map(|row| parse_row(row, vault, endpoints))
        .collect::<Result<Vec<_>, _>>()?;
    ProviderCredentialConfiguration::new(credentials, endpoints, proxies)
        .map_err(|_| StorageError::CorruptConfiguration)
}

fn parse_row(
    row: ProviderCredentialRow,
    vault: &SecretVault,
    endpoints: &ProviderEndpointConfiguration,
) -> Result<ProviderCredential, StorageError> {
    let ProviderCredentialRow {
        id,
        provider_endpoint_id,
        label,
        credential_kind,
        secret_schema_version,
        secret_version,
        credential_generation,
        config_version,
        envelope_version,
        key_id,
        algorithm,
        nonce,
        ciphertext,
        aad_version,
        fingerprint_version,
        secret_fingerprint,
        secret_tail,
        proxy_profile_id,
        max_concurrency,
        enabled,
    } = row;
    let id = CredentialId::from_str(&id).map_err(|_| StorageError::CorruptConfiguration)?;
    let endpoint_id = ProviderEndpointId::from_str(&provider_endpoint_id)
        .map_err(|_| StorageError::CorruptConfiguration)?;
    let endpoint = endpoints
        .get(endpoint_id)
        .ok_or(StorageError::CorruptConfiguration)?;
    let credential_kind = parse_credential_kind(&credential_kind)?;
    let proxy_profile_id = ProxyProfileId::from_str(&proxy_profile_id)
        .map_err(|_| StorageError::CorruptConfiguration)?;
    let max_concurrency = u32::try_from(max_concurrency)
        .ok()
        .and_then(|value| MaxConcurrency::new(value).ok())
        .ok_or(StorageError::CorruptConfiguration)?;
    let secret_schema_version =
        u32::try_from(secret_schema_version).map_err(|_| StorageError::CorruptConfiguration)?;
    let secret_version = parse_version(secret_version)?;
    let credential_generation = parse_version(credential_generation)?;
    let config_version = parse_version(config_version)?;
    let fingerprint_version =
        u16::try_from(fingerprint_version).map_err(|_| StorageError::CorruptConfiguration)?;
    let fingerprint_digest: [u8; 32] = secret_fingerprint
        .try_into()
        .map_err(|_| StorageError::CorruptConfiguration)?;
    let fingerprint =
        CredentialSecretFingerprint::restore(fingerprint_version, fingerprint_digest, secret_tail)
            .map_err(|_| StorageError::CorruptConfiguration)?;
    let draft = ProviderCredentialDraft::new(
        label,
        credential_kind,
        proxy_profile_id,
        max_concurrency,
        enabled == 1,
    )
    .map_err(|_| StorageError::CorruptConfiguration)?;
    let credential = ProviderCredential::restore(
        id,
        endpoint_id,
        draft,
        fingerprint,
        secret_schema_version,
        secret_version,
        credential_generation,
        config_version,
    )
    .map_err(|_| StorageError::CorruptConfiguration)?;
    let envelope = SecretEnvelope::restore(
        u16::try_from(envelope_version).map_err(|_| StorageError::CorruptConfiguration)?,
        key_id,
        &algorithm,
        &nonce,
        ciphertext,
        u16::try_from(aad_version).map_err(|_| StorageError::CorruptConfiguration)?,
    )?;
    verify_secret(envelope, vault, endpoint.provider_kind(), &credential)?;
    Ok(credential)
}

fn verify_secret(
    envelope: SecretEnvelope,
    vault: &SecretVault,
    provider_kind: any2api_domain::ProviderKind,
    credential: &ProviderCredential,
) -> Result<(), StorageError> {
    let secret = vault.open(
        SecretContext::provider_credential(
            credential.id(),
            provider_kind,
            credential.credential_kind(),
            credential.secret_schema_version(),
            credential.secret_version(),
        ),
        &envelope,
    )?;
    let computed = build_fingerprint(vault, provider_kind, credential.credential_kind(), &secret)?;
    if !bool::from(computed.digest().ct_eq(credential.fingerprint().digest()))
        || computed.tail() != credential.fingerprint().tail()
    {
        return Err(StorageError::CorruptConfiguration);
    }
    Ok(())
}

fn parse_version(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptConfiguration)
}

fn parse_credential_kind(value: &str) -> Result<CredentialKind, StorageError> {
    match value {
        "api_key" => Ok(CredentialKind::ApiKey),
        _ => Err(StorageError::CorruptConfiguration),
    }
}
