use tokio::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HealthAcquireError {
    Temporary(TemporaryUnavailability),
    Permanent,
}

/// A transient health block together with the class of condition that raised
/// it, so selection can decide between failing over and waiting in place.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TemporaryUnavailability {
    until: Instant,
    cause: TemporaryUnavailabilityCause,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TemporaryUnavailabilityCause {
    /// Circuit breaker open, transient endpoint or proxy failure, permission
    /// cooldown, or model-unavailable cooldown: another candidate or a lower
    /// fallback tier should serve the request instead of waiting.
    Outage,
    /// Upstream rate-limit or quota-exhaustion cooldown: whether to wait or
    /// spill to a lower tier is governed by the fallback-on-rate-limit policy.
    RateLimitCooldown,
}

impl TemporaryUnavailability {
    pub(crate) const fn new(until: Instant, cause: TemporaryUnavailabilityCause) -> Self {
        Self { until, cause }
    }

    pub(crate) const fn outage(until: Instant) -> Self {
        Self::new(until, TemporaryUnavailabilityCause::Outage)
    }

    pub(crate) const fn rate_limit_cooldown(until: Instant) -> Self {
        Self::new(until, TemporaryUnavailabilityCause::RateLimitCooldown)
    }

    pub(crate) const fn until(self) -> Instant {
        self.until
    }

    pub(crate) const fn cause(self) -> TemporaryUnavailabilityCause {
        self.cause
    }
}
