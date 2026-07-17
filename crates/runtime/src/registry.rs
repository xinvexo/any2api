use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct RuntimeRegistry {
    scheduler_epoch: AtomicU64,
}

impl RuntimeRegistry {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scheduler_epoch: AtomicU64::new(0),
        }
    }

    #[must_use]
    pub fn scheduler_epoch(&self) -> u64 {
        self.scheduler_epoch.load(Ordering::Acquire)
    }

    pub fn advance_scheduler_epoch(&self) -> u64 {
        self.scheduler_epoch.fetch_add(1, Ordering::AcqRel) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeRegistry;

    #[test]
    fn scheduler_epoch_is_monotonic() {
        let registry = RuntimeRegistry::new();

        assert_eq!(registry.advance_scheduler_epoch(), 1);
        assert_eq!(registry.advance_scheduler_epoch(), 2);
        assert_eq!(registry.scheduler_epoch(), 2);
    }
}
