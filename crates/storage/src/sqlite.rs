use std::path::Path;

use sqlx::{
    Connection, SqliteConnection, SqlitePool,
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

#[derive(Clone, Debug)]
pub struct SqliteStore {
    pool: SqlitePool,
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
            .synchronous(SqliteSynchronous::Full);
        let migration_options = options.clone().auto_vacuum(SqliteAutoVacuum::Incremental);
        let mut migration_connection = SqliteConnection::connect_with(&migration_options).await?;
        migration::run(&mut migration_connection).await?;
        migration_connection.close().await?;
        protect_sqlite_files(path)?;

        let options = options.foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await?;
        protect_sqlite_files(path)?;
        Ok(Self { pool })
    }

    #[must_use]
    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn close(&self) {
        self.pool.close().await;
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

    use super::{SqliteStore, sidecar_path};

    #[tokio::test]
    async fn uses_full_synchronous_mode_and_private_sidecars() {
        let directory = tempdir().expect("temporary directory");
        let database = directory.path().join("config.db");
        let store = SqliteStore::connect(&database).await.expect("store");
        let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
            .fetch_one(store.pool())
            .await
            .expect("synchronous mode");
        assert_eq!(synchronous, 2);
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
