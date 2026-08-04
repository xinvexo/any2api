use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

use any2api_domain::gateway_auth_rejected_capacity;

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
    queued_records: AtomicUsize,
    in_flight_records: AtomicUsize,
    dropped_records: AtomicU64,
    persisted_records: AtomicU64,
}

impl TelemetryCounters {
    pub(super) fn try_reserve_slot(&self, capacity: usize, class: TelemetryQueueClass) -> bool {
        if class == TelemetryQueueClass::GatewayAuthRejected {
            let total = u64::try_from(capacity).expect("telemetry queue capacity fits u64");
            let rejected_capacity = usize::try_from(gateway_auth_rejected_capacity(total))
                .expect("gateway auth rejection queue capacity fits usize");
            if !try_reserve(
                &self.inner.gateway_auth_rejected_queue_slots,
                rejected_capacity,
            ) {
                return false;
            }
        }
        if try_reserve(&self.inner.queue_slots, capacity) {
            true
        } else {
            self.release_gateway_auth_rejected_slot(class);
            false
        }
    }

    pub(super) fn reserve_control_slot(&self) {
        self.inner.queue_slots.fetch_add(1, Ordering::AcqRel);
    }

    pub(super) fn enqueued(&self, records: usize) {
        self.inner
            .queued_records
            .fetch_add(records, Ordering::AcqRel);
    }

    pub(super) fn send_failed(&self, records: usize, class: TelemetryQueueClass) {
        self.add_dropped(records);
        subtract(&self.inner.queued_records, records);
        subtract(&self.inner.queue_slots, 1);
        self.release_gateway_auth_rejected_slot(class);
    }

    pub(super) fn rejected(&self, records: usize) {
        self.add_dropped(records);
    }

    pub(super) fn received(&self, records: usize, class: TelemetryQueueClass) {
        self.inner
            .in_flight_records
            .fetch_add(records, Ordering::AcqRel);
        subtract(&self.inner.queued_records, records);
        subtract(&self.inner.queue_slots, 1);
        self.release_gateway_auth_rejected_slot(class);
    }

    pub(super) fn persisted(&self, records: usize) {
        self.inner
            .persisted_records
            .fetch_add(records as u64, Ordering::Relaxed);
        subtract(&self.inner.in_flight_records, records);
    }

    pub(super) fn storage_failed(&self, records: usize) {
        self.add_dropped(records);
        subtract(&self.inner.in_flight_records, records);
    }

    pub(super) fn writer_stopped(&self) {
        let queued = self.inner.queued_records.swap(0, Ordering::AcqRel);
        let in_flight = self.inner.in_flight_records.swap(0, Ordering::AcqRel);
        self.inner.queue_slots.store(0, Ordering::Release);
        self.inner
            .gateway_auth_rejected_queue_slots
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

    fn release_gateway_auth_rejected_slot(&self, class: TelemetryQueueClass) {
        if class == TelemetryQueueClass::GatewayAuthRejected {
            subtract(&self.inner.gateway_auth_rejected_queue_slots, 1);
        }
    }
}

fn try_reserve(counter: &AtomicUsize, capacity: usize) -> bool {
    let mut current = counter.load(Ordering::Acquire);
    loop {
        if current >= capacity {
            return false;
        }
        match counter.compare_exchange_weak(
            current,
            current + 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
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
    fn gateway_auth_rejected_slots_are_bounded_and_released_on_every_terminal_path() {
        let counters = TelemetryCounters::default();
        let class = TelemetryQueueClass::GatewayAuthRejected;
        assert!(counters.try_reserve_slot(4, class));
        assert!(!counters.try_reserve_slot(4, class));

        counters.enqueued(1);
        counters.send_failed(1, class);
        assert!(counters.try_reserve_slot(4, class));
        counters.enqueued(1);
        counters.received(1, class);
        counters.persisted(1);

        assert!(counters.try_reserve_slot(4, class));
        counters.enqueued(1);
        counters.writer_stopped();
        assert!(counters.try_reserve_slot(4, class));
        counters.enqueued(1);
        counters.writer_stopped();

        let metrics = counters.snapshot();
        assert_eq!(metrics.queued_records, 0);
        assert_eq!(metrics.in_flight_records, 0);
        assert_eq!(metrics.dropped_records, 3);
        assert_eq!(metrics.persisted_records, 1);
    }
}
