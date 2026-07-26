mod commit;
mod hash;
mod inspection;
mod lease;
mod policy;
mod registry;
mod snapshot;
mod sweeper;
mod target;

pub use policy::AffinityPolicy;
pub use snapshot::{
    AffinityBindingKind, AffinityBindingSummary, AffinityCredentialCount, AffinityRuntimeSnapshot,
};

pub(crate) use commit::HardAffinityCommitter;
pub(crate) use lease::{SoftBindingLease, SoftBindingStart};
pub(crate) use registry::{AffinityError, AffinityRegistry};
pub(crate) use sweeper::start as start_sweeper;
pub(crate) use target::AffinityTarget;

#[cfg(test)]
mod tests;
