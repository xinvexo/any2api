mod commit;
mod continuation;
mod continuation_lease;
mod hash;
mod inspection;
mod lease;
mod policy;
mod registry;
mod shard;
mod snapshot;
mod sweeper;
mod target;

pub use policy::AffinityPolicy;
pub use snapshot::AffinityRuntimeSnapshot;

pub(crate) use commit::ContinuationBindingCommitter;
pub(crate) use continuation::{ContinuationLookup, ResolvedContinuation};
pub(crate) use continuation_lease::ContinuationLease;
pub(crate) use lease::{BindingLease, BindingStart};
pub(crate) use registry::{AffinityError, AffinityRegistry, BindingCreationPhase};
pub(crate) use sweeper::start as start_sweeper;
pub(crate) use target::AffinityTarget;

#[cfg(test)]
mod tests;
