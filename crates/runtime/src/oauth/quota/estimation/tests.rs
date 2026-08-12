mod epochs;
mod inheritance;
mod sampling;
mod segments;

use std::{collections::VecDeque, sync::Mutex};

use any2api_domain::{OAuthAccountId, QuotaCostUnit, RequestTelemetryPosition};
use any2api_provider::api::{
    OAuthQuotaRateLimit, OAuthQuotaUsage, OAuthQuotaWindow, OAuthQuotaWindowKind,
};
use any2api_storage::api::{
    OAuthQuotaEstimationRepository, OAuthQuotaRequestLogUsage, StorageError,
};
use async_trait::async_trait;
use uuid::Uuid;

use super::{OAuthQuotaEstimator, state::QuotaEstimatorState};
use crate::request_telemetry::{RequestTelemetryCheckpoint, RequestTelemetryObservation};

const RATE_CARD: &str = "openai_codex_credits_2026_08_11";

#[derive(Default)]
struct UsageRepository {
    usage: Mutex<VecDeque<Result<OAuthQuotaRequestLogUsage, StorageError>>>,
    queries: Mutex<Vec<(RequestTelemetryPosition, RequestTelemetryPosition)>>,
}

impl UsageRepository {
    fn push(&self, usage: OAuthQuotaRequestLogUsage) {
        self.usage.lock().unwrap().push_back(Ok(usage));
    }

    fn push_error(&self) {
        self.usage
            .lock()
            .unwrap()
            .push_back(Err(StorageError::CorruptTelemetry));
    }

    fn queries(&self) -> Vec<(RequestTelemetryPosition, RequestTelemetryPosition)> {
        self.queries.lock().unwrap().clone()
    }
}

#[async_trait]
impl OAuthQuotaEstimationRepository for UsageRepository {
    async fn oauth_quota_request_log_usage(
        &self,
        _id: OAuthAccountId,
        interval_start: RequestTelemetryPosition,
        interval_end: RequestTelemetryPosition,
    ) -> Result<OAuthQuotaRequestLogUsage, StorageError> {
        self.queries
            .lock()
            .unwrap()
            .push((interval_start, interval_end));
        self.usage
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Ok(priced(100.0)))
    }
}

async fn observe(
    estimator: &OAuthQuotaEstimator,
    state: Option<QuotaEstimatorState>,
    percent: f64,
    reset_at: Option<i64>,
    checkpoint: RequestTelemetryCheckpoint,
    index: usize,
) -> super::EstimationResult {
    estimator
        .observe(
            OAuthAccountId::from_uuid(Uuid::nil()),
            &usage(percent, reset_at),
            state,
            "identity-a".into(),
            QuotaCostUnit::CodexCredits,
            indexed_observation(checkpoint, index),
            None,
        )
        .await
}

fn usage(percent: f64, reset_at: Option<i64>) -> OAuthQuotaUsage {
    OAuthQuotaUsage {
        rate_limit: Some(OAuthQuotaRateLimit {
            allowed: Some(true),
            limit_reached: Some(false),
            windows: vec![OAuthQuotaWindow {
                id: "primary".into(),
                kind: OAuthQuotaWindowKind::Time,
                used_percent: percent,
                limit_window_seconds: Some(18_000),
                reset_after_seconds: None,
                reset_at,
            }],
        }),
        credits: None,
        access: None,
        reset_credits: None,
        billing: None,
        token_balance: None,
        subscription_tier: None,
        account_status: None,
    }
}

fn priced(credits: f64) -> OAuthQuotaRequestLogUsage {
    OAuthQuotaRequestLogUsage {
        unit: Some(QuotaCostUnit::CodexCredits),
        total_cost_nanos: (credits * 1_000_000_000.0) as u64,
        priced_request_count: 1,
        unpriced_request_count: 0,
        rate_cards: vec![RATE_CARD.into()],
    }
}

fn unpriced() -> OAuthQuotaRequestLogUsage {
    OAuthQuotaRequestLogUsage {
        unpriced_request_count: 1,
        ..OAuthQuotaRequestLogUsage::default()
    }
}

fn checkpoint(generation: u64) -> RequestTelemetryCheckpoint {
    checkpoint_for(Uuid::nil(), generation)
}

fn checkpoint_for(process_id: Uuid, generation: u64) -> RequestTelemetryCheckpoint {
    RequestTelemetryCheckpoint {
        process_id,
        enabled: true,
        policy_generation: 0,
        queue_dropped_request_logs: generation,
        storage_failed_request_logs: 0,
        pruned_request_logs: 0,
    }
}

fn storage_failed_checkpoint(generation: u64) -> RequestTelemetryCheckpoint {
    RequestTelemetryCheckpoint {
        process_id: Uuid::nil(),
        enabled: true,
        policy_generation: 0,
        queue_dropped_request_logs: 0,
        storage_failed_request_logs: generation,
        pruned_request_logs: 0,
    }
}

fn indexed_observation(
    checkpoint: RequestTelemetryCheckpoint,
    index: usize,
) -> RequestTelemetryObservation {
    telemetry_observation(checkpoint, (index as u64 + 1) * 1_000, index as u64)
}

fn telemetry_observation(
    checkpoint: RequestTelemetryCheckpoint,
    observed_at_ms: u64,
    sequence: u64,
) -> RequestTelemetryObservation {
    RequestTelemetryObservation {
        observed_at_ms,
        position: RequestTelemetryPosition {
            process_id: checkpoint.process_id,
            sequence,
        },
        checkpoint,
    }
}
