use any2api_domain::ProviderKind;
use async_trait::async_trait;
use semver::Version;
use sqlx::FromRow;

use crate::{error::StorageError, sqlite::SqliteStore};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredOfficialClientVersion {
    pub provider_kind: ProviderKind,
    pub version: String,
    pub fetched_at: i64,
}

#[async_trait]
pub trait OfficialClientVersionRepository: Send + Sync {
    async fn load_official_client_versions(
        &self,
    ) -> Result<Vec<StoredOfficialClientVersion>, StorageError>;

    async fn upsert_official_client_version(
        &self,
        version: &StoredOfficialClientVersion,
    ) -> Result<(), StorageError>;
}

#[async_trait]
impl OfficialClientVersionRepository for SqliteStore {
    async fn load_official_client_versions(
        &self,
    ) -> Result<Vec<StoredOfficialClientVersion>, StorageError> {
        sqlx::query_as::<_, VersionRow>(
            "SELECT provider_kind, version, fetched_at \
             FROM official_client_versions ORDER BY provider_kind",
        )
        .fetch_all(self.pool())
        .await?
        .into_iter()
        .map(parse_row)
        .collect()
    }

    async fn upsert_official_client_version(
        &self,
        version: &StoredOfficialClientVersion,
    ) -> Result<(), StorageError> {
        validate(version)?;
        sqlx::query(
            "INSERT INTO official_client_versions \
             (provider_kind, version, fetched_at) VALUES (?, ?, ?) \
             ON CONFLICT(provider_kind) DO UPDATE SET \
                 version = excluded.version, \
                 fetched_at = excluded.fetched_at, \
                 updated_at = CURRENT_TIMESTAMP",
        )
        .bind(version.provider_kind.as_str())
        .bind(&version.version)
        .bind(version.fetched_at)
        .execute(self.write_pool())
        .await?;
        Ok(())
    }
}

#[derive(FromRow)]
struct VersionRow {
    provider_kind: String,
    version: String,
    fetched_at: i64,
}

fn parse_row(row: VersionRow) -> Result<StoredOfficialClientVersion, StorageError> {
    let version = StoredOfficialClientVersion {
        provider_kind: row
            .provider_kind
            .parse()
            .map_err(|_| StorageError::CorruptOfficialClientVersion)?,
        version: row.version,
        fetched_at: row.fetched_at,
    };
    validate(&version)?;
    Ok(version)
}

fn validate(version: &StoredOfficialClientVersion) -> Result<(), StorageError> {
    let parsed =
        Version::parse(&version.version).map_err(|_| StorageError::CorruptOfficialClientVersion)?;
    if version.fetched_at < 0 || !parsed.pre.is_empty() || parsed.to_string() != version.version {
        return Err(StorageError::CorruptOfficialClientVersion);
    }
    Ok(())
}
