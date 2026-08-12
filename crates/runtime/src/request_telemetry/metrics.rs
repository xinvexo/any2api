use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

use any2api_domain::gateway_auth_rejected_capacity;
use uuid::Uuid;

use super::RequestTelemetryCheckpoint;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TelemetryQueueClass {
    Regular,
    GatewayAuthRejected,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RequestTelemetryMetrics {
    pub queued_records: usize,
    pub in_flight_records: usize,
    pub dropped_records: u64,
    pub persisted_records: u64,
}

#[derive(Clone, Default)]
pub(super) struct TelemetryCounters {
    inner: Arc<Counters>,
}

#[derive(Default)]
struct Counters {
    queue_slots: AtomicUsize,
    gateway_auth_rejected_queue_slots: AtomicUsize,
    owned_bytes: AtomicUsize,
    gateway_auth_rejected_owned_bytes: AtomicUsize,
    queued_records: AtomicUsize,
    in_flight_records: AtomicUsize,
    queued_owned_bytes: AtomicUsize,
    in_flight_owned_bytes: AtomicUsize,
    dropped_records: AtomicU64,
    persisted_records: AtomicU64,
    request_log_policy_generation: AtomicU64,
    queue_dropped_request_logs: AtomicU64,
    storage_failed_request_logs: AtomicU64,
    pruned_request_logs: AtomicU64,
}

impl TelemetryCounters {
    pub(super) fn try_reserve(
        &self,
        capacity: usize,
        max_bytes: usize,
        owned_bytes: usize,
        class: TelemetryQueueClass,
    ) -> bool {
        if class == TelemetryQueueClass::GatewayAuthRejected {
            let total = u64::try_from(capacity).expect("telemetry queue capacity fits u64");
            let rejected_capacity = usize::try_from(gateway_auth_rejected_capacity(total))
                .expect("gateway auth rejection queue capacity fits usize");
            if !try_reserve_amount(
                &self.inner.gateway_auth_rejected_queue_slots,
                rejected_capacity,
                1,
            ) {
                return false;
            }
        }
        if !try_reserve_amount(&self.inner.queue_slots, capacity, 1) {
            self.release_gateway_auth_rejected_slot(class);
            return false;
        }
        if class == TelemetryQueueClass::GatewayAuthRejected {
            let total = u64::try_from(max_bytes).expect("telemetry byte limit fits u64");
            let rejected_max_bytes = usize::try_from(gateway_auth_rejected_capacity(total))
                .expect("gateway auth rejection byte limit fits usize");
            if !try_reserve_amount(
                &self.inner.gateway_auth_rejected_owned_bytes,
                rejected_max_bytes,
                owned_bytes,
            ) {
                subtract(&self.inner.queue_slots, 1);
                self.release_gateway_auth_rejected_slot(class);
                return false;
            }
        }
        if !try_reserve_amount(&self.inner.owned_bytes, max_bytes, owned_bytes) {
            if class == TelemetryQueueClass::GatewayAuthRejected {
                subtract(&self.inner.gateway_auth_rejected_owned_bytes, owned_bytes);
            }
            subtract(&self.inner.queue_slots, 1);
            self.release_gateway_auth_rejected_slot(class);
            return false;
        }
        true
    }

    pub(super) fn reserve_control_slot(&self) {
        self.inner.queue_slots.fetch_add(1, Ordering::AcqRel);
    }

    pub(super) fn enqueued(&self, records: usize, owned_bytes: usize) {
        self.inner
            .queued_records
            .fetch_add(records, Ordering::AcqRel);
        self.inner
            .queued_owned_bytes
            .fetch_add(owned_bytes, Ordering::AcqRel);
    }

    pub(super) fn send_failed(
        &self,
        records: usize,
        owned_bytes: usize,
        class: TelemetryQueueClass,
        quota_relevant: bool,
    ) {
        self.add_dropped(records);
        if quota_relevant {
            self.queue_dropped_request_logs(records);
        }
        subtract(&self.inner.queued_records, records);
        subtract(&self.inner.queued_owned_bytes, owned_bytes);
        subtract(&self.inner.queue_slots, 1);
        self.release_gateway_auth_rejected_slot(class);
        self.release_owned_bytes(owned_bytes, class);
    }

    pub(super) fn rejected(&self, records: usize, quota_relevant: bool) {
        self.add_dropped(records);
        if quota_relevant {
            self.queue_dropped_request_logs(records);
        }
    }

    pub(super) fn received(&self, records: usize, owned_bytes: usize, class: TelemetryQueueClass) {
        self.inner
            .in_flight_records
            .fetch_add(records, Ordering::AcqRel);
        self.inner
            .in_flight_owned_bytes
            .fetch_add(owned_bytes, Ordering::AcqRel);
        subtract(&self.inner.queued_records, records);
        subtract(&self.inner.queued_owned_bytes, owned_bytes);
        subtract(&self.inner.queue_slots, 1);
        self.release_gateway_auth_rejected_slot(class);
    }

    pub(super) fn persisted(
        &self,
        records: usize,
        owned_bytes: usize,
        rejected_owned_bytes: usize,
    ) {
        self.inner
            .persisted_records
            .fetch_add(records as u64, Ordering::Relaxed);
        subtract(&self.inner.in_flight_records, records);
        self.finish_owned_bytes(owned_bytes, rejected_owned_bytes);
    }

    pub(super) fn storage_failed(
        &self,
        records: usize,
        owned_bytes: usize,
        rejected_owned_bytes: usize,
    ) {
        self.add_dropped(records);
        subtract(&self.inner.in_flight_records, records);
        self.finish_owned_bytes(owned_bytes, rejected_owned_bytes);
    }

    pub(super) fn request_logs_storage_failed(
        &self,
        records: usize,
        owned_bytes: usize,
        quota_relevant_records: usize,
    ) {
        self.storage_failed(records, owned_bytes, 0);
        self.inner
            .storage_failed_request_logs
            .fetch_add(quota_relevant_records as u64, Ordering::Relaxed);
    }

    pub(super) fn request_logs_pruned(&self, records: u64) {
        if records == 0 {
            return;
        }
        self.inner
            .pruned_request_logs
            .fetch_add(records, Ordering::Relaxed);
    }

    pub(super) fn request_log_policy_changed(&self) {
        self.inner
            .request_log_policy_generation
            .fetch_add(1, Ordering::AcqRel);
    }

    pub(super) fn quota_checkpoint(
        &self,
        process_id: Uuid,
        enabled: bool,
    ) -> RequestTelemetryCheckpoint {
        RequestTelemetryCheckpoint {
            process_id,
            enabled,
            policy_generation: self
                .inner
                .request_log_policy_generation
                .load(Ordering::Acquire),
            queue_dropped_request_logs: self
                .inner
                .queue_dropped_request_logs
                .load(Ordering::Relaxed),
            storage_failed_request_logs: self
                .inner
                .storage_failed_request_logs
                .load(Ordering::Relaxed),
            pruned_request_logs: self.inner.pruned_request_logs.load(Ordering::Relaxed),
        }
    }

    pub(super) fn complete_quota_checkpoint(
        &self,
        mut boundary: RequestTelemetryCheckpoint,
    ) -> RequestTelemetryCheckpoint {
        let current = self.quota_checkpoint(boundary.process_id, boundary.enabled);
        boundary.storage_failed_request_logs = current.storage_failed_request_logs;
        boundary.pruned_request_logs = current.pruned_request_logs;
        boundary
    }

    pub(super) fn writer_stopped(&self) {
        let queued = self.inner.queued_records.swap(0, Ordering::AcqRel);
        let in_flight = self.inner.in_flight_records.swap(0, Ordering::AcqRel);
        self.inner.queued_owned_bytes.store(0, Ordering::Release);
        self.inner.in_flight_owned_bytes.store(0, Ordering::Release);
        self.inner.owned_bytes.store(0, Ordering::Release);
        self.inner.queue_slots.store(0, Ordering::Release);
        self.inner
            .gateway_auth_rejected_queue_slots
            .store(0, Ordering::Release);
        self.inner
            .gateway_auth_rejected_owned_bytes
            .store(0, Ordering::Release);
        self.add_dropped(queued.saturating_add(in_flight));
    }

    pub(super) fn snapshot(&self) -> RequestTelemetryMetrics {
        RequestTelemetryMetrics {
            queued_records: self.inner.queued_records.load(Ordering::Acquire),
            in_flight_records: self.inner.in_flight_records.load(Ordering::Acquire),
            dropped_records: self.inner.dropped_records.load(Ordering::Relaxed),
            persisted_records: self.inner.persisted_records.load(Ordering::Relaxed),
        }
    }

    fn add_dropped(&self, records: usize) {
        self.inner
            .dropped_records
            .fetch_add(records as u64, Ordering::Relaxed);
    }

    pub(super) fn queue_dropped_request_logs(&self, records: usize) {
        self.inner
            .queue_dropped_request_logs
            .fetch_add(records as u64, Ordering::Relaxed);
    }

    fn release_gateway_auth_rejected_slot(&self, class: TelemetryQueueClass) {
        if class == TelemetryQueueClass::GatewayAuthRejected {
            subtract(&self.inner.gateway_auth_rejected_queue_slots, 1);
        }
    }

    fn release_owned_bytes(&self, owned_bytes: usize, class: TelemetryQueueClass) {
        subtract(&self.inner.owned_bytes, owned_bytes);
        if class == TelemetryQueueClass::GatewayAuthRejected {
            subtract(&self.inner.gateway_auth_rejected_owned_bytes, owned_bytes);
        }
    }

    fn finish_owned_bytes(&self, owned_bytes: usize, rejected_owned_bytes: usize) {
        subtract(&self.inner.in_flight_owned_bytes, owned_bytes);
        subtract(&self.inner.owned_bytes, owned_bytes);
        subtract(
            &self.inner.gateway_auth_rejected_owned_bytes,
            rejected_owned_bytes,
        );
    }

    #[cfg(test)]
    pub(super) fn owned_bytes(&self) -> (usize, usize, usize) {
        (
            self.inner.queued_owned_bytes.load(Ordering::Acquire),
            self.inner.in_flight_owned_bytes.load(Ordering::Acquire),
            self.inner.owned_bytes.load(Ordering::Acquire),
        )
    }
}

fn try_reserve_amount(counter: &AtomicUsize, capacity: usize, amount: usize) -> bool {
    let mut current = counter.load(Ordering::Acquire);
    loop {
        let Some(next) = current.checked_add(amount) else {
            return false;
        };
        if next > capacity {
            return false;
        }
        match counter.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return true,
            Err(actual) => current = actual,
        }
    }
}

fn subtract(counter: &AtomicUsize, amount: usize) {
    let previous = counter.fetch_sub(amount, Ordering::AcqRel);
    debug_assert!(previous >= amount, "telemetry counter underflow");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_owned_bytes_are_held_until_the_storage_terminal_path() {
        let counters = TelemetryCounters::default();
        let class = TelemetryQueueClass::Regular;
        assert!(counters.try_reserve(8, 150, 100, class));
        counters.enqueued(1, 100);
        counters.received(1, 100, class);
        assert_eq!(counters.owned_bytes(), (0, 100, 100));
        assert!(!counters.try_reserve(8, 150, 51, class));

        counters.persisted(1, 100, 0);
        assert_eq!(counters.owned_bytes(), (0, 0, 0));
        assert!(counters.try_reserve(8, 150, 100, class));
        counters.enqueued(1, 100);
        counters.received(1, 100, class);
        counters.storage_failed(1, 100, 0);
        assert_eq!(counters.owned_bytes(), (0, 0, 0));

        assert!(counters.try_reserve(8, 150, 100, class));
        counters.enqueued(1, 100);
        counters.writer_stopped();
    }

    #[test]
    fn gateway_auth_rejected_slots_are_bounded_and_released_on_every_terminal_path() {
        let counters = TelemetryCounters::default();
        let class = TelemetryQueueClass::GatewayAuthRejected;
        assert!(counters.try_reserve(4, 400, 100, class));
        assert!(!counters.try_reserve(4, 400, 100, class));

        counters.enqueued(1, 100);
        counters.send_failed(1, 100, class, false);
        assert!(counters.try_reserve(4, 400, 100, class));
        counters.enqueued(1, 100);
        counters.received(1, 100, class);
        assert_eq!(counters.owned_bytes(), (0, 100, 100));
        assert!(!counters.try_reserve(4, 400, 1, class));
        counters.persisted(1, 100, 100);

        assert!(counters.try_reserve(4, 400, 100, class));
        counters.enqueued(1, 100);
        counters.writer_stopped();
        assert!(counters.try_reserve(4, 400, 100, class));
        counters.enqueued(1, 100);
        counters.writer_stopped();

        let metrics = counters.snapshot();
        assert_eq!(metrics.queued_records, 0);
        assert_eq!(metrics.in_flight_records, 0);
        assert_eq!(metrics.dropped_records, 3);
        assert_eq!(metrics.persisted_records, 1);
    }
}
