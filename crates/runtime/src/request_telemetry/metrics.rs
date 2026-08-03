use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

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
    queued_records: AtomicUsize,
    in_flight_records: AtomicUsize,
    dropped_records: AtomicU64,
    persisted_records: AtomicU64,
}

impl TelemetryCounters {
    pub(super) fn try_reserve_slot(&self, capacity: usize) -> bool {
        let mut current = self.inner.queue_slots.load(Ordering::Acquire);
        loop {
            if current >= capacity {
                return false;
            }
            match self.inner.queue_slots.compare_exchange_weak(
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

    pub(super) fn reserve_control_slot(&self) {
        self.inner.queue_slots.fetch_add(1, Ordering::AcqRel);
    }

    pub(super) fn enqueued(&self, records: usize) {
        self.inner
            .queued_records
            .fetch_add(records, Ordering::AcqRel);
    }

    pub(super) fn send_failed(&self, records: usize) {
        self.add_dropped(records);
        subtract(&self.inner.queued_records, records);
        subtract(&self.inner.queue_slots, 1);
    }

    pub(super) fn rejected(&self, records: usize) {
        self.add_dropped(records);
    }

    pub(super) fn received(&self, records: usize) {
        self.inner
            .in_flight_records
            .fetch_add(records, Ordering::AcqRel);
        subtract(&self.inner.queued_records, records);
        subtract(&self.inner.queue_slots, 1);
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
}

fn subtract(counter: &AtomicUsize, amount: usize) {
    let previous = counter.fetch_sub(amount, Ordering::AcqRel);
    debug_assert!(previous >= amount, "telemetry counter underflow");
}
