use any2api_domain::{ProviderKind, UpstreamModelName};
use async_trait::async_trait;
use sqlx::FromRow;

use crate::{error::StorageError, sqlite::SqliteStore};

pub const MAX_OAUTH_MODEL_CATALOG_MODELS: usize = 4_096;
pub const MAX_OAUTH_MODEL_CATALOG_SNAPSHOT_BYTES: usize = 128 * 1024;
const MAX_DIRECTORY_SCOPE_CHARS: usize = 96;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredOAuthModelCatalogSnapshot {
    pub provider_kind: ProviderKind,
    pub directory_scope: String,
    pub fetched_at: i64,
    pub models: Vec<String>,
}

#[async_trait]
pub trait OAuthModelCatalogSnapshotRepository: Send + Sync {
    async fn load_oauth_model_catalog_snapshots(
        &self,
    ) -> Result<Vec<StoredOAuthModelCatalogSnapshot>, StorageError>;

    async fn upsert_oauth_model_catalog_snapshot(
        &self,
        snapshot: &StoredOAuthModelCatalogSnapshot,
    ) -> Result<(), StorageError>;
}

#[async_trait]
impl OAuthModelCatalogSnapshotRepository for SqliteStore {
    async fn load_oauth_model_catalog_snapshots(
        &self,
    ) -> Result<Vec<StoredOAuthModelCatalogSnapshot>, StorageError> {
        sqlx::query_as::<_, SnapshotRow>(
            "SELECT provider_kind, directory_scope, fetched_at, models_json \
             FROM oauth_model_catalog_snapshots ORDER BY provider_kind, directory_scope",
        )
        .fetch_all(self.pool())
        .await?
        .into_iter()
        .map(parse_row)
        .collect()
    }

    async fn upsert_oauth_model_catalog_snapshot(
        &self,
        snapshot: &StoredOAuthModelCatalogSnapshot,
    ) -> Result<(), StorageError> {
        validate(snapshot)?;
        let models_json = serde_json::to_vec(&snapshot.models)
            .map_err(|_| StorageError::CorruptOAuthModelCatalogSnapshot)?;
        if !(2..=MAX_OAUTH_MODEL_CATALOG_SNAPSHOT_BYTES).contains(&models_json.len()) {
            return Err(StorageError::CorruptOAuthModelCatalogSnapshot);
        }
        sqlx::query(
            "INSERT INTO oauth_model_catalog_snapshots \
             (provider_kind, directory_scope, fetched_at, models_json) VALUES (?, ?, ?, ?) \
             ON CONFLICT(provider_kind, directory_scope) DO UPDATE SET \
                 fetched_at = excluded.fetched_at, \
                 models_json = excluded.models_json, \
                 updated_at = CURRENT_TIMESTAMP",
        )
        .bind(snapshot.provider_kind.as_str())
        .bind(&snapshot.directory_scope)
        .bind(snapshot.fetched_at)
        .bind(models_json)
        .execute(self.write_pool())
        .await?;
        Ok(())
    }
}

#[derive(FromRow)]
struct SnapshotRow {
    provider_kind: String,
    directory_scope: String,
    fetched_at: i64,
    models_json: Vec<u8>,
}

fn parse_row(row: SnapshotRow) -> Result<StoredOAuthModelCatalogSnapshot, StorageError> {
    let provider_kind = row
        .provider_kind
        .parse()
        .map_err(|_| StorageError::CorruptOAuthModelCatalogSnapshot)?;
    let models = serde_json::from_slice::<Vec<String>>(&row.models_json)
        .map_err(|_| StorageError::CorruptOAuthModelCatalogSnapshot)?;
    let snapshot = StoredOAuthModelCatalogSnapshot {
        provider_kind,
        directory_scope: row.directory_scope,
        fetched_at: row.fetched_at,
        models,
    };
    validate(&snapshot)?;
    Ok(snapshot)
}

fn validate(snapshot: &StoredOAuthModelCatalogSnapshot) -> Result<(), StorageError> {
    if !valid_scope(&snapshot.directory_scope)
        || snapshot.fetched_at < 0
        || snapshot.models.len() > MAX_OAUTH_MODEL_CATALOG_MODELS
    {
        return Err(StorageError::CorruptOAuthModelCatalogSnapshot);
    }
    let mut previous = None;
    for model in &snapshot.models {
        let model = UpstreamModelName::new(model.clone())
            .map_err(|_| StorageError::CorruptOAuthModelCatalogSnapshot)?;
        if previous
            .as_deref()
            .is_some_and(|previous| previous >= model.as_str())
        {
            return Err(StorageError::CorruptOAuthModelCatalogSnapshot);
        }
        previous = Some(model.as_str().to_owned());
    }
    Ok(())
}

fn valid_scope(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_DIRECTORY_SCOPE_CHARS
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}
