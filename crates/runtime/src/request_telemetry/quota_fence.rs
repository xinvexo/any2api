use std::time::{Duration, SystemTime, UNIX_EPOCH};

use any2api_domain::{CompletedRequestLog, RequestTelemetryPosition};

use super::{
    RequestLogPolicy, RequestTelemetryCheckpoint, RequestTelemetryObservation,
    event::{TelemetryEnvelope, TelemetryEvent},
    telemetry::RequestTelemetry,
};

const QUOTA_CHECKPOINT_TIMEOUT: Duration = Duration::from_secs(5);

impl RequestTelemetry {
    pub(super) fn omit_oauth_record(&self) {
        let mut sequence = self
            .quota_sequence
            .lock()
            .expect("quota telemetry sequence");
        *sequence = sequence
            .checked_add(1)
            .expect("quota telemetry sequence exhausted");
        self.counters.queue_dropped_request_logs(1);
    }

    pub(super) fn try_record_oauth(
        &self,
        mut record: CompletedRequestLog,
        policy: RequestLogPolicy,
    ) {
        let mut sequence = self
            .quota_sequence
            .lock()
            .expect("quota telemetry sequence");
        *sequence = sequence
            .checked_add(1)
            .expect("quota telemetry sequence exhausted");
        record.telemetry_position = Some(RequestTelemetryPosition {
            process_id: self.process_id,
            sequence: *sequence,
        });
        self.try_send_event(
            TelemetryEvent::RequestLog(Box::new(record)),
            policy.queue_capacity,
            policy.queue_max_bytes,
        );
    }

    pub(crate) async fn quota_checkpoint(&self) -> RequestTelemetryCheckpoint {
        let receiver = {
            let _sequence = self
                .quota_sequence
                .lock()
                .expect("quota telemetry sequence");
            self.enqueue_quota_checkpoint()
        };
        self.resolve_quota_checkpoint(receiver).await
    }

    pub(crate) async fn quota_observation(&self) -> RequestTelemetryObservation {
        let (observed_at_ms, position, receiver) = {
            let sequence = self
                .quota_sequence
                .lock()
                .expect("quota telemetry sequence");
            let position = RequestTelemetryPosition {
                process_id: self.process_id,
                sequence: *sequence,
            };
            (unix_time_ms(), position, self.enqueue_quota_checkpoint())
        };
        let checkpoint = self.resolve_quota_checkpoint(receiver).await;
        RequestTelemetryObservation {
            observed_at_ms,
            position,
            checkpoint,
        }
    }

    fn enqueue_quota_checkpoint(
        &self,
    ) -> Option<tokio::sync::oneshot::Receiver<RequestTelemetryCheckpoint>> {
        let enabled = self
            .policy
            .read()
            .expect("request telemetry policy")
            .enabled;
        if self.request_logs.is_none() || !enabled {
            return None;
        }
        let sender = self
            .sender
            .read()
            .expect("request telemetry sender")
            .clone()?;
        let permit = sender.try_reserve().ok()?;
        let (reply, result) = tokio::sync::oneshot::channel();
        let boundary = self.counters.quota_checkpoint(self.process_id, enabled);
        self.counters.reserve_control_slot();
        permit.send(TelemetryEnvelope::new(TelemetryEvent::QuotaCheckpoint {
            boundary,
            reply,
        }));
        Some(result)
    }

    async fn resolve_quota_checkpoint(
        &self,
        receiver: Option<tokio::sync::oneshot::Receiver<RequestTelemetryCheckpoint>>,
    ) -> RequestTelemetryCheckpoint {
        let fallback = || self.counters.quota_checkpoint(self.process_id, false);
        let Some(receiver) = receiver else {
            return fallback();
        };
        tokio::time::timeout(QUOTA_CHECKPOINT_TIMEOUT, receiver)
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_else(fallback)
    }
}

fn unix_time_ms() -> u64 {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(milliseconds).unwrap_or(u64::MAX)
}
