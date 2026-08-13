use std::time::{SystemTime, UNIX_EPOCH};

use any2api_domain::{CompletedRequestLog, RequestTelemetryPosition};

use super::{
    RequestLogPolicy,
    event::{TelemetryEnvelope, TelemetryEvent},
    telemetry::RequestTelemetry,
};

#[derive(Clone, Debug)]
pub(crate) struct QuotaObservationBoundary {
    pub(crate) observed_at_ms: u64,
    pub(crate) position: RequestTelemetryPosition,
}

impl RequestTelemetry {
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

    pub(crate) async fn quota_observation(&self) -> QuotaObservationBoundary {
        let (observed_at_ms, position) = {
            let sequence = self
                .quota_sequence
                .lock()
                .expect("quota telemetry sequence");
            let position = RequestTelemetryPosition {
                process_id: self.process_id,
                sequence: *sequence,
            };
            (unix_time_ms(), position)
        };
        if let Some(receiver) = self.enqueue_quota_flush().await {
            let _ = receiver.await;
        }
        QuotaObservationBoundary {
            observed_at_ms,
            position,
        }
    }

    async fn enqueue_quota_flush(&self) -> Option<tokio::sync::oneshot::Receiver<()>> {
        let sender = self
            .sender
            .read()
            .expect("request telemetry sender")
            .clone()?;
        let permit = sender.reserve().await.ok()?;
        let (reply, result) = tokio::sync::oneshot::channel();
        self.counters.reserve_control_slot();
        permit.send(TelemetryEnvelope::new(TelemetryEvent::QuotaFlush { reply }));
        Some(result)
    }
}

fn unix_time_ms() -> u64 {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(milliseconds).unwrap_or(u64::MAX)
}
