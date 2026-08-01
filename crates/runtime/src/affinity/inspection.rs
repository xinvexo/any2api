use std::time::{Duration, Instant};

use super::{
    registry::{AffinityRegistry, AffinityState, BindingSource, BindingState},
    snapshot::AffinityRuntimeSnapshot,
};

impl AffinityRegistry {
    pub(crate) fn snapshot(&self, ttl: Duration, enabled: bool) -> AffinityRuntimeSnapshot {
        let now = Instant::now();
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        remove_expired(&mut state, now, ttl);
        build_snapshot(&state, enabled)
    }

    pub(crate) fn sweep_expired(&self, ttl: Duration) -> usize {
        let now = Instant::now();
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        let before = state.entries.len();
        remove_expired(&mut state, now, ttl);
        before - state.entries.len()
    }
}

fn remove_expired(state: &mut AffinityState, now: Instant, ttl: Duration) {
    state.remove_expired(now, ttl);
}

fn build_snapshot(state: &AffinityState, enabled: bool) -> AffinityRuntimeSnapshot {
    if !enabled {
        return AffinityRuntimeSnapshot {
            active_session_count: 0,
            creating_session_count: 0,
        };
    }

    let mut active_session_count = 0;
    let mut creating_session_count = 0;
    for entry in state.entries.values() {
        match entry {
            BindingState::Creating { .. } => creating_session_count += 1,
            BindingState::Bound { binding } if matches!(binding.source, BindingSource::Session) => {
                active_session_count += 1;
            }
            BindingState::Bound { .. } => {}
        }
    }
    AffinityRuntimeSnapshot {
        active_session_count,
        creating_session_count,
    }
}
