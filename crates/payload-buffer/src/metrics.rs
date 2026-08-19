use std::sync::atomic::{AtomicUsize, Ordering};

static METRICS: AllocationMetrics = AllocationMetrics::new();

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PayloadBufferMetricsSnapshot {
    heap_current_bytes: usize,
    heap_peak_bytes: usize,
    mapped_current_bytes: usize,
    mapped_peak_bytes: usize,
    http_body_capture_current_bytes: usize,
    http_body_capture_peak_bytes: usize,
}

impl PayloadBufferMetricsSnapshot {
    #[must_use]
    pub const fn heap_current_bytes(self) -> usize {
        self.heap_current_bytes
    }

    #[must_use]
    pub const fn heap_peak_bytes(self) -> usize {
        self.heap_peak_bytes
    }

    #[must_use]
    pub const fn mapped_current_bytes(self) -> usize {
        self.mapped_current_bytes
    }

    #[must_use]
    pub const fn mapped_peak_bytes(self) -> usize {
        self.mapped_peak_bytes
    }

    #[must_use]
    pub const fn http_body_capture_current_bytes(self) -> usize {
        self.http_body_capture_current_bytes
    }

    #[must_use]
    pub const fn http_body_capture_peak_bytes(self) -> usize {
        self.http_body_capture_peak_bytes
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PayloadBufferUsage {
    #[default]
    General,
    HttpBodyCapture,
}

#[must_use]
pub fn payload_buffer_metrics() -> PayloadBufferMetricsSnapshot {
    METRICS.snapshot()
}

#[derive(Clone, Copy)]
pub(crate) enum AllocationKind {
    Heap,
    Mapped,
}

pub(crate) struct AllocationLease {
    kind: AllocationKind,
    usage: PayloadBufferUsage,
    bytes: usize,
}

impl AllocationLease {
    pub(crate) const fn empty(kind: AllocationKind, usage: PayloadBufferUsage) -> Self {
        Self {
            kind,
            usage,
            bytes: 0,
        }
    }

    pub(crate) fn new(kind: AllocationKind, usage: PayloadBufferUsage, bytes: usize) -> Self {
        METRICS.add(kind, usage, bytes);
        Self { kind, usage, bytes }
    }

    pub(crate) fn resize(&mut self, bytes: usize) {
        match bytes.cmp(&self.bytes) {
            std::cmp::Ordering::Greater => {
                METRICS.add(self.kind, self.usage, bytes - self.bytes);
            }
            std::cmp::Ordering::Less => {
                METRICS.subtract(self.kind, self.usage, self.bytes - bytes);
            }
            std::cmp::Ordering::Equal => return,
        }
        self.bytes = bytes;
    }

    pub(crate) const fn bytes(&self) -> usize {
        self.bytes
    }
}

impl Drop for AllocationLease {
    fn drop(&mut self) {
        METRICS.subtract(self.kind, self.usage, self.bytes);
    }
}

struct AllocationMetrics {
    heap: MetricPair,
    mapped: MetricPair,
    http_body_capture: MetricPair,
}

impl AllocationMetrics {
    const fn new() -> Self {
        Self {
            heap: MetricPair::new(),
            mapped: MetricPair::new(),
            http_body_capture: MetricPair::new(),
        }
    }

    fn add(&self, kind: AllocationKind, usage: PayloadBufferUsage, bytes: usize) {
        self.kind(kind).add(bytes);
        if usage == PayloadBufferUsage::HttpBodyCapture {
            self.http_body_capture.add(bytes);
        }
    }

    fn subtract(&self, kind: AllocationKind, usage: PayloadBufferUsage, bytes: usize) {
        self.kind(kind).subtract(bytes);
        if usage == PayloadBufferUsage::HttpBodyCapture {
            self.http_body_capture.subtract(bytes);
        }
    }

    fn kind(&self, kind: AllocationKind) -> &MetricPair {
        match kind {
            AllocationKind::Heap => &self.heap,
            AllocationKind::Mapped => &self.mapped,
        }
    }

    fn snapshot(&self) -> PayloadBufferMetricsSnapshot {
        let heap_current_bytes = self.heap.current();
        let mapped_current_bytes = self.mapped.current();
        let http_body_capture_current_bytes = self.http_body_capture.current();
        PayloadBufferMetricsSnapshot {
            heap_current_bytes,
            heap_peak_bytes: self.heap.peak().max(heap_current_bytes),
            mapped_current_bytes,
            mapped_peak_bytes: self.mapped.peak().max(mapped_current_bytes),
            http_body_capture_current_bytes,
            http_body_capture_peak_bytes: self
                .http_body_capture
                .peak()
                .max(http_body_capture_current_bytes),
        }
    }
}

struct MetricPair {
    current: AtomicUsize,
    peak: AtomicUsize,
}

impl MetricPair {
    const fn new() -> Self {
        Self {
            current: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        }
    }

    fn add(&self, bytes: usize) {
        if bytes == 0 {
            return;
        }
        let previous = self
            .current
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                Some(current.saturating_add(bytes))
            })
            .expect("owned byte update always succeeds");
        self.peak
            .fetch_max(previous.saturating_add(bytes), Ordering::Relaxed);
    }

    fn subtract(&self, bytes: usize) {
        if bytes == 0 {
            return;
        }
        let previous = self.current.fetch_sub(bytes, Ordering::AcqRel);
        debug_assert!(previous >= bytes, "payload allocation counter underflow");
    }

    fn current(&self) -> usize {
        self.current.load(Ordering::Acquire)
    }

    fn peak(&self) -> usize {
        self.peak.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::{AllocationKind, AllocationMetrics, PayloadBufferUsage};

    #[test]
    fn metrics_track_storage_and_capture_usage_without_changing_peaks_on_release() {
        let metrics = AllocationMetrics::new();

        metrics.add(AllocationKind::Heap, PayloadBufferUsage::General, 32);
        metrics.add(
            AllocationKind::Mapped,
            PayloadBufferUsage::HttpBodyCapture,
            128,
        );
        metrics.subtract(AllocationKind::Heap, PayloadBufferUsage::General, 32);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.heap_current_bytes(), 0);
        assert_eq!(snapshot.heap_peak_bytes(), 32);
        assert_eq!(snapshot.mapped_current_bytes(), 128);
        assert_eq!(snapshot.mapped_peak_bytes(), 128);
        assert_eq!(snapshot.http_body_capture_current_bytes(), 128);
        assert_eq!(snapshot.http_body_capture_peak_bytes(), 128);
    }
}
