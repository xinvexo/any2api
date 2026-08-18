use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions, migrator_through};

const GATEWAY_ID: &str = "10000000-0000-4000-8000-000000000001";

#[tokio::test]
async fn standard_prefix_migration_refuses_old_gateway_keys_before_ddl() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 19).await;
    let old_token = format!("a2k_v1_{}", "A".repeat(43));
    sqlx::query(
        "INSERT INTO gateway_api_keys \
         (id, name, name_key, token, token_prefix, token_hash, hash_version, token_version, \
          config_version, enabled, created_at, last_used_at, updated_at) \
         VALUES (?, 'Existing', 'existing', ?, 'a2k_v1_AAAAAAAA', zeroblob(32), 2, 4, 9, 0, \
                 '2026-08-11 10:00:00', '2026-08-11 11:00:00', '2026-08-11 12:00:00')",
    )
    .bind(GATEWAY_ID)
    .bind(&old_token)
    .execute(&mut connection)
    .await
    .expect("legacy gateway key");

    migrator_through(20)
        .run_direct(None, &mut connection, false)
        .await
        .expect_err("old gateway keys must be rejected before migration DDL");

    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=19).collect::<Vec<_>>()
    );
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT token, token_prefix FROM gateway_api_keys WHERE id = ?",
    )
    .bind(GATEWAY_ID)
    .fetch_one(&mut connection)
    .await
    .expect("unchanged legacy gateway key");
    assert_eq!(row, (old_token, "a2k_v1_AAAAAAAA".to_owned()));
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}
