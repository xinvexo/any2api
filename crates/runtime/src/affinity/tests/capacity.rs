use std::time::{Duration, Instant};

use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect};

use super::{TTL, target};
use crate::affinity::{AffinityError, AffinityRegistry, BindingStart};

fn bind_session(registry: &std::sync::Arc<AffinityRegistry>, raw: &str) {
    let route_id = ModelRouteId::new();
    let mut lease = match registry
        .begin_session(ProtocolDialect::OpenAiResponses, route_id, raw, TTL)
        .expect("session lease")
    {
        BindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };
    lease.mark_attempting().expect("promote session binding");
    lease
        .commit(target(route_id, CredentialId::new()))
        .expect("commit session binding");
}

#[test]
fn full_registry_rejects_new_sessions_until_expired_entries_are_swept() {
    let registry = AffinityRegistry::with_max_bindings_for_test(2);
    bind_session(&registry, "session-one");
    bind_session(&registry, "session-two");

    assert_eq!(
        registry
            .begin_session(
                ProtocolDialect::OpenAiResponses,
                ModelRouteId::new(),
                "session-over-capacity",
                TTL,
            )
            .err(),
        Some(AffinityError::Capacity)
    );

    registry.stale_bound_entries_for_test(Instant::now() - TTL - Duration::from_secs(1));
    assert_eq!(registry.sweep_expired(TTL), 2);
    assert!(matches!(
        registry
            .begin_session(
                ProtocolDialect::OpenAiResponses,
                ModelRouteId::new(),
                "session-after-sweep",
                TTL,
            )
            .expect("swept registry accepts new sessions"),
        BindingStart::Create(_)
    ));
}

#[test]
fn capacity_pressure_reclaims_expired_entries_without_waiting_for_the_sweeper() {
    let registry = AffinityRegistry::with_max_bindings_for_test(2);
    bind_session(&registry, "session-one");
    bind_session(&registry, "session-two");
    registry.stale_bound_entries_for_test(Instant::now() - TTL - Duration::from_secs(1));

    assert!(matches!(
        registry
            .begin_session(
                ProtocolDialect::OpenAiResponses,
                ModelRouteId::new(),
                "session-after-reclaim",
                TTL,
            )
            .expect("capacity pressure reclaims expired entries"),
        BindingStart::Create(_)
    ));
}

#[test]
fn futile_capacity_scans_are_throttled() {
    let registry = AffinityRegistry::with_max_bindings_for_test(1);
    bind_session(&registry, "session-pinned");

    for raw in ["session-rejected-one", "session-rejected-two"] {
        assert_eq!(
            registry
                .begin_session(
                    ProtocolDialect::OpenAiResponses,
                    ModelRouteId::new(),
                    raw,
                    TTL,
                )
                .err(),
            Some(AffinityError::Capacity)
        );
    }

    assert_eq!(
        registry.full_reclaims_for_test(),
        1,
        "a fresh rejection after a futile scan must not rescan the table"
    );
}
