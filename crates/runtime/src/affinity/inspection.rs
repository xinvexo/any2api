use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use any2api_domain::RoutingCredentialId;

use super::{
    hash::SessionHash,
    registry::{AffinityRegistry, AffinityState, SoftState, TimedBinding},
    snapshot::{
        AffinityBindingKind, AffinityBindingSummary, AffinityCredentialCount,
        AffinityRuntimeSnapshot,
    },
};

impl AffinityRegistry {
    pub(crate) fn snapshot(
        &self,
        soft_ttl: Duration,
        hard_ttl: Duration,
        creating_ttl: Duration,
        limit: usize,
    ) -> AffinityRuntimeSnapshot {
        let now = Instant::now();
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        remove_expired(&mut state, now, soft_ttl, hard_ttl, creating_ttl);
        build_snapshot(&state, now, soft_ttl, hard_ttl, limit)
    }
}

fn remove_expired(
    state: &mut AffinityState,
    now: Instant,
    soft_ttl: Duration,
    hard_ttl: Duration,
    creating_ttl: Duration,
) {
    state
        .hard
        .retain(|_, binding| now.saturating_duration_since(binding.last_seen_at) < hard_ttl);
    state.soft.retain(|_, binding| match binding {
        SoftState::Creating {
            started_at,
            changes,
            ..
        } => {
            let keep = now.saturating_duration_since(*started_at) < creating_ttl;
            if !keep {
                changes.send_replace(1);
            }
            keep
        }
        SoftState::Bound { binding, .. } => {
            now.saturating_duration_since(binding.last_seen_at) < soft_ttl
        }
    });
}

fn build_snapshot(
    state: &AffinityState,
    now: Instant,
    soft_ttl: Duration,
    hard_ttl: Duration,
    limit: usize,
) -> AffinityRuntimeSnapshot {
    let mut counts = HashMap::<RoutingCredentialId, (usize, usize)>::new();
    let mut bindings = Vec::new();
    let mut soft_binding_count = 0;
    let mut creating_count = 0;
    for (hash, binding) in &state.soft {
        match binding {
            SoftState::Creating { .. } => creating_count += 1,
            SoftState::Bound { binding, .. } => {
                soft_binding_count += 1;
                counts.entry(binding.target.credential_id()).or_default().0 += 1;
                push_summary(
                    &mut bindings,
                    limit,
                    *hash,
                    binding,
                    AffinityBindingKind::Soft,
                    remaining_ms(now, binding.last_seen_at, soft_ttl),
                );
            }
        }
    }
    for (hash, binding) in &state.hard {
        counts.entry(binding.target.credential_id()).or_default().1 += 1;
        push_summary(
            &mut bindings,
            limit,
            *hash,
            binding,
            AffinityBindingKind::Hard,
            remaining_ms(now, binding.last_seen_at, hard_ttl),
        );
    }
    bindings.sort_by(|left, right| left.session_hash_prefix.cmp(&right.session_hash_prefix));
    let mut credential_counts = counts
        .into_iter()
        .map(
            |(credential_id, (soft_bindings, hard_bindings))| AffinityCredentialCount {
                credential_id,
                soft_bindings,
                hard_bindings,
            },
        )
        .collect::<Vec<_>>();
    credential_counts.sort_by_key(AffinityCredentialCount::credential_id);
    AffinityRuntimeSnapshot {
        soft_binding_count,
        hard_binding_count: state.hard.len(),
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
    kind: AffinityBindingKind,
    expires_in_ms: u64,
) {
    if output.len() >= limit {
        return;
    }
    output.push(AffinityBindingSummary {
        kind,
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
