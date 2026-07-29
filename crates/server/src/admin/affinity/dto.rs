use any2api_runtime::api::{AffinityRuntimeSnapshot, PublishedSnapshot};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct AffinityRuntimeResponse {
    config_revision: u64,
    affinity_enabled: bool,
    active_session_count: usize,
    creating_session_count: usize,
}

impl AffinityRuntimeResponse {
    pub(crate) fn new(published: &PublishedSnapshot, snapshot: &AffinityRuntimeSnapshot) -> Self {
        Self {
            config_revision: published.revision().get(),
            affinity_enabled: published.affinity_policy().enabled(),
            active_session_count: snapshot.active_session_count(),
            creating_session_count: snapshot.creating_session_count(),
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct AffinityClearResponse {
    cleared_count: usize,
}

impl AffinityClearResponse {
    pub(crate) const fn new(cleared_count: usize) -> Self {
        Self { cleared_count }
    }
}
