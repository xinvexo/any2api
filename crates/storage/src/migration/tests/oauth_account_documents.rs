use serde_json::{Value, json};
use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

#[tokio::test]
async fn migration_canonicalizes_representative_provider_documents() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 12).await;

    for (id, provider, document, email, expires_at) in [
        (
            "codex-account",
            "codex",
            br#"{"type":"codex","access_token":"codex-access","refresh_token":"codex-refresh","id_token":"codex-id","account_id":"codex-subject","last_refresh":"2026-01-01T00:00:00Z","expired":"2027-01-15T08:00:00Z"}"#.as_slice(),
            Some("codex@example.com"),
            Some(1_800_000_000_i64),
        ),
        (
            "grok-account",
            "grok",
            br#"{"type":"grok","access_token":"grok-access","refresh_token":"","sub":"grok-subject","email":""}"#.as_slice(),
            Some("grok@example.com"),
            None,
        ),
    ] {
        sqlx::query(
            "INSERT INTO oauth_accounts \
             (id, provider_kind, label, label_key, oauth_json, token_version, \
              account_generation, config_version, requests_per_minute, enabled, \
              safe_account_email, expires_at) \
             VALUES (?, ?, ?, ?, ?, 3, 4, 5, 60, 1, ?, ?)",
        )
        .bind(id)
        .bind(provider)
        .bind(id)
        .bind(id)
        .bind(document)
        .bind(email)
        .bind(expires_at)
        .execute(&mut connection)
        .await
        .expect("representative OAuth account");
    }

    migrate_through(&mut connection, 13).await;

    let rows = sqlx::query_as::<_, (String, String, Vec<u8>, i64, i64, i64)>(
        "SELECT id, typeof(oauth_json), oauth_json, token_version, account_generation, \
         config_version FROM oauth_accounts ORDER BY id",
    )
    .fetch_all(&mut connection)
    .await
    .expect("canonical OAuth documents");
    let expected = [
        json!({
            "access_token": "codex-access",
            "refresh_token": "codex-refresh",
            "id_token": "codex-id",
            "account_id": "codex-subject",
            "email": "codex@example.com",
        }),
        json!({
            "access_token": "grok-access",
            "refresh_token": null,
            "id_token": null,
            "account_id": "grok-subject",
            "email": "grok@example.com",
        }),
    ];
    for (row, expected) in rows.iter().zip(expected) {
        assert_eq!(row.1, "blob");
        assert_eq!(
            serde_json::from_slice::<Value>(&row.2).expect("JSON"),
            expected
        );
        assert_eq!((row.3, row.4, row.5), (3, 4, 5));
    }
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=13).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}
