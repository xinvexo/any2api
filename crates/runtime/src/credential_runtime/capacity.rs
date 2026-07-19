const IN_FLIGHT_MASK: u64 = u32::MAX as u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CredentialCapacity {
    in_flight: u32,
    max_concurrency: u32,
}

impl CredentialCapacity {
    #[must_use]
    pub const fn in_flight(self) -> u32 {
        self.in_flight
    }

    #[must_use]
    pub const fn max_concurrency(self) -> u32 {
        self.max_concurrency
    }

    #[must_use]
    pub const fn is_full(self) -> bool {
        self.in_flight >= self.max_concurrency
    }

    pub(super) const fn full(max_concurrency: u32) -> Self {
        Self {
            in_flight: max_concurrency,
            max_concurrency,
        }
    }
}

pub(super) const fn pack(max_concurrency: u32, in_flight: u32) -> u64 {
    ((max_concurrency as u64) << 32) | in_flight as u64
}

pub(super) const fn unpack(state: u64) -> CredentialCapacity {
    CredentialCapacity {
        in_flight: (state & IN_FLIGHT_MASK) as u32,
        max_concurrency: (state >> 32) as u32,
    }
}
