mod accumulation;
mod epochs;
mod sampling;

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
    observe_usage(
        estimator,
        state,
        usage(percent, reset_at),
        checkpoint,
        index,
    )
    .await
}

async fn observe_usage(
    estimator: &OAuthQuotaEstimator,
    state: Option<QuotaEstimatorState>,
    usage: OAuthQuotaUsage,
    checkpoint: RequestTelemetryCheckpoint,
    index: usize,
) -> super::EstimationResult {
    observe_identified(estimator, state, usage, "identity-a", checkpoint, index).await
}

async fn observe_identified(
    estimator: &OAuthQuotaEstimator,
    state: Option<QuotaEstimatorState>,
    usage: OAuthQuotaUsage,
    credential_fingerprint: &str,
    checkpoint: RequestTelemetryCheckpoint,
    index: usize,
) -> super::EstimationResult {
    estimator
        .observe(
            OAuthAccountId::from_uuid(Uuid::nil()),
            &usage,
            state,
            credential_fingerprint.into(),
            QuotaCostUnit::CodexCredits,
            indexed_observation(checkpoint, index),
            None,
        )
        .await
}

async fn learned_capacity_state(
    estimator: &OAuthQuotaEstimator,
    repository: &UsageRepository,
    baseline_percent: f64,
    reset_at: Option<i64>,
) -> QuotaEstimatorState {
    let state = observe(estimator, None, baseline_percent, reset_at, checkpoint(), 0)
        .await
        .state;
    repository.push(priced(150.0));
    observe(
        estimator,
        Some(state),
        baseline_percent + 10.0,
        reset_at,
        checkpoint(),
        1,
    )
    .await
    .state
}

fn usage(percent: f64, reset_at: Option<i64>) -> OAuthQuotaUsage {
    usage_for_window(window(percent, reset_at))
}

fn window(percent: f64, reset_at: Option<i64>) -> OAuthQuotaWindow {
    OAuthQuotaWindow {
        id: "primary".into(),
        kind: OAuthQuotaWindowKind::Time,
        used_percent: percent,
        limit_window_seconds: Some(18_000),
        reset_after_seconds: None,
        reset_at,
    }
}

fn usage_for_window(window: OAuthQuotaWindow) -> OAuthQuotaUsage {
    OAuthQuotaUsage {
        rate_limit: Some(OAuthQuotaRateLimit {
            allowed: Some(true),
            limit_reached: Some(false),
            windows: vec![window],
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

fn usage_with_tier(percent: f64, reset_at: Option<i64>, tier: &str) -> OAuthQuotaUsage {
    let mut usage = usage(percent, reset_at);
    usage.subscription_tier = Some(tier.into());
    usage
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

fn costless() -> OAuthQuotaRequestLogUsage {
    OAuthQuotaRequestLogUsage::default()
}

fn unpriced() -> OAuthQuotaRequestLogUsage {
    OAuthQuotaRequestLogUsage {
        unpriced_request_count: 1,
        ..OAuthQuotaRequestLogUsage::default()
    }
}

fn checkpoint() -> RequestTelemetryCheckpoint {
    checkpoint_for(Uuid::nil())
}

fn checkpoint_for(process_id: Uuid) -> RequestTelemetryCheckpoint {
    RequestTelemetryCheckpoint {
        process_id,
        enabled: true,
        policy_generation: 0,
        account_queue_dropped_request_logs: 0,
        account_storage_failed_request_logs: 0,
        unattributed_lost_request_logs: 0,
        pruned_through_sequence: 0,
    }
}

fn queue_dropped_checkpoint(dropped: u64) -> RequestTelemetryCheckpoint {
    RequestTelemetryCheckpoint {
        account_queue_dropped_request_logs: dropped,
        ..checkpoint()
    }
}

fn storage_failed_checkpoint(failed: u64) -> RequestTelemetryCheckpoint {
    RequestTelemetryCheckpoint {
        account_storage_failed_request_logs: failed,
        ..checkpoint()
    }
}

fn pruned_checkpoint(pruned_through_sequence: u64) -> RequestTelemetryCheckpoint {
    RequestTelemetryCheckpoint {
        pruned_through_sequence,
        ..checkpoint()
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
