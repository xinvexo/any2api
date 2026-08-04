use std::{hint::black_box, sync::Arc, time::Instant};

use any2api_domain::{CredentialId, ModelRouteId};

use super::{TTL, target};
use crate::affinity::{
    AffinityRegistry, AffinityTarget,
    registry::{BindingSource, BindingState, ContinuationLifecycle, TimedBinding},
};

const BINDING_COUNTS: [usize; 4] = [1, 10_000, 100_000, 250_000];
const EVENTS_PER_SAMPLE: usize = 128;
const SAMPLE_COUNT: usize = 5;

#[test]
#[ignore = "manual continuation hot-path scaling benchmark"]
fn continuation_commit_scaling_benchmark() {
    for binding_count in BINDING_COUNTS {
        let (registry, hot_identity, hot_target) = seeded_registry(binding_count);
        let mut samples = Vec::with_capacity(SAMPLE_COUNT);
        for _ in 0..SAMPLE_COUNT {
            let started = Instant::now();
            for _ in 0..EVENTS_PER_SAMPLE {
                registry
                    .bind_ready_continuation(
                        black_box(&hot_identity),
                        black_box(hot_target.clone()),
                        None,
                        TTL,
                    )
                    .expect("idempotent continuation commit");
            }
            samples.push(started.elapsed());
        }
        samples.sort_unstable();
        let median = samples[SAMPLE_COUNT / 2];
        eprintln!(
            "continuation commit benchmark: bindings={binding_count}, events={EVENTS_PER_SAMPLE}, \
             median={median:?}, ns_per_event={}",
            median.as_nanos() / EVENTS_PER_SAMPLE as u128,
        );
    }
}

fn seeded_registry(binding_count: usize) -> (Arc<AffinityRegistry>, String, AffinityTarget) {
    let registry = AffinityRegistry::new();
    let target = target(ModelRouteId::new(), CredentialId::new());
    let hot_identity = "resp-benchmark-hot".to_owned();
    let now = Instant::now();
    let mut state = registry.state.lock().expect("affinity state");
    state.entries.reserve(binding_count);
    for index in 0..binding_count {
        let raw = if index == 0 {
            hot_identity.clone()
        } else {
            format!("resp-benchmark-{index}")
        };
        state.entries.insert(
            registry.hasher.continuation(&raw),
            BindingState::Bound {
                binding: TimedBinding {
                    target: target.clone(),
                    last_seen_at: now,
                    source: BindingSource::Continuation(ContinuationLifecycle::Ready {
                        state: None,
                    }),
                },
            },
        );
    }
    drop(state);
    (registry, hot_identity, target)
}
