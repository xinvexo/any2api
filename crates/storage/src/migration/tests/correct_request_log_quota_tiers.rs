use serde_json::json;
use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

const BUILT_IN_CARD: &str = "openai_codex_credits_2026_08_11";
const STANDARD_COST_56: i64 = 268_750_000;
const FAST_COST_56: i64 = 671_875_000;

#[tokio::test]
async fn quota_tier_correction_reprices_only_verified_unconfirmed_fast_rows() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 37).await;
    sqlx::query(
        "INSERT INTO setting_overrides (key, value_json) VALUES ('oauth.codex.rate_card', ?)",
    )
    .bind(
        json!({
            "id": "custom-card",
            "credits_per_usd": 25,
            "models": {
                "gpt-5.6-sol": {
                    "standard": {
                        "input_nanos_per_million": 125_000_000_000_u64,
                        "cached_input_nanos_per_million": 12_500_000_000_u64,
                        "output_nanos_per_million": 750_000_000_000_u64
                    },
                    "fast": {
                        "input_nanos_per_million": 312_500_000_000_u64,
                        "cached_input_nanos_per_million": 31_250_000_000_u64,
                        "output_nanos_per_million": 1_875_000_000_000_u64
                    }
                }
            }
        })
        .to_string(),
    )
    .execute(&mut connection)
    .await
    .expect("custom rate card setting");

    insert_log(
        &mut connection,
        "effective-standard",
        "gpt-5.6-sol",
        BUILT_IN_CARD,
        FAST_COST_56,
        "fast",
        Some("standard"),
    )
    .await;
    insert_log(
        &mut connection,
        "effective-missing",
        "gpt-5.6-sol",
        BUILT_IN_CARD,
        FAST_COST_56,
        "fast",
        None,
    )
    .await;
    insert_log(
        &mut connection,
        "effective-fast",
        "gpt-5.6-sol",
        BUILT_IN_CARD,
        FAST_COST_56,
        "fast",
        Some("fast"),
    )
    .await;
    insert_log(
        &mut connection,
        "different-model-ratio",
        "gpt-5.4",
        BUILT_IN_CARD,
        STANDARD_COST_56,
        "fast",
        None,
    )
    .await;
    insert_log(
        &mut connection,
        "custom-card",
        "gpt-5.6-sol",
        "custom-card",
        FAST_COST_56,
        "fast",
        None,
    )
    .await;
    insert_log(
        &mut connection,
        "unknown-card",
        "gpt-5.6-sol",
        "unknown-card",
        FAST_COST_56,
        "fast",
        None,
    )
    .await;
    insert_log(
        &mut connection,
        "nonmatching-cost",
        "gpt-5.6-sol",
        BUILT_IN_CARD,
        FAST_COST_56 + 1,
        "fast",
        None,
    )
    .await;
    insert_log(
        &mut connection,
        "public-alias",
        "my-codex-model",
        BUILT_IN_CARD,
        FAST_COST_56,
        "fast",
        None,
    )
    .await;

    migrate_through(&mut connection, 38).await;

    assert_eq!(
        cost_and_tier(&mut connection, "effective-standard").await,
        (STANDARD_COST_56, "standard".to_owned())
    );
    assert_eq!(
        cost_and_tier(&mut connection, "effective-missing").await,
        (STANDARD_COST_56, "standard".to_owned())
    );
    assert_eq!(
        cost_and_tier(&mut connection, "effective-fast").await,
        (FAST_COST_56, "fast".to_owned())
    );
    assert_eq!(
        cost_and_tier(&mut connection, "different-model-ratio").await,
        (134_375_000, "standard".to_owned())
    );
    assert_eq!(
        cost_and_tier(&mut connection, "custom-card").await,
        (STANDARD_COST_56, "standard".to_owned())
    );
    assert_eq!(
        cost_and_tier(&mut connection, "unknown-card").await,
        (FAST_COST_56, "fast".to_owned())
    );
    assert_eq!(
        cost_and_tier(&mut connection, "nonmatching-cost").await,
        (FAST_COST_56 + 1, "fast".to_owned())
    );
    assert_eq!(
        cost_and_tier(&mut connection, "public-alias").await,
        (FAST_COST_56, "fast".to_owned())
    );
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=38).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}

async fn insert_log(
    connection: &mut SqliteConnection,
    request_id: &str,
    public_model: &str,
    rate_card: &str,
    quota_cost_nanos: i64,
    quota_service_tier: &str,
    effective_speed_tier: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO request_logs \
         (request_id, started_at_ms, config_revision, ingress_protocol, operation, \
          public_model, status_code, attempt_count, latency_ms, input_tokens, output_tokens, \
          cache_read_tokens, quota_cost_unit, quota_cost_nanos, quota_cost_rate_card, \
          quota_service_tier, requested_speed_tier, effective_speed_tier, is_stream, client_ip) \
         VALUES (?, 1000, 1, 'openai_responses', 'responses', ?, 200, 1, 10, 2000, 100, \
                 500, 'codex_credits', ?, ?, ?, 'fast', ?, 0, '127.0.0.1')",
    )
    .bind(request_id)
    .bind(public_model)
    .bind(quota_cost_nanos)
    .bind(rate_card)
    .bind(quota_service_tier)
    .bind(effective_speed_tier)
    .execute(connection)
    .await
    .expect("request log");
}

async fn cost_and_tier(connection: &mut SqliteConnection, request_id: &str) -> (i64, String) {
    sqlx::query_as(
        "SELECT quota_cost_nanos, quota_service_tier FROM request_logs WHERE request_id = ?",
    )
    .bind(request_id)
    .fetch_one(connection)
    .await
    .expect("request log cost")
}
