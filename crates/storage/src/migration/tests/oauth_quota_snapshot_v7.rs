use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

const ACCOUNT_ID: &str = "11111111-1111-4111-8111-111111111111";

#[tokio::test]
async fn quota_snapshot_v7_migration_preserves_usage_and_clears_v6_estimator_state() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 26).await;
    insert_representative_v6_state(&mut connection).await;

    migrate_through(&mut connection, 27).await;

    assert_snapshot_migrated(&mut connection).await;
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=27).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}

async fn insert_representative_v6_state(connection: &mut SqliteConnection) {
    sqlx::query(
        "INSERT INTO oauth_accounts \
         (id, provider_kind, label, label_key, oauth_json, token_version, account_generation, \
          config_version, requests_per_minute, enabled) \
         VALUES (?, 'codex', 'Existing account', 'existing account', \
                 CAST('{\"access_token\":\"preserved\"}' AS BLOB), 3, 4, 5, 60, 1)",
    )
    .bind(ACCOUNT_ID)
    .execute(&mut *connection)
    .await
    .expect("representative OAuth account");
    let payload = serde_json::json!({
        "usage": {
            "rate_limit": {
                "allowed": true,
                "limit_reached": false,
                "windows": [{
                    "id": "primary",
                    "kind": "time",
                    "used_percent": 42.0,
                    "limit_window_seconds": 18000,
                    "reset_after_seconds": 100,
                    "reset_at": 1900000100
                }]
            },
            "credits": null,
            "access": null,
            "reset_credits": null,
            "billing": null,
            "token_balance": null,
            "subscription_tier": null,
            "account_status": null
        },
        "estimator_state": {
            "credential_fingerprint": "identity-a",
            "next_epoch": 3,
            "windows": [{"samples_without_epoch": "must be cleared"}]
        }
    });
    sqlx::query(
        "INSERT INTO oauth_quota_snapshots \
         (oauth_account_id, schema_version, fetched_at, payload, updated_at) \
         VALUES (?, 6, 1234, CAST(? AS BLOB), '2026-08-12 00:00:00')",
    )
    .bind(ACCOUNT_ID)
    .bind(serde_json::to_vec(&payload).expect("v6 payload"))
    .execute(&mut *connection)
    .await
    .expect("v6 quota snapshot");
}

async fn assert_snapshot_migrated(connection: &mut SqliteConnection) {
    let (version, fetched_at, payload, updated_at) =
        sqlx::query_as::<_, (i64, i64, Vec<u8>, String)>(
            "SELECT schema_version, fetched_at, payload, updated_at FROM oauth_quota_snapshots \
             WHERE oauth_account_id = ?",
        )
        .bind(ACCOUNT_ID)
        .fetch_one(&mut *connection)
        .await
        .expect("v7 quota snapshot");
    let payload: serde_json::Value = serde_json::from_slice(&payload).expect("v7 payload JSON");
    assert_eq!(version, 7);
    assert_eq!(fetched_at, 1234);
    assert_eq!(
        payload["usage"]["rate_limit"]["windows"][0]["used_percent"],
        42.0
    );
    assert!(payload["estimator_state"].is_null());
    assert_eq!(updated_at, "2026-08-12 00:00:00");
    assert!(
        sqlx::query(
            "INSERT INTO oauth_quota_snapshots \
             (oauth_account_id, schema_version, fetched_at, payload) \
             VALUES ('55555555-5555-4555-8555-555555555555', 6, 1, CAST('{}' AS BLOB))",
        )
        .execute(connection)
        .await
        .is_err()
    );
}
