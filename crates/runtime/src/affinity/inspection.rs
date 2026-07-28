use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use any2api_domain::RoutingCredentialId;

use super::{
    hash::SessionHash,
    registry::{AffinityRegistry, AffinityState, BindingState, TimedBinding},
    snapshot::{AffinityBindingSummary, AffinityCredentialCount, AffinityRuntimeSnapshot},
};

impl AffinityRegistry {
    pub(crate) fn snapshot(&self, ttl: Duration, limit: usize) -> AffinityRuntimeSnapshot {
        let now = Instant::now();
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        remove_expired(&mut state, now, ttl);
        build_snapshot(&state, now, ttl, limit)
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
    state.entries.retain(|_, entry| match entry {
        BindingState::Creating { .. } => true,
        BindingState::Bound { binding } => {
            now.saturating_duration_since(binding.last_seen_at) < ttl
        }
    });
}

fn build_snapshot(
    state: &AffinityState,
    now: Instant,
    ttl: Duration,
    limit: usize,
) -> AffinityRuntimeSnapshot {
    let include_details = limit > 0;
    let mut counts = HashMap::<RoutingCredentialId, usize>::new();
    let mut bindings = Vec::new();
    let mut binding_count = 0;
    let mut creating_count = 0;
    for (hash, entry) in &state.entries {
        match entry {
            BindingState::Creating { .. } => creating_count += 1,
            BindingState::Bound { binding } => {
                binding_count += 1;
                if include_details {
                    *counts.entry(binding.target.credential_id()).or_default() += 1;
                    push_summary(
                        &mut bindings,
                        limit,
                        *hash,
                        binding,
                        remaining_ms(now, binding.last_seen_at, ttl),
                    );
                }
            }
        }
    }
    bindings.sort_by(|left, right| left.session_hash_prefix.cmp(&right.session_hash_prefix));
    let mut credential_counts = counts
        .into_iter()
        .map(|(credential_id, bindings)| AffinityCredentialCount {
            credential_id,
            bindings,
        })
        .collect::<Vec<_>>();
    credential_counts.sort_by_key(AffinityCredentialCount::credential_id);
    AffinityRuntimeSnapshot {
        binding_count,
        creating_count,
        credential_counts,
        bindings,
    }
}

fn push_summary(
    output: &mut Vec<AffinityBindingSummary>,
    limit: usize,
    hash: SessionHash,
    binding: &TimedBinding,
    expires_in_ms: u64,
) {
    if output.len() >= limit {
        return;
    }
    output.push(AffinityBindingSummary {
        session_hash_prefix: hash.prefix(),
        credential_id: binding.target.credential_id(),
        route_target_id: binding.target.target_id(),
        upstream_model: binding.target.upstream_model().to_owned(),
        protocol_dialect: binding.target.protocol_dialect(),
        expires_in_ms,
    });
}

fn remaining_ms(now: Instant, last_seen: Instant, ttl: Duration) -> u64 {
    let elapsed = now.saturating_duration_since(last_seen);
    u64::try_from(ttl.saturating_sub(elapsed).as_millis()).unwrap_or(u64::MAX)
}
