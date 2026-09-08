use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through};

#[tokio::test]
async fn request_cost_upgrade_preserves_old_amounts_without_inventing_exchange_rates() {
    let mut connection = SqliteConnection::connect(":memory:").await.expect("SQLite");
    migrate_through(&mut connection, 46).await;
    sqlx::query(
        "INSERT INTO request_logs \
         (request_id, started_at_ms, config_revision, ingress_protocol, operation, \
          status_code, attempt_count, latency_ms, quota_cost_unit, quota_cost_nanos, \
          quota_cost_rate_card, quota_service_tier, is_stream, client_ip) \
         VALUES ('historical', 1000, 1, 'openai_responses', 'responses', 200, 1, 10, \
                 'codex_credits', 268750000, 'historical-card', 'standard', 0, '127.0.0.1')",
    )
    .execute(&mut connection)
    .await
    .expect("old request");

    migrate_through(&mut connection, 47).await;
    let historical: (i64, String, Option<i64>) = sqlx::query_as(
        "SELECT quota_cost_nanos, quota_cost_rate_card, quota_credits_per_usd \
         FROM request_logs WHERE request_id = 'historical'",
    )
    .fetch_one(&mut connection)
    .await
    .expect("upgraded request");
    assert_eq!(historical, (268750000, "historical-card".into(), None));

    sqlx::query(
        "UPDATE request_logs SET quota_credits_per_usd = 25 WHERE request_id = 'historical'",
    )
    .execute(&mut connection)
    .await
    .expect("persist exchange rate");
    assert!(
        sqlx::query(
            "UPDATE request_logs SET quota_credits_per_usd = 0 WHERE request_id = 'historical'"
        )
        .execute(&mut connection)
        .await
        .is_err()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}
