use std::{collections::HashMap, str::FromStr};

use any2api_domain::{
    MaxConcurrency, OAuthAccount, OAuthAccountConfiguration, OAuthAccountDraft, OAuthAccountId,
    ProviderKind, ProxyConfiguration, ProxyProfileId,
};
use sqlx::{FromRow, SqliteConnection};

use crate::{
    error::StorageError,
    oauth_account_document::OAuthAccountDocument,
    oauth_account_material::{StoredOAuthAccountMaterial, StoredOAuthAccountMaterials},
};

#[derive(Debug, FromRow)]
struct OAuthAccountRow {
    id: String,
    provider_kind: String,
    label: String,
    oauth_json: Vec<u8>,
    token_version: i64,
    account_generation: i64,
    config_version: i64,
    proxy_profile_id: String,
    max_concurrency: i64,
    enabled: i64,
    safe_account_email: Option<String>,
    expires_at: Option<i64>,
}

#[derive(Debug, FromRow)]
struct OAuthAccountModelRow {
    oauth_account_id: String,
    upstream_model: String,
}

pub(crate) async fn load_oauth_accounts_from(
    connection: &mut SqliteConnection,
    proxies: &ProxyConfiguration,
) -> Result<(OAuthAccountConfiguration, StoredOAuthAccountMaterials), StorageError> {
    let rows = sqlx::query_as::<_, OAuthAccountRow>(concat!(
        "SELECT id, provider_kind, label, oauth_json, token_version, account_generation, ",
        "config_version, proxy_profile_id, max_concurrency, enabled, safe_account_email, ",
        "expires_at FROM oauth_accounts ORDER BY provider_kind, label"
    ))
    .fetch_all(&mut *connection)
    .await?;
    let model_rows = sqlx::query_as::<_, OAuthAccountModelRow>(concat!(
        "SELECT oauth_account_id, upstream_model FROM oauth_account_models ",
        "ORDER BY oauth_account_id, upstream_model"
    ))
    .fetch_all(&mut *connection)
    .await?;
    let mut models = group_models(model_rows);
    let mut accounts = Vec::with_capacity(rows.len());
    let mut materials = Vec::with_capacity(rows.len());
    for row in rows {
        let selected_models = models.remove(&row.id).unwrap_or_default();
        let (account, material) = parse_row(row, selected_models)?;
        accounts.push(account);
        materials.push(material);
    }
    if !models.is_empty() {
        return Err(StorageError::CorruptConfiguration);
    }
    let configuration = OAuthAccountConfiguration::new(accounts, proxies)
        .map_err(|_| StorageError::CorruptConfiguration)?;
    Ok((configuration, StoredOAuthAccountMaterials::new(materials)))
}

fn parse_row(
    row: OAuthAccountRow,
    models: Vec<String>,
) -> Result<(OAuthAccount, StoredOAuthAccountMaterial), StorageError> {
    let id = OAuthAccountId::from_str(&row.id).map_err(|_| StorageError::CorruptConfiguration)?;
    let provider_kind = parse_provider_kind(&row.provider_kind)?;
    let proxy_profile_id = ProxyProfileId::from_str(&row.proxy_profile_id)
        .map_err(|_| StorageError::CorruptConfiguration)?;
    let max_concurrency = u32::try_from(row.max_concurrency)
        .ok()
        .and_then(|value| MaxConcurrency::new(value).ok())
        .ok_or(StorageError::CorruptConfiguration)?;
    let enabled = parse_bool(row.enabled)?;
    let token_version = parse_version(row.token_version)?;
    let account_generation = parse_version(row.account_generation)?;
    let config_version = parse_version(row.config_version)?;
    let draft = OAuthAccountDraft::new(row.label, max_concurrency, enabled)
        .map_err(|_| StorageError::CorruptConfiguration)?;
    let account = OAuthAccount::restore(
        id,
        provider_kind,
        draft,
        proxy_profile_id,
        row.safe_account_email,
        row.expires_at,
        token_version,
        account_generation,
        config_version,
        models,
    )
    .map_err(|_| StorageError::CorruptConfiguration)?;
    let document = OAuthAccountDocument::new(provider_kind, row.oauth_json.into())
        .map_err(|_| StorageError::CorruptConfiguration)?;
    let material = StoredOAuthAccountMaterial::new(
        id,
        provider_kind,
        token_version,
        account_generation,
        document,
    );
    Ok((account, material))
}

fn group_models(rows: Vec<OAuthAccountModelRow>) -> HashMap<String, Vec<String>> {
    let mut models = HashMap::<String, Vec<String>>::new();
    for row in rows {
        models
            .entry(row.oauth_account_id)
            .or_default()
            .push(row.upstream_model);
    }
    models
}

fn parse_provider_kind(value: &str) -> Result<ProviderKind, StorageError> {
    match value {
        "codex" => Ok(ProviderKind::Codex),
        "claude" => Ok(ProviderKind::Claude),
        _ => Err(StorageError::CorruptConfiguration),
    }
}

fn parse_bool(value: i64) -> Result<bool, StorageError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(StorageError::CorruptConfiguration),
    }
}

fn parse_version(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptConfiguration)
}
