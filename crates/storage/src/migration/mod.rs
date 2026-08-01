use sqlx::{SqliteConnection, migrate::Migrator};

use crate::error::StorageError;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

pub(crate) async fn run(connection: &mut SqliteConnection) -> Result<(), StorageError> {
    MIGRATOR.run(&mut *connection).await?;
    let violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(connection)
        .await?;
    if !violations.is_empty() {
        return Err(StorageError::CorruptConfiguration);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
