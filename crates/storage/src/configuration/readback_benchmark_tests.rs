use std::{
    convert::Infallible,
    hint::black_box,
    time::{Duration, Instant},
};

use any2api_domain::{
    ConfigRevision, GatewayApiKeyId, SettingKey, SettingOverrideChange, SettingValue,
};
use tempfile::tempdir;

use crate::{
    configuration::{
        ConfigurationMutation, ConfigurationTransactionOutcome, ConfigurationTransactionRepository,
        StoredConfiguration, bump_revision, ensure_write_matches, load_configuration_from,
    },
    error::ConfigurationWriteComponent,
    gateway_api_key::GatewayApiKeyVerifier,
    sqlite::SqliteStore,
};

const GATEWAY_KEY_COUNT: usize = 10_000;
const SAMPLE_COUNT: usize = 7;

#[tokio::test]
#[ignore = "manual large-configuration publication benchmark"]
async fn large_setting_publish_compares_full_and_impact_readback() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("benchmark.sqlite3"))
        .await
        .expect("store");
    seed_gateway_keys(&store, GATEWAY_KEY_COUNT).await;

    baseline_prepare(&store).await;
    optimized_prepare(&store).await;
    let mut baseline = Vec::with_capacity(SAMPLE_COUNT);
    let mut optimized = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        baseline.push(measure(baseline_prepare(&store)).await);
        optimized.push(measure(optimized_prepare(&store)).await);
    }

    let baseline = median(baseline);
    let optimized = median(optimized);
    eprintln!(
        "configuration readback benchmark: {GATEWAY_KEY_COUNT} Gateway keys, median of \
         {SAMPLE_COUNT}; full+full={baseline:?}, full+setting-impact={optimized:?}, speedup={:.2}x",
        baseline.as_secs_f64() / optimized.as_secs_f64(),
    );
}

async fn baseline_prepare(store: &SqliteStore) {
    let mut transaction = store
        .pool()
        .begin_with("BEGIN IMMEDIATE")
        .await
        .expect("baseline transaction");
    let current = load_configuration_from(&mut transaction)
        .await
        .expect("baseline initial load");
    sqlx::query(
        "INSERT INTO setting_overrides (key, value_json) VALUES (?, 'true') \
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
    )
    .bind(SettingKey::SchedulerFallbackOnRateLimit.as_str())
    .execute(&mut *transaction)
    .await
    .expect("baseline setting write");
    let revision = bump_revision(&mut transaction, current.revision())
        .await
        .expect("baseline revision");
    let candidate = load_configuration_from(&mut transaction)
        .await
        .expect("baseline second full load");
    ensure_write_matches(
        candidate.revision(),
        revision,
        ConfigurationWriteComponent::Revision,
    )
    .expect("baseline revision readback");
    black_box(candidate.gateway_api_keys().keys().len());
    transaction.rollback().await.expect("baseline rollback");
}

async fn optimized_prepare(store: &SqliteStore) {
    let outcome = <SqliteStore as ConfigurationTransactionRepository<
        Infallible,
        StoredConfiguration,
    >>::transact_configuration(
        store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::ApplySettingChanges {
            changes: vec![SettingOverrideChange::Set {
                key: SettingKey::SchedulerFallbackOnRateLimit,
                value: SettingValue::Boolean(true),
            }],
        },
        Box::new(Err),
    )
    .await
    .expect("optimized candidate");
    let ConfigurationTransactionOutcome::Rejected(candidate) = outcome else {
        panic!("changed benchmark mutation must reach the rejecting compiler");
    };
    black_box(candidate.gateway_api_keys().keys().len());
}

async fn seed_gateway_keys(store: &SqliteStore, count: usize) {
    let verifier = GatewayApiKeyVerifier::new();
    let mut transaction = store.pool().begin().await.expect("seed transaction");
    for index in 0..count {
        let token = format!(
            "{}{:0width$}",
            any2api_domain::GATEWAY_TOKEN_PREFIX,
            index,
            width = any2api_domain::GATEWAY_TOKEN_BODY_LEN,
        );
        let token_prefix = token[..16].to_owned();
        let name = format!("Benchmark {index:05}");
        sqlx::query(
            "INSERT INTO gateway_api_keys \
             (id, name, name_key, token, token_prefix, token_hash, hash_version, \
              token_version, config_version, enabled, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, 2, 1, 1, 1, '2026-08-03 00:00:00')",
        )
        .bind(GatewayApiKeyId::new().to_string())
        .bind(&name)
        .bind(name.to_ascii_lowercase())
        .bind(&token)
        .bind(token_prefix)
        .bind(verifier.hash(token.as_bytes()).as_slice())
        .execute(&mut *transaction)
        .await
        .expect("seed gateway key");
    }
    transaction.commit().await.expect("commit seed data");
}

async fn measure(future: impl Future<Output = ()>) -> Duration {
    let started = Instant::now();
    future.await;
    started.elapsed()
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}
