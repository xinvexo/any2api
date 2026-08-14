use any2api_domain::ProviderKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BalancingRuntimeSnapshot {
    pub(super) scheduler_epoch: u64,
    pub(super) queue: BalancingQueueSnapshot,
    pub(super) totals: BalancingTotalsSnapshot,
    pub(super) providers: Vec<BalancingProviderSnapshot>,
    pub(super) breakers: BreakerStateCounts,
}

impl BalancingRuntimeSnapshot {
    pub const fn scheduler_epoch(&self) -> u64 {
        self.scheduler_epoch
    }

    pub const fn queue(&self) -> BalancingQueueSnapshot {
        self.queue
    }

    pub const fn totals(&self) -> BalancingTotalsSnapshot {
        self.totals
    }

    pub fn providers(&self) -> &[BalancingProviderSnapshot] {
        &self.providers
    }

    pub const fn breakers(&self) -> BreakerStateCounts {
        self.breakers
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BreakerStateCounts {
    pub(super) closed: usize,
    pub(super) open: usize,
    pub(super) half_open: usize,
}

impl BreakerStateCounts {
    pub const fn closed(self) -> usize {
        self.closed
    }

    pub const fn open(self) -> usize {
        self.open
    }

    pub const fn half_open(self) -> usize {
        self.half_open
    }

    pub(crate) fn record(&mut self, state: crate::health::CircuitStateSnapshot) {
        match state {
            crate::health::CircuitStateSnapshot::Closed => self.closed += 1,
            crate::health::CircuitStateSnapshot::Open => self.open += 1,
            crate::health::CircuitStateSnapshot::HalfOpen => self.half_open += 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BalancingQueueSnapshot {
    pub(super) waiting: u32,
    pub(super) max_waiting: u32,
    pub(super) timeout_secs: u64,
    pub(super) rejects_when_rate_limited: bool,
    pub(super) fallback_on_rate_limit: bool,
}

impl BalancingQueueSnapshot {
    pub const fn waiting(self) -> u32 {
        self.waiting
    }

    pub const fn max_waiting(self) -> u32 {
        self.max_waiting
    }

    pub const fn timeout_secs(self) -> u64 {
        self.timeout_secs
    }

    pub const fn rejects_when_rate_limited(self) -> bool {
        self.rejects_when_rate_limited
    }

    pub const fn fallback_on_rate_limit(self) -> bool {
        self.fallback_on_rate_limit
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BalancingTotalsSnapshot {
    pub(super) credential_count: usize,
    pub(super) enabled_credential_count: usize,
    pub(super) limited_credential_count: usize,
    pub(super) rate_limited_credential_count: usize,
    pub(super) in_flight: u64,
    pub(super) requests_in_window: u64,
    pub(super) fixed_waiters: u64,
    pub(super) selected: u64,
}

impl BalancingTotalsSnapshot {
    pub const fn credential_count(self) -> usize {
        self.credential_count
    }

    pub const fn enabled_credential_count(self) -> usize {
        self.enabled_credential_count
    }

    pub const fn limited_credential_count(self) -> usize {
        self.limited_credential_count
    }

    pub const fn rate_limited_credential_count(self) -> usize {
        self.rate_limited_credential_count
    }

    pub const fn in_flight(self) -> u64 {
        self.in_flight
    }

    pub const fn requests_in_window(self) -> u64 {
        self.requests_in_window
    }

    pub const fn fixed_waiters(self) -> u64 {
        self.fixed_waiters
    }

    pub const fn selected(self) -> u64 {
        self.selected
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BalancingProviderSnapshot {
    pub(super) provider_kind: ProviderKind,
    pub(super) credential_count: usize,
    pub(super) enabled_credential_count: usize,
    pub(super) limited_credential_count: usize,
    pub(super) rate_limited_credential_count: usize,
    pub(super) in_flight: u64,
    pub(super) requests_in_window: u64,
    pub(super) fixed_waiters: u64,
    pub(super) selected: u64,
}

impl BalancingProviderSnapshot {
    pub const fn provider_kind(self) -> ProviderKind {
        self.provider_kind
    }

    pub const fn credential_count(self) -> usize {
        self.credential_count
    }

    pub const fn enabled_credential_count(self) -> usize {
        self.enabled_credential_count
    }

    pub const fn limited_credential_count(self) -> usize {
        self.limited_credential_count
    }

    pub const fn rate_limited_credential_count(self) -> usize {
        self.rate_limited_credential_count
    }

    pub const fn in_flight(self) -> u64 {
        self.in_flight
    }

    pub const fn requests_in_window(self) -> u64 {
        self.requests_in_window
    }

    pub const fn fixed_waiters(self) -> u64 {
        self.fixed_waiters
    }

    pub const fn selected(self) -> u64 {
        self.selected
    }
}
