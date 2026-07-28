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
pub use snapshot::{AffinityBindingSummary, AffinityCredentialCount, AffinityRuntimeSnapshot};

pub(crate) use commit::ContinuationBindingCommitter;
pub(crate) use lease::{BindingLease, BindingStart};
pub(crate) use registry::{AffinityError, AffinityRegistry};
pub(crate) use sweeper::start as start_sweeper;
pub(crate) use target::AffinityTarget;

#[cfg(test)]
mod tests;
