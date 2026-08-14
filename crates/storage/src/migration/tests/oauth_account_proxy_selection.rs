use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::tempdir;

use super::{DIRECT_PROXY_ID, MIGRATOR, foreign_key_violations, migrate_through};

const ACCOUNT_ID: &str = "10000000-0000-0000-0000-000000000001";
const PROXY_ID: &str = "20000000-0000-0000-0000-000000000001";

#[tokio::test]
async fn existing_oauth_accounts_become_global_and_can_select_an_exact_profile() {
    let directory = tempdir().expect("temporary directory");
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("oauth-proxy.sqlite3"))
        .create_if_missing(true)
        .foreign_keys(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("SQLite pool");
    let mut connection = pool.acquire().await.expect("migration connection");
    migrate_through(&mut connection, 30).await;

    sqlx::query(
        "INSERT INTO proxy_profiles \
         (id, name, name_key, kind, host, port, enabled, built_in, config_version) \
         VALUES (?, 'OAuth HTTP', 'oauth http', 'http', '127.0.0.1', 8080, 1, 0, 1)",
    )
    .bind(PROXY_ID)
    .execute(&mut *connection)
    .await
    .expect("custom proxy");
    sqlx::query(
        "INSERT INTO oauth_accounts \
         (id, provider_kind, label, label_key, oauth_json, token_version, \
          account_generation, config_version, proxy_profile_id, requests_per_minute, enabled) \
         VALUES (?, 'codex', 'Existing', 'existing', ?, 3, 4, 5, ?, 60, 1)",
    )
    .bind(ACCOUNT_ID)
    .bind(br#"{"access_token":"secret","refresh_token":null,"id_token":null,"account_id":null,"email":null}"#.as_slice())
    .bind(DIRECT_PROXY_ID)
    .execute(&mut *connection)
    .await
    .expect("legacy OAuth account");
    sqlx::query(
        "INSERT INTO oauth_account_models (oauth_account_id, upstream_model) VALUES (?, 'gpt-5')",
    )
    .bind(ACCOUNT_ID)
    .execute(&mut *connection)
    .await
    .expect("OAuth model");

    MIGRATOR
        .run_direct(&mut *connection)
        .await
        .expect("proxy selection migration");

    let migrated = sqlx::query_as::<_, (Option<String>, i64, i64, i64)>(
        "SELECT proxy_profile_id, token_version, account_generation, config_version \
         FROM oauth_accounts WHERE id = ?",
    )
    .bind(ACCOUNT_ID)
    .fetch_one(&mut *connection)
    .await
    .expect("migrated OAuth account");
    assert_eq!(migrated, (None, 3, 4, 5));
    let model: String = sqlx::query_scalar(
        "SELECT upstream_model FROM oauth_account_models WHERE oauth_account_id = ?",
    )
    .bind(ACCOUNT_ID)
    .fetch_one(&mut *connection)
    .await
    .expect("preserved OAuth model");
    assert_eq!(model, "gpt-5");

    sqlx::query("UPDATE oauth_accounts SET proxy_profile_id = ? WHERE id = ?")
        .bind(PROXY_ID)
        .bind(ACCOUNT_ID)
        .execute(&mut *connection)
        .await
        .expect("select custom proxy");
    sqlx::query("UPDATE oauth_accounts SET proxy_profile_id = ? WHERE id = ?")
        .bind(DIRECT_PROXY_ID)
        .bind(ACCOUNT_ID)
        .execute(&mut *connection)
        .await
        .expect("select explicit DIRECT");

    assert!(foreign_key_violations(&mut connection).await.is_empty());
}
