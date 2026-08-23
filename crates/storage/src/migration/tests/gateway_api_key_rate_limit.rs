use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

const GATEWAY_ID: &str = "10000000-0000-4000-8000-000000000046";

#[tokio::test]
async fn gateway_rate_limit_migration_preserves_keys_and_adds_a_bounded_optional_limit() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 45).await;
    let token = format!("sk-{}", "A".repeat(43));
    sqlx::query(
        "INSERT INTO gateway_api_keys \
         (id, name, name_key, token, token_prefix, token_hash, hash_version, token_version, \
          config_version, enabled) \
         VALUES (?, 'Existing', 'existing', ?, 'sk-AAAAAAAAAAAAA', zeroblob(32), 2, 4, 9, 1)",
    )
    .bind(GATEWAY_ID)
    .bind(token)
    .execute(&mut connection)
    .await
    .expect("existing gateway key");

    migrate_through(&mut connection, 46).await;

    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=46).collect::<Vec<_>>()
    );
    let rpm: Option<i64> =
        sqlx::query_scalar("SELECT requests_per_minute FROM gateway_api_keys WHERE id = ?")
            .bind(GATEWAY_ID)
            .fetch_one(&mut connection)
            .await
            .expect("preserved gateway key");
    assert_eq!(rpm, None);

    for invalid in [0_i64, 100_001] {
        let error = sqlx::query("UPDATE gateway_api_keys SET requests_per_minute = ? WHERE id = ?")
            .bind(invalid)
            .bind(GATEWAY_ID)
            .execute(&mut connection)
            .await
            .expect_err("out-of-range RPM must be rejected");
        assert!(error.as_database_error().is_some());
    }
    sqlx::query("UPDATE gateway_api_keys SET requests_per_minute = 600 WHERE id = ?")
        .bind(GATEWAY_ID)
        .execute(&mut connection)
        .await
        .expect("bounded RPM");
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}
