use any2api_domain::ConfigRevision;
use sqlx::SqliteConnection;

use crate::error::StorageError;

pub(crate) async fn bump_revision(
    connection: &mut SqliteConnection,
    expected: ConfigRevision,
) -> Result<ConfigRevision, StorageError> {
    let next: Option<i64> = sqlx::query_scalar(
        "UPDATE config_state \
         SET revision = revision + 1, updated_at = CURRENT_TIMESTAMP \
         WHERE singleton_id = 1 AND revision = ? AND revision < ? \
         RETURNING revision",
    )
    .bind(i64::try_from(expected.get()).map_err(|_| StorageError::RevisionOverflow)?)
    .bind(i64::MAX)
    .fetch_optional(connection)
    .await?;
    let next = next.ok_or(StorageError::RevisionOverflow)?;
    let next = u64::try_from(next).map_err(|_| StorageError::InvalidRevision(next))?;
    ConfigRevision::new(next).map_err(|_| StorageError::InvalidRevision(next as i64))
}
