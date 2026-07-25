use sqlx::{SqlitePool, migrate::Migrator};

use crate::error::StorageError;

mod pre_v24_gateway_log_repair;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

pub(crate) async fn run(pool: &SqlitePool) -> Result<(), StorageError> {
    pre_v24_gateway_log_repair::run(pool).await?;
    MIGRATOR.run(pool).await?;
    Ok(())
}

#[cfg(test)]
#[path = "migration_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "migration_grok_tests.rs"]
mod grok_tests;

#[cfg(test)]
#[path = "migration_grok_oauth_tests.rs"]
mod grok_oauth_tests;
