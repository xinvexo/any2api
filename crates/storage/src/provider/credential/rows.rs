use std::{collections::HashMap, str::FromStr};

use any2api_domain::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, ProviderCredential,
    ProviderCredentialConfiguration, ProviderCredentialDraft, ProviderCredentialModel,
    ProviderEndpointConfiguration, ProviderEndpointId, ProxyConfiguration, ProxyProfileId,
    RequestsPerMinute,
};
use sqlx::{FromRow, SqliteConnection};
use subtle::ConstantTimeEq;

use crate::{error::StorageError, secret::SecretBytes};

use super::{
    api_key::build_fingerprint,
    secret_material::{StoredProviderCredentialSecret, StoredProviderCredentialSecrets},
};

#[derive(FromRow)]
struct ProviderCredentialRow {
    id: String,
    provider_endpoint_id: String,
    label: String,
    credential_kind: String,
    secret_version: i64,
    credential_generation: i64,
    config_version: i64,
    api_key: Vec<u8>,
    fingerprint_version: i64,
    secret_fingerprint: Vec<u8>,
    secret_tail: Option<String>,
    proxy_profile_id: String,
    requests_per_minute: Option<i64>,
    enabled: i64,
}

#[derive(Debug, FromRow)]
struct ProviderCredentialModelRow {
    credential_id: String,
    upstream_model: String,
    public_model: Option<String>,
}

pub(crate) async fn load_provider_credentials_from(
    connection: &mut SqliteConnection,
    endpoints: &ProviderEndpointConfiguration,
    proxies: &ProxyConfiguration,
) -> Result<
    (
        ProviderCredentialConfiguration,
        StoredProviderCredentialSecrets,
    ),
    StorageError,
> {
    let rows = sqlx::query_as::<_, ProviderCredentialRow>(
        "SELECT id, provider_endpoint_id, label, credential_kind, secret_version, \
         credential_generation, config_version, api_key, \
         fingerprint_version, secret_fingerprint, \
         secret_tail, proxy_profile_id, requests_per_minute, enabled \
         FROM provider_credentials ORDER BY provider_endpoint_id ASC, label ASC",
    )
    .fetch_all(&mut *connection)
    .await?;
    let model_rows = sqlx::query_as::<_, ProviderCredentialModelRow>(
        "SELECT credential_id, upstream_model, public_model FROM provider_credential_models \
         ORDER BY credential_id, upstream_model",
    )
    .fetch_all(&mut *connection)
    .await?;
    let mut models = group_models(model_rows)?;
    let mut credentials = Vec::with_capacity(rows.len());
    let mut secrets = Vec::with_capacity(rows.len());
    for row in rows {
        let selected_models = models.remove(&row.id).unwrap_or_default();
        let (credential, secret) = parse_row(row, selected_models, endpoints)?;
        credentials.push(credential);
        secrets.push(secret);
    }
    if !models.is_empty() {
        return Err(StorageError::CorruptConfiguration);
    }
    let configuration = ProviderCredentialConfiguration::new(credentials, endpoints, proxies)
        .map_err(|_| StorageError::CorruptConfiguration)?;
    Ok((configuration, StoredProviderCredentialSecrets::new(secrets)))
}

fn parse_row(
    row: ProviderCredentialRow,
    models: Vec<ProviderCredentialModel>,
    endpoints: &ProviderEndpointConfiguration,
) -> Result<(ProviderCredential, StoredProviderCredentialSecret), StorageError> {
    let ProviderCredentialRow {
        id,
        provider_endpoint_id,
        label,
        credential_kind,
        secret_version,
        credential_generation,
        config_version,
        api_key,
        fingerprint_version,
        secret_fingerprint,
        secret_tail,
        proxy_profile_id,
        requests_per_minute,
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
    let requests_per_minute = requests_per_minute
        .map(|value| {
            u32::try_from(value)
                .ok()
                .and_then(|value| RequestsPerMinute::new(value).ok())
                .ok_or(StorageError::CorruptConfiguration)
        })
        .transpose()?;
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
        requests_per_minute,
        enabled == 1,
    )
    .map_err(|_| StorageError::CorruptConfiguration)?;
    let credential = ProviderCredential::restore(
        id,
        endpoint_id,
        draft,
        fingerprint,
        secret_version,
        credential_generation,
        config_version,
        models,
    )
    .map_err(|_| StorageError::CorruptConfiguration)?;
    let secret: SecretBytes = api_key.into();
    verify_secret(&secret, endpoint.provider_kind(), &credential)?;
    let material = StoredProviderCredentialSecret::new(
        credential.id(),
        credential.credential_kind(),
        credential.secret_version(),
        credential.credential_generation(),
        secret,
    );
    Ok((credential, material))
}

fn group_models(
    rows: Vec<ProviderCredentialModelRow>,
) -> Result<HashMap<String, Vec<ProviderCredentialModel>>, StorageError> {
    let mut models = HashMap::<String, Vec<ProviderCredentialModel>>::new();
    for row in rows {
        let model = ProviderCredentialModel::new(row.upstream_model, row.public_model)
            .map_err(|_| StorageError::CorruptConfiguration)?;
        models.entry(row.credential_id).or_default().push(model);
    }
    Ok(models)
}

fn verify_secret(
    secret: &SecretBytes,
    provider_kind: any2api_domain::ProviderKind,
    credential: &ProviderCredential,
) -> Result<(), StorageError> {
    let computed = build_fingerprint(provider_kind, credential.credential_kind(), secret)?;
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
