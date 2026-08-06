use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use sqlx::{
    Connection, Sqlite, SqliteConnection, SqlitePool, Transaction,
    sqlite::{
        SqliteAutoVacuum, SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions,
        SqliteSynchronous,
    },
};

use crate::{
    error::StorageError,
    local_data::{ensure_private_directory, ensure_private_file, protect_private_file},
    migration,
};

const READ_POOL_CONNECTIONS: u32 = 8;
const READ_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// Truncate the WAL after this many write transactions so long uptimes with
/// steady telemetry flushes cannot grow the log without bound.
const WAL_CHECKPOINT_WRITE_INTERVAL: u64 = 256;

#[derive(Clone, Debug)]
pub struct SqliteStore {
    read_pool: SqlitePool,
    write_pool: SqlitePool,
    write_transactions: Arc<AtomicU64>,
}

impl SqliteStore {
    pub async fn connect(path: &Path) -> Result<Self, StorageError> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            ensure_private_directory(parent).map_err(|source| StorageError::ProtectLocalData {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        ensure_private_file(path).map_err(|source| StorageError::ProtectLocalData {
            path: path.to_path_buf(),
            source,
        })?;

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(false)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(BUSY_TIMEOUT);
        let migration_options = options.clone().auto_vacuum(SqliteAutoVacuum::Incremental);
        let mut migration_connection = SqliteConnection::connect_with(&migration_options).await?;
        migration::run(&mut migration_connection).await?;
        migration_connection.close().await?;
        protect_sqlite_files(path)?;

        let options = options.foreign_keys(true);
        // All writes funnel through a single connection so write transactions
        // serialize on pool acquisition instead of contending for the SQLite
        // write lock, and heavy read-side queries can no longer starve them.
        let write_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await?;
        let read_pool = SqlitePoolOptions::new()
            .max_connections(READ_POOL_CONNECTIONS)
            .idle_timeout(READ_POOL_IDLE_TIMEOUT)
            .connect_with(options)
            .await?;
        protect_sqlite_files(path)?;
        Ok(Self {
            read_pool,
            write_pool,
            write_transactions: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Pool for read-only statements and transactions.
    #[must_use]
    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.read_pool
    }

    /// Pool holding the single connection reserved for writes.
    #[must_use]
    pub(crate) fn write_pool(&self) -> &SqlitePool {
        &self.write_pool
    }

    pub(crate) async fn begin_write(&self) -> Result<Transaction<'static, Sqlite>, StorageError> {
        self.checkpoint_wal_if_due().await;
        Ok(self.write_pool.begin_with("BEGIN IMMEDIATE").await?)
    }

    async fn checkpoint_wal_if_due(&self) {
        let writes = self.write_transactions.fetch_add(1, Ordering::Relaxed) + 1;
        if !writes.is_multiple_of(WAL_CHECKPOINT_WRITE_INTERVAL) {
            return;
        }
        // Best effort: a checkpoint blocked by concurrent readers simply runs
        // again after the next interval.
        if let Err(error) = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .fetch_all(&self.write_pool)
            .await
        {
            tracing::debug!(%error, "WAL checkpoint failed; will retry on a later write");
        }
    }

    pub async fn close(&self) {
        self.write_pool.close().await;
        self.read_pool.close().await;
    }
}

fn protect_sqlite_files(path: &Path) -> Result<(), StorageError> {
    for path in [
        path.to_path_buf(),
        sidecar_path(path, "-wal"),
        sidecar_path(path, "-shm"),
    ] {
        protect_private_file(&path)
            .map_err(|source| StorageError::ProtectLocalData { path, source })?;
    }
    Ok(())
}

fn sidecar_path(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(suffix);
    value.into()
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    #[cfg(unix)]
    use super::sidecar_path;
    use super::{READ_POOL_IDLE_TIMEOUT, SqliteStore};

    #[tokio::test]
    async fn uses_normal_synchronous_mode_and_private_sidecars() {
        let directory = tempdir().expect("temporary directory");
        let database = directory.path().join("config.db");
        let store = SqliteStore::connect(&database).await.expect("store");
        // WAL + NORMAL trades the per-transaction fsync of FULL for losing at
        // most the latest transactions on power failure; WAL still rules out
        // corruption, and telemetry flushes are no longer fsync-bound.
        for pool in [store.pool(), store.write_pool()] {
            let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
                .fetch_one(pool)
                .await
                .expect("synchronous mode");
            assert_eq!(synchronous, 1);
        }
        let auto_vacuum: i64 = sqlx::query_scalar("PRAGMA auto_vacuum")
            .fetch_one(store.pool())
            .await
            .expect("auto vacuum mode");
        assert_eq!(auto_vacuum, 2);

        #[cfg(unix)]
        for path in [
            database.clone(),
            sidecar_path(&database, "-wal"),
            sidecar_path(&database, "-shm"),
        ] {
            assert!(path.exists(), "{}", path.display());
            assert_eq!(mode(&path), 0o600, "{}", path.display());
        }
    }

    #[tokio::test]
    async fn write_pool_holds_a_single_connection() {
        let directory = tempdir().expect("temporary directory");
        let store = SqliteStore::connect(&directory.path().join("write-pool.db"))
            .await
            .expect("store");
        assert_eq!(store.write_pool().options().get_max_connections(), 1);

        let transaction = store.begin_write().await.expect("write transaction");
        assert!(
            store.write_pool().try_acquire().is_none(),
            "write connection is exclusively held"
        );
        drop(transaction);
    }

    #[tokio::test]
    async fn read_pool_retires_idle_connections() {
        let directory = tempdir().expect("temporary directory");
        let store = SqliteStore::connect(&directory.path().join("read-pool.db"))
            .await
            .expect("store");

        assert_eq!(
            store.pool().options().get_idle_timeout(),
            Some(READ_POOL_IDLE_TIMEOUT)
        );
    }

    #[tokio::test]
    async fn does_not_rewrite_an_existing_none_mode_database() {
        use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

        let directory = tempdir().expect("temporary directory");
        let database = directory.path().join("legacy.db");
        let options = SqliteConnectOptions::new()
            .filename(&database)
            .create_if_missing(true);
        let mut connection = SqliteConnection::connect_with(&options)
            .await
            .expect("legacy connection");
        sqlx::query("CREATE TABLE legacy_marker (id INTEGER PRIMARY KEY)")
            .execute(&mut connection)
            .await
            .expect("legacy schema");
        connection.close().await.expect("close legacy connection");

        let store = SqliteStore::connect(&database)
            .await
            .expect("upgraded store");
        let auto_vacuum: i64 = sqlx::query_scalar("PRAGMA auto_vacuum")
            .fetch_one(store.pool())
            .await
            .expect("legacy auto vacuum mode");
        assert_eq!(auto_vacuum, 0);
        let marker: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM legacy_marker")
            .fetch_one(store.pool())
            .await
            .expect("legacy table remains");
        assert_eq!(marker, 0);
    }

    #[cfg(unix)]
    fn mode(path: &std::path::Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;

        std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777
    }
}
