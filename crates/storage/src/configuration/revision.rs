use any2api_domain::ConfigRevision;
use sqlx::SqliteConnection;

use crate::error::{ConfigurationWriteComponent, StorageError};

const SQLITE_MAX_REVISION: u64 = i64::MAX as u64;

pub(crate) async fn load_revision_from(
    connection: &mut SqliteConnection,
) -> Result<ConfigRevision, StorageError> {
    let revision: i64 =
        sqlx::query_scalar("SELECT revision FROM config_state WHERE singleton_id = 1")
            .fetch_one(connection)
            .await?;
    parse_revision(revision)
}

pub(crate) async fn bump_revision(
    connection: &mut SqliteConnection,
    expected: ConfigRevision,
) -> Result<ConfigRevision, StorageError> {
    let Ok(expected_sql) = i64::try_from(expected.get()) else {
        return classify_failed_update(connection, expected).await;
    };
    let next: Option<i64> = sqlx::query_scalar(
        "UPDATE config_state \
         SET revision = revision + 1, updated_at = CURRENT_TIMESTAMP \
         WHERE singleton_id = 1 AND revision = ? AND revision < ? \
         RETURNING revision",
    )
    .bind(expected_sql)
    .bind(i64::MAX)
    .fetch_optional(&mut *connection)
    .await?;
    match next {
        Some(next) => parse_revision(next),
        None => classify_failed_update(connection, expected).await,
    }
}

async fn classify_failed_update(
    connection: &mut SqliteConnection,
    expected: ConfigRevision,
) -> Result<ConfigRevision, StorageError> {
    let actual = load_revision_from(connection).await?;
    if actual != expected {
        return Err(StorageError::RevisionConflict { expected, actual });
    }
    if actual.get() == SQLITE_MAX_REVISION {
        return Err(StorageError::RevisionOverflow);
    }
    Err(StorageError::ConfigurationWriteMismatch(
        ConfigurationWriteComponent::Revision,
    ))
}

pub(super) fn parse_revision(value: i64) -> Result<ConfigRevision, StorageError> {
    let revision = u64::try_from(value).map_err(|_| StorageError::InvalidRevision(value))?;
    ConfigRevision::new(revision).map_err(|_| StorageError::InvalidRevision(value))
}

#[cfg(test)]
mod tests {
    use any2api_domain::ConfigRevision;
    use sqlx::{Connection, SqliteConnection};

    use crate::error::{ConfigurationWriteComponent, StorageError};

    use super::{SQLITE_MAX_REVISION, bump_revision};

    #[tokio::test]
    async fn matching_revision_increments_once() {
        let mut connection = connection_with_revision(1).await;

        let next = bump_revision(&mut connection, ConfigRevision::INITIAL)
            .await
            .expect("revision increment");

        assert_eq!(next.get(), 2);
        assert_eq!(stored_revision(&mut connection).await, 2);
    }

    #[tokio::test]
    async fn stale_and_unrepresentable_expected_revisions_are_conflicts() {
        let mut connection = connection_with_revision(2).await;
        let stale = bump_revision(&mut connection, ConfigRevision::INITIAL)
            .await
            .expect_err("stale revision");
        assert!(matches!(
            stale,
            StorageError::RevisionConflict { expected, actual }
                if expected == ConfigRevision::INITIAL && actual.get() == 2
        ));

        let future = ConfigRevision::new(SQLITE_MAX_REVISION + 1).expect("future revision");
        let conflict = bump_revision(&mut connection, future)
            .await
            .expect_err("unrepresentable expected revision");
        assert!(matches!(
            conflict,
            StorageError::RevisionConflict { expected, actual }
                if expected == future && actual.get() == 2
        ));
        assert_eq!(stored_revision(&mut connection).await, 2);
    }

    #[tokio::test]
    async fn sqlite_integer_limit_is_the_only_overflow_result() {
        let mut connection = connection_with_revision(i64::MAX).await;
        let expected = ConfigRevision::new(SQLITE_MAX_REVISION).expect("maximum revision");

        let error = bump_revision(&mut connection, expected)
            .await
            .expect_err("revision overflow");

        assert!(matches!(error, StorageError::RevisionOverflow));
        assert_eq!(stored_revision(&mut connection).await, i64::MAX);
    }

    #[tokio::test]
    async fn ignored_update_is_a_typed_write_mismatch() {
        let mut connection = connection_with_revision(1).await;
        sqlx::query(
            "CREATE TRIGGER ignore_revision_update \
             BEFORE UPDATE OF revision ON config_state BEGIN SELECT RAISE(IGNORE); END",
        )
        .execute(&mut connection)
        .await
        .expect("ignore-update trigger");

        let error = bump_revision(&mut connection, ConfigRevision::INITIAL)
            .await
            .expect_err("ignored revision update");

        assert!(matches!(
            error,
            StorageError::ConfigurationWriteMismatch(ConfigurationWriteComponent::Revision)
        ));
        assert_eq!(stored_revision(&mut connection).await, 1);
    }

    async fn connection_with_revision(revision: i64) -> SqliteConnection {
        let mut connection = SqliteConnection::connect(":memory:")
            .await
            .expect("SQLite connection");
        sqlx::query(
            "CREATE TABLE config_state (\
                 singleton_id INTEGER PRIMARY KEY, revision INTEGER NOT NULL, updated_at TEXT\
             )",
        )
        .execute(&mut connection)
        .await
        .expect("config state table");
        sqlx::query("INSERT INTO config_state (singleton_id, revision) VALUES (1, ?)")
            .bind(revision)
            .execute(&mut connection)
            .await
            .expect("config revision");
        connection
    }

    async fn stored_revision(connection: &mut SqliteConnection) -> i64 {
        sqlx::query_scalar("SELECT revision FROM config_state WHERE singleton_id = 1")
            .fetch_one(connection)
            .await
            .expect("stored revision")
    }
}
