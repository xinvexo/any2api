use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

use super::{foreign_key_violations, migrate_through, migration_versions};

#[tokio::test]
async fn version_cache_migration_preserves_existing_configuration() {
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(false),
    )
    .await
    .expect("SQLite connection");
    migrate_through(&mut connection, 44).await;
    sqlx::query("INSERT INTO setting_overrides (key, value_json) VALUES ('server.port', '4321')")
        .execute(&mut connection)
        .await
        .expect("existing setting");

    migrate_through(&mut connection, 45).await;

    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=45).collect::<Vec<_>>()
    );
    let existing: String =
        sqlx::query_scalar("SELECT value_json FROM setting_overrides WHERE key = 'server.port'")
            .fetch_one(&mut connection)
            .await
            .expect("preserved setting");
    assert_eq!(existing, "4321");
    sqlx::query(
        "INSERT INTO official_client_versions (provider_kind, version, fetched_at) \
         VALUES ('codex', '1.2.3', 100), ('claude', '2.3.4', 101), ('grok', '3.4.5', 102)",
    )
    .execute(&mut connection)
    .await
    .expect("version rows");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM official_client_versions")
        .fetch_one(&mut connection)
        .await
        .expect("version count");
    assert_eq!(count, 3);
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}
