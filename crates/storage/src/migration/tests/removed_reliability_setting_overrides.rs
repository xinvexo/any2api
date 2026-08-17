use sqlx::{Connection, SqliteConnection};

use super::{migrate_through, migration_versions, migrator_through};

const REMOVED_KEYS: [&str; 17] = [
    "retry.max_total_attempts",
    "retry.max_credential_switches",
    "retry.max_same_credential_retries",
    "retry.base_delay",
    "retry.max_delay",
    "retry.jitter_ratio",
    "cooldown.rate_limit_fallback",
    "cooldown.model_unsupported",
    "cooldown.permission_denied",
    "cooldown.transient_endpoint",
    "breaker.endpoint.failure_threshold",
    "breaker.endpoint.failure_window",
    "breaker.endpoint.open_duration",
    "breaker.proxy.failure_threshold",
    "breaker.proxy.failure_window",
    "breaker.proxy.open_duration",
    "breaker.half_open_max_probes",
];

#[tokio::test]
async fn reliability_setting_migration_rejects_each_removed_override_and_preserves_data() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 34).await;

    for key in REMOVED_KEYS {
        sqlx::query("DELETE FROM setting_overrides")
            .execute(&mut connection)
            .await
            .expect("clear prior override");
        sqlx::query("INSERT INTO setting_overrides (key, value_json) VALUES (?, '1')")
            .bind(key)
            .execute(&mut connection)
            .await
            .expect("removed override");

        migrator_through(35)
            .run_direct(&mut connection)
            .await
            .expect_err("removed reliability override must reject migration");

        assert_eq!(
            migration_versions(&mut connection).await,
            (1..=34).collect::<Vec<_>>()
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT value_json FROM setting_overrides WHERE key = ?",
            )
            .bind(key)
            .fetch_one(&mut connection)
            .await
            .expect("preserved override"),
            "1"
        );
    }

    sqlx::query("DELETE FROM setting_overrides")
        .execute(&mut connection)
        .await
        .expect("remove rejected override");
    sqlx::query(
        "INSERT INTO setting_overrides (key, value_json) \
         VALUES ('retry.precommit_total_budget', '42')",
    )
    .execute(&mut connection)
    .await
    .expect("retained reliability override");

    migrate_through(&mut connection, 35).await;
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=35).collect::<Vec<_>>()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT value_json FROM setting_overrides \
             WHERE key = 'retry.precommit_total_budget'",
        )
        .fetch_one(&mut connection)
        .await
        .expect("preserved retained override"),
        "42"
    );
}
