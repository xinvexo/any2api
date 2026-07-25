use sqlx::SqlitePool;

const REQUEST_LOGS_MIGRATION: i64 = 9;
const GROK_PROVIDER_MIGRATION: i64 = 24;

pub(super) async fn run(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let Some(version) = latest_successful_migration(pool).await? else {
        return Ok(());
    };
    if !(REQUEST_LOGS_MIGRATION..GROK_PROVIDER_MIGRATION).contains(&version) {
        return Ok(());
    }

    let mut transaction = pool.begin().await?;
    sqlx::query(
        "UPDATE request_logs SET gateway_api_key_id = NULL \
         WHERE gateway_api_key_id IS NOT NULL \
         AND NOT EXISTS (SELECT 1 FROM gateway_api_keys \
                         WHERE gateway_api_keys.id = request_logs.gateway_api_key_id)",
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await
}

async fn latest_successful_migration(pool: &SqlitePool) -> Result<Option<i64>, sqlx::Error> {
    let migration_table_exists: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master \
         WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    if migration_table_exists == 0 {
        return Ok(None);
    }
    sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1")
        .fetch_one(pool)
        .await
}

#[cfg(test)]
#[path = "pre_v24_gateway_log_repair_tests.rs"]
mod tests;
