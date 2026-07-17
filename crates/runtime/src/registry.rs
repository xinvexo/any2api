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

    pub(crate) fn reconcile_configuration(&self) -> ConfigurationActivation<'_> {
        ConfigurationActivation { registry: self }
    }
}

#[must_use]
pub(crate) struct ConfigurationActivation<'a> {
    registry: &'a RuntimeRegistry,
}

impl ConfigurationActivation<'_> {
    pub(crate) fn notify_after_snapshot_swap(self) {
        self.registry.advance_scheduler_epoch();
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
