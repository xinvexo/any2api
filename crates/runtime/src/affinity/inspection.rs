use std::time::{Duration, Instant};

use super::{
    registry::{AffinityRegistry, BindingSource, BindingState},
    snapshot::AffinityRuntimeSnapshot,
};

impl AffinityRegistry {
    /// Counts are aggregated shard by shard, so the snapshot is a slightly
    /// smeared view when bindings change concurrently; per-shard counts are
    /// exact at the instant each shard is visited.
    pub(crate) fn snapshot(&self, ttl: Duration, enabled: bool) -> AffinityRuntimeSnapshot {
        let now = Instant::now();
        let mut active_session_count = 0;
        let mut creating_session_count = 0;
        self.bindings.for_each_shard(|shard| {
            shard.remove_expired(now, ttl);
            if !enabled {
                return;
            }
            for (_, entry) in shard.iter() {
                match entry {
                    BindingState::Creating { .. } => creating_session_count += 1,
                    BindingState::Bound { binding }
                        if matches!(binding.source, BindingSource::Session) =>
                    {
                        active_session_count += 1;
                    }
                    BindingState::Bound { .. } => {}
                }
            }
        });
        AffinityRuntimeSnapshot {
            active_session_count,
            creating_session_count,
        }
    }

    pub(crate) fn sweep_expired(&self, ttl: Duration) -> usize {
        self.bindings.remove_expired_all(Instant::now(), ttl)
    }
}
