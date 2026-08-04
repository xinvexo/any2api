use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};

use any2api_domain::ModelRouteId;

use super::{CandidateRequirements, RouteCandidate};

pub(crate) type RouteCandidateTiers = BTreeMap<u16, Vec<RouteCandidate>>;

/// Shares the candidate set for one published snapshot per (route,
/// requirements) pair, built on first access; every input is immutable for
/// the snapshot's lifetime, so request paths only clone the `Arc`.
#[derive(Debug, Default)]
pub(crate) struct RouteCandidateCache {
    entries: RwLock<HashMap<RouteCandidateCacheKey, Arc<RouteCandidateTiers>>>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct RouteCandidateCacheKey {
    route_id: ModelRouteId,
    requirements: CandidateRequirements,
}

impl RouteCandidateCache {
    pub(crate) fn get_or_build(
        &self,
        route_id: ModelRouteId,
        requirements: CandidateRequirements,
        build: impl FnOnce() -> RouteCandidateTiers,
    ) -> Arc<RouteCandidateTiers> {
        let key = RouteCandidateCacheKey {
            route_id,
            requirements,
        };
        if let Some(cached) = self
            .entries
            .read()
            .expect("route candidate cache lock poisoned")
            .get(&key)
        {
            return Arc::clone(cached);
        }
        let built = Arc::new(build());
        // Synthetic OAuth routes are keyed by request-supplied model names;
        // skipping empty sets keeps unknown models from growing the cache.
        if built.is_empty() {
            return built;
        }
        Arc::clone(
            self.entries
                .write()
                .expect("route candidate cache lock poisoned")
                .entry(key)
                .or_insert(built),
        )
    }

    #[cfg(test)]
    pub(crate) fn entry_count(&self) -> usize {
        self.entries
            .read()
            .expect("route candidate cache lock poisoned")
            .len()
    }
}
